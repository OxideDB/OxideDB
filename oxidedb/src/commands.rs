//! Command Handlers
//!
//! This module contains the implementation of CLI commands for OxideDB,
//! providing clean separation between CLI parsing and business logic.

use crate::{
    config::{
        AnalyzePluginArgs, InstallPluginArgs, IssueLicenseArgs, LicenseArgs, LicenseEdition,
        LicenseSubcommands, ManagePluginsArgs, PluginNameArgs, PluginSubcommands,
        RegisterSuperuserArgs, SignPluginArgs, StartArgs,
    },
    startup::ApplicationBootstrap,
    OxideDbConfig, Result,
};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use ed25519_dalek::{Signer, SigningKey};
use oxide_api::services::{DatabasePermissionService, PluginConfigService};
use oxide_core::{
    plugin_security::{PluginCapability, PluginTrustLevel, ResourceLimits, VfsOperation},
    register_system_hooks, AppError, AuthService, EventBus, InMemoryEventBus,
};
use oxide_db::SqliteDb;
use oxide_plugin_runtime::PluginManager;
use serde::Serialize;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tracing::{error, info};
use zip::{write::FileOptions, ZipArchive, ZipWriter};

const PLUGIN_SIGNATURE_FILE_NAME: &str = "plugin.sig";
const PLUGIN_SIGNATURE_PAYLOAD_MAGIC: &[u8] = b"OxideDB plugin package signature v1\n";
const PLUGIN_SIGNING_KEY_ENV: &str = "OXIDEDB_PLUGIN_SIGNING_KEY";
const LICENSE_SIGNING_KEY_ENV: &str = "OXIDEDB_LICENSE_SIGNING_KEY";

/// Trait for command handlers to enable modular command processing
pub trait CommandHandler {
    type Args;
    fn execute(args: Self::Args) -> impl std::future::Future<Output = Result<()>> + Send;
}

/// Handler for the start server command
pub struct StartCommand;

impl CommandHandler for StartCommand {
    type Args = StartArgs;

    async fn execute(args: Self::Args) -> Result<()> {
        // Create and validate configuration
        let config = OxideDbConfig::from_start_args(&args);
        config.validate()?;

        // Initialize application
        let bootstrap = ApplicationBootstrap::new(config);
        let services = bootstrap.initialize().await?;

        // Start server (this will run indefinitely)
        bootstrap.start_server(services).await
    }
}

/// Handler for the register superuser command
pub struct RegisterSuperuserCommand;

impl CommandHandler for RegisterSuperuserCommand {
    type Args = RegisterSuperuserArgs;

    async fn execute(args: Self::Args) -> Result<()> {
        info!("Registering superuser account");
        info!("Database path: {:?}", args.db_path);
        info!("Email: {}", args.email);

        // Initialize minimal services needed for user registration
        let services = Self::initialize_minimal_services(&args).await?;

        // Register the superuser
        Self::register_superuser(&args, &services).await?;

        info!("✅ Superuser registration completed successfully");
        Ok(())
    }
}

impl RegisterSuperuserCommand {
    /// Initialize minimal services needed for superuser registration
    async fn initialize_minimal_services(args: &RegisterSuperuserArgs) -> Result<MinimalServices> {
        let event_bus: Arc<dyn EventBus> = Arc::new(InMemoryEventBus::new());

        let jwt_secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "dev_secret_key_change_in_production".to_string());
        let auth_config = oxide_core::auth::AuthServiceConfig::new(jwt_secret);
        let auth_service = Arc::new(AuthService::new(auth_config));

        // Resolve database path
        let database_path = Self::resolve_database_path(&args.db_path)?;

        let database = Arc::new(SqliteDb::new(
            &database_path,
            Arc::clone(&event_bus),
            Arc::clone(&auth_service),
        )?);
        database.initialize().await?;

        // Update auth service with discovered collections
        let auth_collections = database.list_auth_collections().await?;
        auth_service.update_auth_collections(&auth_collections);

        // Create permission service for authorization hooks
        let permission_service = Arc::new(DatabasePermissionService::new(
            Arc::clone(&database) as Arc<dyn oxide_db::Db>
        ));

        // Register all system hooks using the new centralized system
        // This is CRITICAL for password hashing to work properly!
        register_system_hooks(
            event_bus.as_ref(),
            Arc::clone(&auth_service),
            Arc::clone(&permission_service) as Arc<dyn oxide_core::auth::PermissionService>,
        )
        .await?;
        info!("✅ System hooks registered for password hashing and validation");

        Ok(MinimalServices {
            database,
            auth_service,
        })
    }

    /// Resolve the database path, handling directory vs file paths
    fn resolve_database_path(db_path: &std::path::Path) -> Result<String> {
        // Ensure the database directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::internal(format!("Failed to create database directory: {}", e))
            })?;
        }

        let database_path = if db_path.is_dir() {
            db_path.join("oxidedb.sqlite").to_string_lossy().to_string()
        } else {
            // Ensure parent directory exists for file path
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    AppError::internal(format!("Failed to create database directory: {}", e))
                })?;
            }
            db_path.to_string_lossy().to_string()
        };

        Ok(database_path)
    }

    /// Register the superuser with the given arguments
    async fn register_superuser(
        args: &RegisterSuperuserArgs,
        services: &MinimalServices,
    ) -> Result<()> {
        // Find superuser collection or use default
        let auth_collections = services.database.list_auth_collections().await?;
        let superuser_collection = auth_collections
            .iter()
            .find(|c| c.name == "_superusers")
            .map(|c| c.name.as_str())
            .unwrap_or("_users");

        if let Some(superuser_config) = services
            .auth_service
            .config()
            .get_auth_collection(superuser_collection)
        {
            let register_request = oxide_db::db::RegisterRequest {
                collection: superuser_collection.to_string(),
                identifier: args.email.clone(),
                credential: args.password.clone(),
                additional_data: Some(serde_json::json!({
                    "verified": true,
                    "name": args.name.clone().unwrap_or_else(|| "System Administrator".to_string()),
                    "role": "superuser"
                })),
            };

            match services
                .database
                .register_user(register_request, &superuser_config)
                .await
            {
                Ok(user_id) => {
                    info!(
                        "✅ Superuser registered successfully with ID: {} in collection '{}'",
                        user_id, superuser_collection
                    );
                }
                Err(e) => {
                    error!("❌ Failed to register superuser: {}", e);
                    return Err(e);
                }
            }
        } else {
            return Err(AppError::internal(format!(
                "Auth collection '{}' not found",
                superuser_collection
            )));
        }

        Ok(())
    }
}

