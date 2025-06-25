//! Command Handlers
//!
//! This module contains the implementation of CLI commands for OxideDB,
//! providing clean separation between CLI parsing and business logic.

use crate::{
    config::{
        RegisterSuperuserArgs, StartArgs, ManagePluginsArgs, PluginSubcommands,
        InstallPluginArgs, PluginNameArgs, AnalyzePluginArgs,
    }, 
    startup::ApplicationBootstrap, 
    OxideDbConfig, 
    Result
};
use oxide_api::services::{DatabasePermissionService, PluginConfigService};
use oxide_core::{
    AppError, AuthService, EventBus, InMemoryEventBus, register_system_hooks,
    plugin_security::{PluginCapability, PluginTrustLevel, ResourceLimits},
};
use oxide_db::SqliteDb;
use oxide_plugin_runtime::PluginManager;
use std::sync::Arc;
use tracing::{error, info};
use std::path::PathBuf;
use std::fs;
use zip::ZipArchive;
use std::io::{Read, Cursor};

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
            Arc::clone(&permission_service) as Arc<dyn oxide_core::auth::PermissionService>
        ).await?;
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
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::internal(format!("Failed to create database directory: {}", e)))?;
        }

        let database_path = if db_path.is_dir() {
            db_path.join("oxidedb.sqlite").to_string_lossy().to_string()
        } else {
            // Ensure parent directory exists for file path
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| AppError::internal(format!("Failed to create database directory: {}", e)))?;
            }
            db_path.to_string_lossy().to_string()
        };

        Ok(database_path)
    }

    /// Register the superuser with the given arguments
    async fn register_superuser(args: &RegisterSuperuserArgs, services: &MinimalServices) -> Result<()> {
        // Find superuser collection or use default
        let auth_collections = services.database.list_auth_collections().await?;
        let superuser_collection = auth_collections.iter()
            .find(|c| c.name == "_superusers")
            .map(|c| c.name.as_str())
            .unwrap_or("_users");

        if let Some(superuser_config) = services.auth_service.config().get_auth_collection(superuser_collection) {
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

            match services.database.register_user(register_request, &superuser_config).await {
                Ok(user_id) => {
                    info!("✅ Superuser registered successfully with ID: {} in collection '{}'", 
                          user_id, superuser_collection);
                }
                Err(e) => {
                    error!("❌ Failed to register superuser: {}", e);
                    return Err(e);
                }
            }
        } else {
            return Err(AppError::internal(format!("Auth collection '{}' not found", superuser_collection)));
        }

        Ok(())
    }
}

/// Minimal services needed for superuser registration
struct MinimalServices {
    database: Arc<SqliteDb>,
    auth_service: Arc<AuthService>,
}

/// Handler for the plugin management command
pub struct ManagePluginsCommand;

impl CommandHandler for ManagePluginsCommand {
    type Args = ManagePluginsArgs;

    async fn execute(args: Self::Args) -> Result<()> {
        match args.command {
            PluginSubcommands::List => Self::list_plugins(&args).await,
            PluginSubcommands::Install(ref install_args) => Self::install_plugin(&args, install_args).await,
            PluginSubcommands::Enable(ref name_args) => Self::enable_plugin(&args, name_args).await,
            PluginSubcommands::Disable(ref name_args) => Self::disable_plugin(&args, name_args).await,
            PluginSubcommands::Uninstall(ref name_args) => Self::uninstall_plugin(&args, name_args).await,
            PluginSubcommands::Show(ref name_args) => Self::show_plugin(&args, name_args).await,
            PluginSubcommands::Analyze(ref analyze_args) => Self::analyze_plugin(&args, analyze_args).await,
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
            Arc::clone(&permission_service) as Arc<dyn oxide_core::auth::PermissionService>
        ).await?;

        // Create plugin config service
        let plugin_config_service = PluginConfigService::new(
            Arc::clone(&database) as Arc<dyn oxide_db::Db>,
            args.plugin_folder.clone(),
        );

        // Initialize plugin runtime - simplified for CLI operations
        let security_policies = args.security_policy.clone().into();
        let plugin_manager = PluginManager::new(
            Arc::clone(&database) as Arc<dyn oxide_db::Db>,
            security_policies,
        )?;

        Ok(PluginManagementServices {
            database,
            plugin_config_service,
            plugin_manager: Arc::new(plugin_manager),
        })
    }