/// Minimal services needed for superuser registration
struct MinimalServices {
    database: Arc<SqliteDb>,
    auth_service: Arc<AuthService>,
}

/// Handler for the license management command
pub struct LicenseCommand;

impl CommandHandler for LicenseCommand {
    type Args = LicenseArgs;

    async fn execute(args: Self::Args) -> Result<()> {
        match args.command {
            LicenseSubcommands::Issue(ref issue_args) => Self::issue_license(issue_args).await,
        }
    }
}

impl LicenseCommand {
    async fn issue_license(args: &IssueLicenseArgs) -> Result<()> {
        let document = Self::issue_license_document(args)?;

        if let Some(output) = &args.output {
            if let Some(parent) = output
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                fs::create_dir_all(parent).map_err(|e| {
                    AppError::internal(format!(
                        "Failed to create license output directory '{}': {}",
                        parent.display(),
                        e
                    ))
                })?;
            }

            fs::write(output, format!("{document}\n")).map_err(|e| {
                AppError::internal(format!(
                    "Failed to write license document '{}': {}",
                    output.display(),
                    e
                ))
            })?;
            println!("Signed license written to {}", output.display());
        } else {
            println!("{document}");
        }

        Ok(())
    }

    fn issue_license_document(args: &IssueLicenseArgs) -> Result<String> {
        let now = current_unix_timestamp();
        if args.expires_at <= now {
            return Err(AppError::validation(
                "expires_at",
                "License expiration must be in the future",
            ));
        }

        if let Some(not_before) = args.not_before {
            if not_before >= args.expires_at {
                return Err(AppError::validation(
                    "not_before",
                    "License not_before must be earlier than expires_at",
                ));
            }
        }

        let signing_key = Self::load_license_signing_key(args)?;
        let claims = IssuedLicenseClaims {
            edition: license_edition_name(&args.edition).to_string(),
            expires_at: args.expires_at,
            installation_id: args.installation_id.clone(),
            not_before: args.not_before,
            issued_at: now,
        };
        let payload = serde_json::to_vec(&claims)
            .map_err(|e| AppError::internal(format!("Failed to encode license claims: {}", e)))?;
        let signature = signing_key.sign(&payload);

        let document = serde_json::json!({
            "algorithm": "ed25519",
            "payload": URL_SAFE_NO_PAD.encode(payload),
            "signature": URL_SAFE_NO_PAD.encode(signature.to_bytes()),
        });

        serde_json::to_string(&document)
            .map_err(|e| AppError::internal(format!("Failed to encode license document: {}", e)))
    }

    fn load_license_signing_key(args: &IssueLicenseArgs) -> Result<SigningKey> {
        let raw_material = if let Some(private_key) = &args.private_key {
            private_key.clone()
        } else if let Some(key_file) = &args.key_file {
            fs::read_to_string(key_file).map_err(|e| {
                AppError::internal(format!(
                    "Failed to read license signing key file '{}': {}",
                    key_file.display(),
                    e
                ))
            })?
        } else if let Ok(private_key) = std::env::var(LICENSE_SIGNING_KEY_ENV) {
            private_key
        } else {
            return Err(AppError::validation(
                "private_key",
                "Provide --private-key, --key-file, or OXIDEDB_LICENSE_SIGNING_KEY",
            ));
        };

        let key_material = ManagePluginsCommand::extract_key_material(&raw_material)
            .ok_or_else(|| AppError::validation("private_key", "Signing key material was empty"))?;
        let key_bytes = ManagePluginsCommand::decode_key_material(&key_material).map_err(|e| {
            AppError::validation(
                "private_key",
                &format!("Invalid signing key encoding: {}", e),
            )
        })?;

        signing_key_from_bytes(key_bytes)
    }
}

#[derive(Serialize)]
struct IssuedLicenseClaims {
    edition: String,
    expires_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    installation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    not_before: Option<i64>,
    issued_at: i64,
}

fn license_edition_name(edition: &LicenseEdition) -> &'static str {
    match edition {
        LicenseEdition::Professional => "professional",
        LicenseEdition::Enterprise => "enterprise",
    }
}

fn signing_key_from_bytes(key_bytes: Vec<u8>) -> Result<SigningKey> {
    match key_bytes.len() {
        32 => {
            let seed: [u8; 32] = key_bytes
                .try_into()
                .map_err(|_| AppError::validation("private_key", "Invalid Ed25519 seed length"))?;
            Ok(SigningKey::from_bytes(&seed))
        }
        64 => {
            let keypair: [u8; 64] = key_bytes.try_into().map_err(|_| {
                AppError::validation("private_key", "Invalid Ed25519 keypair length")
            })?;
            SigningKey::from_keypair_bytes(&keypair).map_err(|e| {
                AppError::validation(
                    "private_key",
                    &format!("Invalid Ed25519 keypair bytes: {}", e),
                )
            })
        }
        length => Err(AppError::validation(
            "private_key",
            &format!(
                "Invalid Ed25519 private key length: expected 32-byte seed or 64-byte keypair, got {} bytes",
                length
            ),
        )),
    }
}

fn current_unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Handler for the plugin management command
pub struct ManagePluginsCommand;

impl CommandHandler for ManagePluginsCommand {
    type Args = ManagePluginsArgs;

    async fn execute(args: Self::Args) -> Result<()> {
        match args.command {
            PluginSubcommands::List => Self::list_plugins(&args).await,
            PluginSubcommands::Install(ref install_args) => {
                Self::install_plugin(&args, install_args).await
            }
            PluginSubcommands::Enable(ref name_args) => Self::enable_plugin(&args, name_args).await,
            PluginSubcommands::Disable(ref name_args) => {
                Self::disable_plugin(&args, name_args).await
            }
            PluginSubcommands::Uninstall(ref name_args) => {
                Self::uninstall_plugin(&args, name_args).await
            }
            PluginSubcommands::Show(ref name_args) => Self::show_plugin(&args, name_args).await,
            PluginSubcommands::Analyze(ref analyze_args) => {
                Self::analyze_plugin(&args, analyze_args).await
            }
            PluginSubcommands::Sign(ref sign_args) => Self::sign_plugin(sign_args).await,
        }
    }
}

impl ManagePluginsCommand {
    /// Initialize minimal services for plugin management
    async fn initialize_services(args: &ManagePluginsArgs) -> Result<PluginManagementServices> {
        let event_bus: Arc<dyn EventBus> = Arc::new(InMemoryEventBus::new());

        let jwt_secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "dev_secret_key_change_in_production".to_string());
        let auth_config = oxide_core::auth::AuthServiceConfig::new(jwt_secret);
        let auth_service = Arc::new(AuthService::new(auth_config));

        // Resolve database path
        let database_path = Self::resolve_database_path(&args.db_path)?;

        let database = Arc::new(SqliteDb::new(
            &database_path,
            Arc::clone(&event_bus),
            Arc::clone(&auth_service),
        )?);
        database.initialize().await?;

        // Update auth service with discovered collections
        let auth_collections = database.list_auth_collections().await?;
        auth_service.update_auth_collections(&auth_collections);

        // Create permission service
        let permission_service = Arc::new(DatabasePermissionService::new(
            Arc::clone(&database) as Arc<dyn oxide_db::Db>
        ));

        // Register system hooks
        register_system_hooks(
            event_bus.as_ref(),
            Arc::clone(&auth_service),
            Arc::clone(&permission_service) as Arc<dyn oxide_core::auth::PermissionService>,
        )
        .await?;

        // Create plugin config service
        let plugin_config_service = PluginConfigService::new(
            Arc::clone(&database) as Arc<dyn oxide_db::Db>,
            args.plugin_folder.clone(),
        );

        // Initialize plugin runtime - simplified for CLI operations
        let security_policies = args.security_policy.clone().into();
        let _plugin_manager = PluginManager::new(
            Arc::clone(&database) as Arc<dyn oxide_db::Db>,
            security_policies,
        )?;