    /// Resolve database path
    fn resolve_database_path(db_path: &PathBuf) -> Result<String> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::internal(format!("Failed to create database directory: {}", e)))?;
        }

        let database_path = if db_path.is_dir() {
            db_path.join("oxidedb.sqlite").to_string_lossy().to_string()
        } else {
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| AppError::internal(format!("Failed to create database directory: {}", e)))?;
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
    async fn install_plugin(args: &ManagePluginsArgs, install_args: &InstallPluginArgs) -> Result<()> {
        info!("📦 Installing plugin from: {:?}", install_args.package_path);
        
        if !install_args.package_path.exists() {
            return Err(AppError::validation("package_path", "Plugin package file does not exist"));
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
        println!("   Required Capabilities: {:?}", package.required_capabilities);
        
        // Check if plugin already exists
        if services.plugin_config_service.plugin_exists(&package.name).await? && !install_args.force {
            return Err(AppError::conflict(format!("Plugin '{}' already exists. Use --force to overwrite.", package.name)));
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
        let _record_id = services.plugin_config_service.install_plugin_from_directory(
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
        ).await?;

        info!("✅ Plugin '{}' installed successfully", package.name);
        Ok(())
    }

    /// Enable a plugin
    async fn enable_plugin(args: &ManagePluginsArgs, name_args: &PluginNameArgs) -> Result<()> {
        info!("🟢 Enabling plugin: {}", name_args.plugin_name);
        let services = Self::initialize_services(args).await?;
        
        services.plugin_config_service.enable_plugin(&name_args.plugin_name).await?;
        info!("✅ Plugin '{}' enabled successfully", name_args.plugin_name);
        Ok(())
    }

    /// Disable a plugin
    async fn disable_plugin(args: &ManagePluginsArgs, name_args: &PluginNameArgs) -> Result<()> {
        info!("🔴 Disabling plugin: {}", name_args.plugin_name);
        let services = Self::initialize_services(args).await?;
        
        services.plugin_config_service.disable_plugin(&name_args.plugin_name).await?;
        info!("✅ Plugin '{}' disabled successfully", name_args.plugin_name);
        Ok(())
    }

    /// Uninstall a plugin
    async fn uninstall_plugin(args: &ManagePluginsArgs, name_args: &PluginNameArgs) -> Result<()> {
        info!("🗑️ Uninstalling plugin: {}", name_args.plugin_name);
        let services = Self::initialize_services(args).await?;
        
        services.plugin_config_service.uninstall_plugin(&name_args.plugin_name).await?;
        info!("✅ Plugin '{}' uninstalled successfully", name_args.plugin_name);
        Ok(())
    }

    /// Show detailed plugin information
    async fn show_plugin(args: &ManagePluginsArgs, name_args: &PluginNameArgs) -> Result<()> {
        info!("🔍 Showing plugin details: {}", name_args.plugin_name);
        let services = Self::initialize_services(args).await?;
        
        let config = services.plugin_config_service.get_plugin_config(&name_args.plugin_name).await?;
        
        println!("\n📦 Plugin Details: {}", config.name);
        println!("{:-<80}", "");
        println!("Name: {}", config.name);
        println!("Version: {}", config.version);
        println!("Author: {}", config.author);
        println!("Description: {}", config.description);
        println!("Status: {:?}", config.status);
        println!("Trust Level: {:?}", config.trust_level);
        println!("Enabled: {}", config.enabled);
        println!("Installed At: {}", chrono::DateTime::from_timestamp(config.installed_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| "Unknown".to_string()));
        
        println!("\n🔧 Capabilities ({}):", config.capabilities.len());
        for capability in &config.capabilities {
            println!("  • {:?}", capability);
        }
        
        println!("\n💾 Resource Limits:");
        println!("  Memory: {} MB", config.resource_limits.max_memory / (1024 * 1024));
        println!("  Execution Time: {} ms", config.resource_limits.max_execution_time);
        println!("  Host Calls: {}", config.resource_limits.max_host_calls);
        println!("  Rate Limit: {} calls/min", config.resource_limits.rate_limit);
        
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
    async fn analyze_plugin(_args: &ManagePluginsArgs, analyze_args: &AnalyzePluginArgs) -> Result<()> {
        info!("🔍 Analyzing plugin package: {:?}", analyze_args.package_path);
        
        if !analyze_args.package_path.exists() {
            return Err(AppError::validation("package_path", "Plugin package file does not exist"));
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
        
        println!("\n🔧 Required Capabilities ({}):", package.required_capabilities.len());
        for capability in &package.required_capabilities {
            println!("  • {:?}", capability);
        }
        
        println!("\n🔒 Security Information:");
        println!("  Recommended Trust Level: {:?}", package.recommended_trust_level);
        
        println!("\n📁 Package Contents:");
        for file in &package.files {
            println!("  • {}", file);
        }
        
        Ok(())
    }

    /// Extract and analyze plugin package
    fn extract_plugin_package(package_data: Vec<u8>) -> Result<PluginPackageInfo> {
        let cursor = Cursor::new(&package_data);
        let mut archive = ZipArchive::new(cursor)
            .map_err(|e| AppError::internal(format!("Failed to read ZIP archive: {}", e)))?;
        
        // Create temporary extraction directory
        let temp_dir = std::env::temp_dir().join(format!("oxidedb_plugin_{}", 
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()));
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| AppError::internal(format!("Failed to create temp directory: {}", e)))?;

        let mut files = Vec::new();
        let mut manifest_content = None;

        // Extract all files
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| AppError::internal(format!("Failed to read file from archive: {}", e)))?;
            
            let filename = file.name().to_string();
            files.push(filename.clone());
            
            if filename == "plugin.toml" {
                let mut content = String::new();
                file.read_to_string(&mut content)
                    .map_err(|e| AppError::internal(format!("Failed to read plugin.toml: {}", e)))?;
                manifest_content = Some(content);
            }
            
            // Extract file to temp directory
            let outpath = temp_dir.join(&filename);
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| AppError::internal(format!("Failed to create directory: {}", e)))?;
            }
            
            let mut outfile = std::fs::File::create(&outpath)
                .map_err(|e| AppError::internal(format!("Failed to create file: {}", e)))?;
            
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| AppError::internal(format!("Failed to extract file: {}", e)))?;
        }

        // Parse manifest
        let manifest_str = manifest_content
            .ok_or_else(|| AppError::validation("package", "plugin.toml not found in package"))?;
        
        let manifest: toml::Value = toml::from_str(&manifest_str)
            .map_err(|e| AppError::internal(format!("Failed to parse plugin.toml: {}", e)))?;

        let plugin_section = manifest.get("plugin")
            .ok_or_else(|| AppError::validation("manifest", "[plugin] section not found in plugin.toml"))?;
        
        let security_section = manifest.get("security")
            .ok_or_else(|| AppError::validation("manifest", "[security] section not found in plugin.toml"))?;

        let name = plugin_section.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::validation("manifest", "plugin.name not found"))?
            .to_string();

        let version = plugin_section.get("version")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::validation("manifest", "plugin.version not found"))?
            .to_string();

        let description = plugin_section.get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("No description")
            .to_string();

        let author = plugin_section.get("author")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        let wasm_file = plugin_section.get("wasm_file")
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
                collections: vec!["*".to_string()] 
            }),
            "ReadRecords" => Ok(PluginCapability::ReadRecords { 
                collections: vec!["*".to_string()] 
            }),
            "UpdateRecords" => Ok(PluginCapability::UpdateRecords { 
                collections: vec!["*".to_string()] 
            }),
            "DeleteRecords" => Ok(PluginCapability::DeleteRecords { 
                collections: vec!["*".to_string()] 
            }),
            "RegisterHttpRoutes" => Ok(PluginCapability::RegisterHttpRoutes { 
                path_patterns: vec![".*".to_string()], 
                methods: vec!["GET".to_string(), "POST".to_string()] 
            }),
            "HandleHttpRequests" => Ok(PluginCapability::HandleHttpRequests),
            "BlockOperations" => Ok(PluginCapability::BlockOperations),
            "HttpRequest" => Ok(PluginCapability::HttpRequest { 
                allowed_urls: vec![".*".to_string()], 
                rate_limit: 60 
            }),
            _ => Err(AppError::validation("capability", &format!("Unknown capability: {}", capability))),
        }
    }

    /// Parse trust level string
    fn parse_trust_level_string(trust_level: &str) -> Result<PluginTrustLevel> {
        match trust_level.to_lowercase().as_str() {
            "untrusted" => Ok(PluginTrustLevel::Untrusted),
            "partiallytrusted" | "partially_trusted" => Ok(PluginTrustLevel::PartiallyTrusted),
            "fullytrusted" | "fully_trusted" => Ok(PluginTrustLevel::FullyTrusted),
            "system" => Ok(PluginTrustLevel::System),
            _ => Err(AppError::validation("trust_level", &format!("Unknown trust level: {}", trust_level))),
        }
    }
}

/// Services needed for plugin management
struct PluginManagementServices {
    database: Arc<SqliteDb>,
    plugin_config_service: PluginConfigService,
    plugin_manager: Arc<PluginManager>,
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
} 