        Ok(PluginManagementServices {
            plugin_config_service,
        })
    }

    /// Resolve database path
    fn resolve_database_path(db_path: &Path) -> Result<String> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::internal(format!("Failed to create database directory: {}", e))
            })?;
        }

        let database_path = if db_path.is_dir() {
            db_path.join("oxidedb.sqlite").to_string_lossy().to_string()
        } else {
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    AppError::internal(format!("Failed to create database directory: {}", e))
                })?;
            }
            db_path.to_string_lossy().to_string()
        };

        Ok(database_path)
    }

    /// List all installed plugins
    async fn list_plugins(args: &ManagePluginsArgs) -> Result<()> {
        info!("📋 Listing installed plugins");
        let services = Self::initialize_services(args).await?;

        let plugin_configs = services.plugin_config_service.list_plugin_configs().await?;

        if plugin_configs.is_empty() {
            println!("No plugins installed.");
            return Ok(());
        }

        println!("\n📦 Installed Plugins:");
        println!("{:-<80}", "");

        for config in plugin_configs {
            let status_icon = match config.status {
                oxide_core::plugin_config::PluginStatus::Enabled => "✅",
                oxide_core::plugin_config::PluginStatus::Disabled => "⏸️",
                oxide_core::plugin_config::PluginStatus::Error => "❌",
                oxide_core::plugin_config::PluginStatus::Loading => "⏳",
                oxide_core::plugin_config::PluginStatus::Uninstalling => "🗑️",
            };

            println!("{} {} (v{})", status_icon, config.name, config.version);
            println!("   Author: {}", config.author);
            println!("   Description: {}", config.description);
            println!("   Trust Level: {:?}", config.trust_level);
            println!("   Capabilities: {}", config.capabilities.len());
            println!();
        }

        Ok(())
    }

    /// Install a plugin from ZIP package
    async fn install_plugin(
        args: &ManagePluginsArgs,
        install_args: &InstallPluginArgs,
    ) -> Result<()> {
        info!("📦 Installing plugin from: {:?}", install_args.package_path);

        if !install_args.package_path.exists() {
            return Err(AppError::validation(
                "package_path",
                "Plugin package file does not exist",
            ));
        }

        let services = Self::initialize_services(args).await?;

        // Read and analyze the plugin package
        let package_data = fs::read(&install_args.package_path)
            .map_err(|e| AppError::internal(format!("Failed to read plugin package: {}", e)))?;

        let package = Self::extract_plugin_package(package_data)?;

        info!("📋 Plugin Analysis:");
        println!("   Name: {}", package.name);
        println!("   Version: {}", package.version);
        println!("   Author: {}", package.author);
        println!("   Description: {}", package.description);
        println!(
            "   Required Capabilities: {:?}",
            package.required_capabilities
        );

        // Check if plugin already exists
        if services
            .plugin_config_service
            .plugin_exists(&package.name)
            .await?
            && !install_args.force
        {
            return Err(AppError::conflict(format!(
                "Plugin '{}' already exists. Use --force to overwrite.",
                package.name
            )));
        }

        // Grant capabilities
        let final_capabilities = if install_args.auto_grant_capabilities {
            package.required_capabilities.clone()
        } else {
            // Interactive capability granting would go here
            // For now, we'll use the required capabilities
            package.required_capabilities.clone()
        };

        // Install the plugin
        let _record_id = services
            .plugin_config_service
            .install_plugin_from_directory(
                package.name.clone(),
                package.version.clone(),
                package.description.clone(),
                package.author.clone(),
                install_args.trust_level.clone().into(),
                final_capabilities,
                ResourceLimits::default(),
                package.extraction_path,
                package.wasm_filename,
                Some(serde_json::json!({
                    "source": "cli_install",
                    "package_path": install_args.package_path.display().to_string()
                })),
            )
            .await?;

        info!("✅ Plugin '{}' installed successfully", package.name);
        Ok(())
    }

    /// Enable a plugin
    async fn enable_plugin(args: &ManagePluginsArgs, name_args: &PluginNameArgs) -> Result<()> {
        info!("🟢 Enabling plugin: {}", name_args.plugin_name);
        let services = Self::initialize_services(args).await?;

        services
            .plugin_config_service
            .enable_plugin(&name_args.plugin_name)
            .await?;
        info!("✅ Plugin '{}' enabled successfully", name_args.plugin_name);
        Ok(())
    }

    /// Disable a plugin
    async fn disable_plugin(args: &ManagePluginsArgs, name_args: &PluginNameArgs) -> Result<()> {
        info!("🔴 Disabling plugin: {}", name_args.plugin_name);
        let services = Self::initialize_services(args).await?;

        services
            .plugin_config_service
            .disable_plugin(&name_args.plugin_name)
            .await?;
        info!(
            "✅ Plugin '{}' disabled successfully",
            name_args.plugin_name
        );
        Ok(())
    }

    /// Uninstall a plugin
    async fn uninstall_plugin(args: &ManagePluginsArgs, name_args: &PluginNameArgs) -> Result<()> {
        info!("🗑️ Uninstalling plugin: {}", name_args.plugin_name);
        let services = Self::initialize_services(args).await?;

        services
            .plugin_config_service
            .uninstall_plugin(&name_args.plugin_name)
            .await?;
        info!(
            "✅ Plugin '{}' uninstalled successfully",
            name_args.plugin_name
        );
        Ok(())
    }

    /// Show detailed plugin information
    async fn show_plugin(args: &ManagePluginsArgs, name_args: &PluginNameArgs) -> Result<()> {
        info!("🔍 Showing plugin details: {}", name_args.plugin_name);
        let services = Self::initialize_services(args).await?;

        let config = services
            .plugin_config_service
            .get_plugin_config(&name_args.plugin_name)
            .await?;

        println!("\n📦 Plugin Details: {}", config.name);
        println!("{:-<80}", "");
        println!("Name: {}", config.name);
        println!("Version: {}", config.version);
        println!("Author: {}", config.author);
        println!("Description: {}", config.description);
        println!("Status: {:?}", config.status);
        println!("Trust Level: {:?}", config.trust_level);
        println!("Enabled: {}", config.enabled);
        println!(
            "Installed At: {}",
            chrono::DateTime::from_timestamp(config.installed_at, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                .unwrap_or_else(|| "Unknown".to_string())
        );

        println!("\n🔧 Capabilities ({}):", config.capabilities.len());
        for capability in &config.capabilities {
            println!("  • {:?}", capability);
        }

        println!("\n💾 Resource Limits:");
        println!(
            "  Memory: {} MB",
            config.resource_limits.max_memory / (1024 * 1024)
        );
        println!(
            "  Execution Time: {} ms",
            config.resource_limits.max_execution_time
        );
        println!("  Host Calls: {}", config.resource_limits.max_host_calls);
        println!(
            "  Rate Limit: {} calls/min",
            config.resource_limits.rate_limit
        );

        if let Some(wasm_path) = &config.wasm_path {
            println!("\n📁 Files:");
            println!("  WASM Path: {}", wasm_path);
            if let Some(size) = config.wasm_size {
                println!("  WASM Size: {} bytes", size);
            }
        }

        Ok(())
    }

    /// Analyze a plugin package
    async fn analyze_plugin(
        _args: &ManagePluginsArgs,
        analyze_args: &AnalyzePluginArgs,
    ) -> Result<()> {
        info!(
            "🔍 Analyzing plugin package: {:?}",
            analyze_args.package_path
        );

        if !analyze_args.package_path.exists() {
            return Err(AppError::validation(
                "package_path",
                "Plugin package file does not exist",
            ));
        }

        let package_data = fs::read(&analyze_args.package_path)
            .map_err(|e| AppError::internal(format!("Failed to read plugin package: {}", e)))?;

        let package = Self::extract_plugin_package(package_data)?;

        println!("\n📋 Plugin Package Analysis");
        println!("{:-<80}", "");
        println!("Name: {}", package.name);
        println!("Version: {}", package.version);
        println!("Author: {}", package.author);
        println!("Description: {}", package.description);
        println!("WASM File: {}", package.wasm_filename);
        println!("Package Size: {} bytes", package.package_size);

        println!(
            "\n🔧 Required Capabilities ({}):",
            package.required_capabilities.len()
        );
        for capability in &package.required_capabilities {
            println!("  • {:?}", capability);
        }

        println!("\n🔒 Security Information:");
        println!(
            "  Recommended Trust Level: {:?}",
            package.recommended_trust_level
        );

        println!("\n📁 Package Contents:");
        for file in &package.files {
            println!("  • {}", file);
        }

        Ok(())
    }

    /// Sign a plugin package with an Ed25519 private key.
    async fn sign_plugin(sign_args: &SignPluginArgs) -> Result<()> {
        info!("✍️ Signing plugin package: {:?}", sign_args.package_path);

        if !sign_args.package_path.exists() {
            return Err(AppError::validation(
                "package_path",
                "Plugin package file does not exist",
            ));
        }

        let package_data = fs::read(&sign_args.package_path)
            .map_err(|e| AppError::internal(format!("Failed to read plugin package: {}", e)))?;
        let package = Self::extract_plugin_signing_parts(&package_data)?;
        let signing_key = Self::load_plugin_signing_key(sign_args)?;
        let payload = Self::build_plugin_signature_payload(
            &package.manifest_data,
            package.wasm_path.as_bytes(),
            &package.wasm_data,
        );
        let signature = signing_key.sign(&payload);
        let output_path = Self::signed_plugin_output_path(sign_args);

        if output_path == sign_args.package_path {
            return Err(AppError::validation(
                "output",
                "Signed plugin output path must differ from the input package path",
            ));
        }

        if output_path.exists() && !sign_args.force {
            return Err(AppError::validation(
                "output",
                "Output file already exists; pass --force to overwrite it",
            ));
        }

        if let Some(parent) = output_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|e| {
                    AppError::internal(format!("Failed to create output directory: {}", e))
                })?;
            }
        }

        Self::write_signed_plugin_package(&output_path, &package.files, &signature.to_bytes())?;

        println!("\n✅ Signed plugin package");
        println!("Input: {}", sign_args.package_path.display());
        println!("Output: {}", output_path.display());
        println!(
            "Trusted public key: {}",
            STANDARD.encode(signing_key.verifying_key().to_bytes())
        );
        println!(
            "Configure this key with OXIDEDB_PLUGIN_TRUSTED_KEYS or OXIDEDB_PLUGIN_TRUSTED_KEYS_FILE before enforcing strict signing."
        );

        Ok(())
    }

    fn extract_plugin_signing_parts(package_data: &[u8]) -> Result<PluginSigningPackage> {
        let cursor = Cursor::new(package_data);
        let mut archive = ZipArchive::new(cursor)
            .map_err(|e| AppError::internal(format!("Failed to read ZIP archive: {}", e)))?;
        let mut files = Vec::new();
        let mut manifest_data = None;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| {
                AppError::internal(format!("Failed to read file from archive: {}", e))
            })?;

            let Some(safe_path) = Self::sanitize_plugin_zip_entry_path(file.name())? else {
                continue;
            };
            let normalized_name = Self::normalized_plugin_relative_path(&safe_path);

            if matches!(
                normalized_name.as_str(),
                "signature" | PLUGIN_SIGNATURE_FILE_NAME
            ) {
                continue;
            }

            let mut data = Vec::new();
            file.read_to_end(&mut data).map_err(|e| {
                AppError::internal(format!("Failed to read plugin package file: {}", e))
            })?;

            if normalized_name == "plugin.toml" {
                manifest_data = Some(data.clone());
            }

            files.push(PluginPackageFile {
                name: normalized_name,
                data,
            });
        }

        let manifest_data = manifest_data
            .ok_or_else(|| AppError::validation("package", "plugin.toml not found in package"))?;
        let manifest_text = std::str::from_utf8(&manifest_data).map_err(|e| {
            AppError::validation("manifest", &format!("plugin.toml must be UTF-8: {}", e))
        })?;
        let manifest: toml::Value = toml::from_str(manifest_text)
            .map_err(|e| AppError::internal(format!("Failed to parse plugin.toml: {}", e)))?;
        let wasm_file = manifest
            .get("plugin")
            .and_then(|section| section.get("wasm_file"))
            .and_then(|value| value.as_str())
            .ok_or_else(|| AppError::validation("manifest", "plugin.wasm_file not found"))?;
        let wasm_path = Self::sanitize_plugin_zip_entry_path(wasm_file)?
            .ok_or_else(|| AppError::validation("manifest", "plugin.wasm_file must be a file"))?;
        let wasm_path = Self::normalized_plugin_relative_path(&wasm_path);
        let wasm_data = files
            .iter()
            .find(|file| file.name == wasm_path)
            .map(|file| file.data.clone())
            .ok_or_else(|| {
                let available_wasm_files = files
                    .iter()
                    .filter(|file| file.name.ends_with(".wasm"))
                    .map(|file| file.name.clone())
                    .collect::<Vec<_>>();
                AppError::validation(
                    "manifest",
                    &format!(
                        "plugin.wasm_file '{}' was not found in package; available WASM files: {:?}",
                        wasm_file, available_wasm_files
                    ),
                )
            })?;

        Ok(PluginSigningPackage {
            manifest_data,
            wasm_path,
            wasm_data,
            files,
        })
    }

    fn signed_plugin_output_path(sign_args: &SignPluginArgs) -> PathBuf {
        if let Some(output) = &sign_args.output {
            return output.clone();
        }

        let stem = sign_args
            .package_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("plugin");
        let extension = sign_args
            .package_path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("zip");

        sign_args
            .package_path
            .with_file_name(format!("{stem}-signed.{extension}"))
    }

    fn load_plugin_signing_key(sign_args: &SignPluginArgs) -> Result<SigningKey> {
        let raw_material = if let Some(private_key) = &sign_args.private_key {
            private_key.clone()
        } else if let Some(key_file) = &sign_args.key_file {
            fs::read_to_string(key_file).map_err(|e| {
                AppError::internal(format!(
                    "Failed to read plugin signing key file '{}': {}",
                    key_file.display(),
                    e
                ))
            })?
        } else if let Ok(private_key) = std::env::var(PLUGIN_SIGNING_KEY_ENV) {
            private_key
        } else {
            return Err(AppError::validation(
                "private_key",
                "Provide --private-key, --key-file, or OXIDEDB_PLUGIN_SIGNING_KEY",
            ));
        };

        let key_material = Self::extract_key_material(&raw_material)
            .ok_or_else(|| AppError::validation("private_key", "Signing key material was empty"))?;
        let key_bytes = Self::decode_key_material(&key_material).map_err(|e| {
            AppError::validation(
                "private_key",
                &format!("Invalid signing key encoding: {}", e),
            )
        })?;

        match key_bytes.len() {
            32 => {
                let seed: [u8; 32] = key_bytes.try_into().map_err(|_| {
                    AppError::validation("private_key", "Invalid Ed25519 seed length")
                })?;
                Ok(SigningKey::from_bytes(&seed))
            }
            64 => {
                let keypair: [u8; 64] = key_bytes.try_into().map_err(|_| {
                    AppError::validation("private_key", "Invalid Ed25519 keypair length")
                })?;
                SigningKey::from_keypair_bytes(&keypair).map_err(|e| {
                    AppError::validation(
                        "private_key",
                        &format!("Invalid Ed25519 keypair bytes: {}", e),
                    )
                })
            }
            length => Err(AppError::validation(
                "private_key",
                &format!(
                    "Invalid Ed25519 private key length: expected 32-byte seed or 64-byte keypair, got {} bytes",
                    length
                ),
            )),
        }
    }

    fn write_signed_plugin_package(
        output_path: &Path,
        files: &[PluginPackageFile],
        signature: &[u8; 64],
    ) -> Result<()> {
        let output_file = fs::File::create(output_path).map_err(|e| {
            AppError::internal(format!(
                "Failed to create signed plugin package '{}': {}",
                output_path.display(),
                e
            ))
        })?;
        let mut writer = ZipWriter::new(output_file);
        let file_options =
            FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        for file in files {
            if matches!(file.name.as_str(), "signature" | PLUGIN_SIGNATURE_FILE_NAME) {
                continue;
            }

            writer.start_file(&file.name, file_options).map_err(|e| {
                AppError::internal(format!("Failed to write plugin ZIP entry: {}", e))
            })?;
            writer.write_all(&file.data).map_err(|e| {
                AppError::internal(format!("Failed to write plugin ZIP entry data: {}", e))
            })?;
        }

        let signature_document = serde_json::json!({
            "algorithm": "ed25519",
            "payload": "OxideDB plugin package signature v1",
            "signature": STANDARD.encode(signature),
        });
        let signature_bytes = serde_json::to_vec_pretty(&signature_document)
            .map_err(|e| AppError::internal(format!("Failed to encode plugin signature: {}", e)))?;

        writer
            .start_file(PLUGIN_SIGNATURE_FILE_NAME, file_options)
            .map_err(|e| {
                AppError::internal(format!("Failed to write plugin signature entry: {}", e))
            })?;
        writer.write_all(&signature_bytes).map_err(|e| {
            AppError::internal(format!("Failed to write plugin signature data: {}", e))
        })?;
        writer
            .finish()
            .map_err(|e| AppError::internal(format!("Failed to finish signed ZIP: {}", e)))?;

        Ok(())
    }

    fn build_plugin_signature_payload(
        manifest_data: &[u8],
        wasm_path: &[u8],
        wasm_data: &[u8],
    ) -> Vec<u8> {
        let mut payload = Vec::with_capacity(
            PLUGIN_SIGNATURE_PAYLOAD_MAGIC.len()
                + std::mem::size_of::<u64>() * 3
                + manifest_data.len()
                + wasm_path.len()
                + wasm_data.len(),
        );

        payload.extend_from_slice(PLUGIN_SIGNATURE_PAYLOAD_MAGIC);
        Self::append_len_prefixed_bytes(&mut payload, manifest_data);
        Self::append_len_prefixed_bytes(&mut payload, wasm_path);
        Self::append_len_prefixed_bytes(&mut payload, wasm_data);
        payload
    }

    fn append_len_prefixed_bytes(payload: &mut Vec<u8>, bytes: &[u8]) {
        payload.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        payload.extend_from_slice(bytes);
    }

    fn sanitize_plugin_zip_entry_path(file_name: &str) -> Result<Option<PathBuf>> {
        if file_name.ends_with('/') || file_name.ends_with('\\') {
            return Ok(None);
        }

        let mut relative_path = PathBuf::new();
        let mut has_components = false;

        for component in Path::new(file_name).components() {
            match component {
                Component::Normal(part) => {
                    if part.to_string_lossy().starts_with('.') {
                        return Ok(None);
                    }
                    relative_path.push(part);
                    has_components = true;
                }
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(AppError::validation(
                        "package",
                        &format!("Unsafe path in plugin ZIP entry: {}", file_name),
                    ));
                }
            }
        }

        if has_components {
            Ok(Some(relative_path))
        } else {
            Ok(None)
        }
    }

    fn normalized_plugin_relative_path(path: &Path) -> String {
        path.components()
            .filter_map(|component| match component {
                Component::Normal(part) => Some(part.to_string_lossy().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    fn extract_key_material(value: &str) -> Option<String> {
        for line in value.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some((name, material)) = trimmed.split_once('=') {
                let name = name.trim().to_ascii_lowercase();
                if matches!(
                    name.as_str(),
                    "private_key" | "signing_key" | "key" | "ed25519"
                ) {
                    return Some(material.trim().to_string());
                }
            }

            return Some(trimmed.to_string());
        }

        None
    }

    fn decode_key_material(material: &str) -> std::result::Result<Vec<u8>, String> {
        let mut material = material.trim().trim_matches('"').trim_matches('\'');

        for prefix in ["ed25519:", "base64:", "hex:"] {
            if let Some(stripped) = material.strip_prefix(prefix) {
                material = stripped.trim();
                break;
            }
        }

        if material.len().is_multiple_of(2) && material.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Self::decode_hex_material(material);
        }

        STANDARD.decode(material).map_err(|e| e.to_string())
    }

    fn decode_hex_material(material: &str) -> std::result::Result<Vec<u8>, String> {
        let mut bytes = Vec::with_capacity(material.len() / 2);
        let mut chars = material.as_bytes().chunks_exact(2);

        for pair in &mut chars {
            let high = Self::hex_value(pair[0])?;
            let low = Self::hex_value(pair[1])?;
            bytes.push((high << 4) | low);
        }

        if !chars.remainder().is_empty() {
            return Err("hex input must contain an even number of digits".to_string());
        }

        Ok(bytes)
    }

    fn hex_value(byte: u8) -> std::result::Result<u8, String> {
        match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            b'A'..=b'F' => Ok(byte - b'A' + 10),
            _ => Err("hex input contains a non-hex digit".to_string()),
        }
    }

    /// Extract and analyze plugin package
    fn extract_plugin_package(package_data: Vec<u8>) -> Result<PluginPackageInfo> {
        let cursor = Cursor::new(&package_data);
        let mut archive = ZipArchive::new(cursor)
            .map_err(|e| AppError::internal(format!("Failed to read ZIP archive: {}", e)))?;

        // Create temporary extraction directory
        let temp_dir = std::env::temp_dir().join(format!(
            "oxidedb_plugin_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ));
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| AppError::internal(format!("Failed to create temp directory: {}", e)))?;

        let mut files = Vec::new();
        let mut manifest_content = None;

        // Extract all files
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| {
                AppError::internal(format!("Failed to read file from archive: {}", e))
            })?;

            let Some(safe_path) = Self::sanitize_plugin_zip_entry_path(file.name())? else {
                continue;
            };
            let filename = Self::normalized_plugin_relative_path(&safe_path);
            files.push(filename.clone());

            let mut file_data = Vec::new();
            file.read_to_end(&mut file_data)
                .map_err(|e| AppError::internal(format!("Failed to read archive file: {}", e)))?;

            if filename == "plugin.toml" {
                let content = String::from_utf8(file_data.clone())
                    .map_err(|e| AppError::internal(format!("plugin.toml must be UTF-8: {}", e)))?;
                manifest_content = Some(content);
            }

            // Extract file to temp directory
            let outpath = temp_dir.join(&safe_path);
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    AppError::internal(format!("Failed to create directory: {}", e))
                })?;
            }

            let mut outfile = std::fs::File::create(&outpath)
                .map_err(|e| AppError::internal(format!("Failed to create file: {}", e)))?;

            outfile
                .write_all(&file_data)
                .map_err(|e| AppError::internal(format!("Failed to extract file: {}", e)))?;
        }

        // Parse manifest
        let manifest_str = manifest_content
            .ok_or_else(|| AppError::validation("package", "plugin.toml not found in package"))?;

        let manifest: toml::Value = toml::from_str(&manifest_str)
            .map_err(|e| AppError::internal(format!("Failed to parse plugin.toml: {}", e)))?;

        let plugin_section = manifest.get("plugin").ok_or_else(|| {
            AppError::validation("manifest", "[plugin] section not found in plugin.toml")
        })?;

        let security_section = manifest.get("security").ok_or_else(|| {
            AppError::validation("manifest", "[security] section not found in plugin.toml")
        })?;

        let name = plugin_section
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::validation("manifest", "plugin.name not found"))?
            .to_string();

        let version = plugin_section
            .get("version")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::validation("manifest", "plugin.version not found"))?
            .to_string();

        let description = plugin_section
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("No description")
            .to_string();

        let author = plugin_section
            .get("author")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        let wasm_file = plugin_section
            .get("wasm_file")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::validation("manifest", "plugin.wasm_file not found"))?
            .to_string();

        // Parse capabilities
        let required_capabilities: Vec<PluginCapability> = security_section
            .get("required_capabilities")
            .and_then(|v| v.as_array())
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|v| v.as_str())
            .filter_map(|s| Self::parse_capability_string(s).ok())
            .collect();

        let recommended_trust_level = security_section
            .get("recommended_trust_level")
            .and_then(|v| v.as_str())
            .map(|s| Self::parse_trust_level_string(s).unwrap_or(PluginTrustLevel::Untrusted))
            .unwrap_or(PluginTrustLevel::Untrusted);

        Ok(PluginPackageInfo {
            name,
            version,
            description,
            author,
            wasm_filename: wasm_file,
            package_size: package_data.len(),
            required_capabilities,
            recommended_trust_level,
            files,
            extraction_path: temp_dir,
        })
    }

    /// Parse capability string to PluginCapability enum
    fn parse_capability_string(capability: &str) -> Result<PluginCapability> {
        match capability {
            "LogInfo" => Ok(PluginCapability::LogInfo),
            "LogError" => Ok(PluginCapability::LogError),
            "ReadEventData" => Ok(PluginCapability::ReadEventData),
            "ModifyEventData" => Ok(PluginCapability::ModifyEventData),
            "CreateRecords" => Ok(PluginCapability::CreateRecords {
                collections: vec!["*".to_string()],
            }),
            "ReadRecords" => Ok(PluginCapability::ReadRecords {
                collections: vec!["*".to_string()],
            }),
            "UpdateRecords" => Ok(PluginCapability::UpdateRecords {
                collections: vec!["*".to_string()],
            }),
            "DeleteRecords" => Ok(PluginCapability::DeleteRecords {
                collections: vec!["*".to_string()],
            }),
            "RegisterHttpRoutes" => Ok(PluginCapability::RegisterHttpRoutes {
                path_patterns: vec![".*".to_string()],
                methods: vec!["GET".to_string(), "POST".to_string()],
            }),
            "HandleHttpRequests" => Ok(PluginCapability::HandleHttpRequests),
            "BlockOperations" => Ok(PluginCapability::BlockOperations),
            "AccessVfs" => Ok(PluginCapability::AccessVfs {
                namespaces: vec!["*".to_string()],
                operations: vec![VfsOperation::Read, VfsOperation::List, VfsOperation::Usage],
            }),
            "HttpRequest" => Ok(PluginCapability::HttpRequest {
                allowed_urls: vec![".*".to_string()],
                rate_limit: 60,
            }),
            _ => Err(AppError::validation(
                "capability",
                &format!("Unknown capability: {}", capability),
            )),
        }
    }

    /// Parse trust level string
    fn parse_trust_level_string(trust_level: &str) -> Result<PluginTrustLevel> {
        match trust_level.to_lowercase().as_str() {
            "untrusted" => Ok(PluginTrustLevel::Untrusted),
            "partiallytrusted" | "partially_trusted" => Ok(PluginTrustLevel::PartiallyTrusted),
            "fullytrusted" | "fully_trusted" => Ok(PluginTrustLevel::FullyTrusted),
            "system" => Ok(PluginTrustLevel::System),
            _ => Err(AppError::validation(
                "trust_level",
                &format!("Unknown trust level: {}", trust_level),
            )),
        }
    }
}

/// Services needed for plugin management
struct PluginManagementServices {
    plugin_config_service: PluginConfigService,
}

struct PluginSigningPackage {
    manifest_data: Vec<u8>,
    wasm_path: String,
    wasm_data: Vec<u8>,
    files: Vec<PluginPackageFile>,
}

struct PluginPackageFile {
    name: String,
    data: Vec<u8>,
}

/// Information extracted from a plugin package
struct PluginPackageInfo {
    name: String,
    version: String,
    description: String,
    author: String,
    wasm_filename: String,
    package_size: usize,
    required_capabilities: Vec<PluginCapability>,
    recommended_trust_level: PluginTrustLevel,
    files: Vec<String>,
    extraction_path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_resolve_database_path_file() {
        let result = RegisterSuperuserCommand::resolve_database_path(&PathBuf::from("test.db"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test.db");
    }

    #[test]
    fn test_resolve_database_path_directory() {
        let temp_dir = std::env::temp_dir().join("oxidedb_test");
        std::fs::create_dir_all(&temp_dir).unwrap();

        let result = RegisterSuperuserCommand::resolve_database_path(&temp_dir);
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("oxidedb.sqlite"));

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn plugin_signature_payload_is_length_prefixed() {
        let payload = ManagePluginsCommand::build_plugin_signature_payload(
            b"manifest",
            b"plugin.wasm",
            b"wasm",
        );

        let mut expected = Vec::new();
        expected.extend_from_slice(PLUGIN_SIGNATURE_PAYLOAD_MAGIC);
        expected.extend_from_slice(&(8_u64).to_be_bytes());
        expected.extend_from_slice(b"manifest");
        expected.extend_from_slice(&(11_u64).to_be_bytes());
        expected.extend_from_slice(b"plugin.wasm");
        expected.extend_from_slice(&(4_u64).to_be_bytes());
        expected.extend_from_slice(b"wasm");

        assert_eq!(payload, expected);
    }

    #[test]
    fn plugin_signing_key_accepts_hex_seed() {
        let args = SignPluginArgs {
            package_path: PathBuf::from("plugin.zip"),
            private_key: Some(
                "hex:000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f".to_string(),
            ),
            key_file: None,
            output: None,
            force: false,
        };

        let signing_key = ManagePluginsCommand::load_plugin_signing_key(&args).unwrap();

        assert_eq!(signing_key.to_bytes().len(), 32);
    }

    #[test]
    fn license_issue_emits_verifiable_document() {
        use ed25519_dalek::{Signature, Verifier};

        let signing_key = SigningKey::from_bytes(&[9_u8; 32]);
        let expires_at = current_unix_timestamp() + 3600;
        let args = IssueLicenseArgs {
            edition: LicenseEdition::Professional,
            installation_id: Some("installation-1".to_string()),
            expires_at,
            not_before: None,
            private_key: Some(STANDARD.encode(signing_key.to_bytes())),
            key_file: None,
            output: None,
        };

        let document = LicenseCommand::issue_license_document(&args).unwrap();
        let document: serde_json::Value = serde_json::from_str(&document).unwrap();
        let payload = URL_SAFE_NO_PAD
            .decode(document["payload"].as_str().unwrap())
            .unwrap();
        let signature_bytes: [u8; 64] = URL_SAFE_NO_PAD
            .decode(document["signature"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let signature = Signature::from_bytes(&signature_bytes);

        signing_key
            .verifying_key()
            .verify(&payload, &signature)
            .unwrap();

        let claims: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(claims["edition"], "professional");
        assert_eq!(claims["installation_id"], "installation-1");
        assert_eq!(claims["expires_at"], expires_at);
    }
}
