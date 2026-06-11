//! Application Configuration
//!
//! This module contains all configuration structures and validation logic
//! for the OxideDB application, providing a single source of truth for
//! application settings.

use clap::{Parser, Subcommand, ValueEnum};
use oxide_core::AppError;
use std::path::PathBuf;
use tracing::Level;

const DEFAULT_JWT_SECRET: &str = "dev_secret_key_change_in_production";
const PRODUCTION_ENV_VARS: [&str; 5] = ["OXIDEDB_ENV", "OXIDE_ENV", "APP_ENV", "ENV", "NODE_ENV"];

/// Main application configuration
#[derive(Debug, Clone)]
pub struct OxideDbConfig {
    /// Server configuration
    pub server: ServerConfig,
    /// Database configuration
    pub database: DatabaseConfig,
    /// Plugin system configuration
    pub plugins: PluginConfig,
    /// Logging configuration
    pub logging: LoggingConfig,
    /// Security configuration
    pub security: SecurityConfig,
}

impl OxideDbConfig {
    /// Create configuration from command line arguments
    pub fn from_start_args(args: &StartArgs) -> Self {
        Self {
            server: ServerConfig {
                bind_address: args.bind_address.clone(),
                api_port: args.api_port,
                admin_path: args.admin_path.clone(),
                enable_admin: args.enable_admin,
                admin_mode: args.admin_mode.clone(),
                admin_ui_path: args.admin_ui_path.clone(),
                enable_cors: args.enable_cors,
                enable_request_logging: args.enable_request_logging,
            },
            database: DatabaseConfig {
                db_path: args.db_path.clone(),
                auto_populate: args.auto_populate_db,
            },
            plugins: PluginConfig {
                plugin_folder: args.plugin_folder.clone(),
                security_policy: args.security_policy.clone(),
            },
            logging: LoggingConfig {
                enable_logging: args.enable_logging,
                logging_db_path: args.logging_db_path.clone(),
                log_level: args.log_level.clone(),
            },
            security: SecurityConfig::default(),
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), AppError> {
        self.server.validate()?;
        self.database.validate()?;
        self.plugins.validate()?;
        self.logging.validate()?;
        self.security.validate()?;
        Ok(())
    }
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind_address: String,
    pub api_port: u16,
    pub admin_path: String,
    pub enable_admin: bool,
    pub admin_mode: AdminMode,
    pub admin_ui_path: PathBuf,
    pub enable_cors: bool,
    pub enable_request_logging: bool,
}

impl ServerConfig {
    fn validate(&self) -> Result<(), AppError> {
        if self.api_port == 0 {
            return Err(AppError::validation("api_port", "API port cannot be 0"));
        }

        if self.bind_address.is_empty() {
            return Err(AppError::validation(
                "bind_address",
                "Bind address cannot be empty",
            ));
        }

        if matches!(self.admin_mode, AdminMode::External) && !self.admin_ui_path.exists() {
            return Err(AppError::validation(
                "admin_ui_path",
                &format!(
                    "External admin UI path {:?} does not exist",
                    self.admin_ui_path
                ),
            ));
        }

        Ok(())
    }
}

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub db_path: PathBuf,
    pub auto_populate: bool,
}

impl DatabaseConfig {
    fn validate(&self) -> Result<(), AppError> {
        // Validate parent directory exists or can be created
        if self.db_path.to_string_lossy() != ":memory:" {
            if let Some(parent) = self.db_path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        AppError::validation(
                            "db_path",
                            &format!("Cannot create database directory {:?}: {}", parent, e),
                        )
                    })?;
                }
            }
        }
        Ok(())
    }

    /// Get the resolved database path
    pub fn resolved_path(&self) -> Result<String, AppError> {
        if self.db_path.to_string_lossy() == ":memory:" {
            return Ok(":memory:".to_string());
        }

        // Ensure parent directory exists
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::internal(format!("Failed to create database directory: {}", e))
            })?;
        }

        // If the path is a directory, append the default database filename
        let resolved_path = if self.db_path.is_dir() {
            self.db_path.join("oxidedb.sqlite")
        } else {
            self.db_path.clone()
        };

        Ok(resolved_path.to_string_lossy().to_string())
    }
}

/// Plugin configuration
#[derive(Debug, Clone)]
pub struct PluginConfig {
    pub plugin_folder: PathBuf,
    pub security_policy: SecurityPolicy,
}

impl PluginConfig {
    fn validate(&self) -> Result<(), AppError> {
        // Plugin folder is optional - it's okay if it doesn't exist
        Ok(())
    }
}

/// Logging configuration
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub enable_logging: bool,
    pub logging_db_path: PathBuf,
    pub log_level: LogLevel,
}

impl LoggingConfig {
    fn validate(&self) -> Result<(), AppError> {
        if self.enable_logging {
            // Ensure logging directory can be created
            if let Some(parent) = self.logging_db_path.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        AppError::validation(
                            "logging_db_path",
                            &format!("Cannot create logging directory {:?}: {}", parent, e),
                        )
                    })?;
                }
            }
        }
        Ok(())
    }
}

/// Security configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub jwt_secret: String,
    pub require_https: bool,
    pub max_request_size: usize,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| DEFAULT_JWT_SECRET.to_string()),
            require_https: false, // Default to false for development
            max_request_size: 16 * 1024 * 1024, // 16MB default
        }
    }
}

impl SecurityConfig {
    fn validate(&self) -> Result<(), AppError> {
        if self.jwt_secret == DEFAULT_JWT_SECRET {
            if is_production_environment() {
                return Err(AppError::validation(
                    "jwt_secret",
                    "JWT_SECRET must be set to a strong non-default value in production",
                ));
            }

            tracing::warn!("Using default JWT secret - change this before production!");
        }

        if self.jwt_secret.len() < 32 {
            return Err(AppError::validation(
                "jwt_secret",
                "JWT secret must be at least 32 characters",
            ));
        }

        Ok(())
    }
}

fn is_production_environment() -> bool {
    PRODUCTION_ENV_VARS.iter().any(|name| {
        std::env::var(name)
            .map(|value| is_production_environment_value(&value))
            .unwrap_or(false)
    })
}

fn is_production_environment_value(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("production")
}

/// Security policy levels for plugin execution
#[derive(Debug, Clone, ValueEnum)]
pub enum SecurityPolicy {
    /// Strict security - only fully trusted plugins allowed
    Strict,
    /// Allow untrusted plugins with limited capabilities
    Untrusted,
    /// Development mode - relaxed security for testing
    Dev,
}

impl From<SecurityPolicy> for oxide_core::plugin_security::SecurityPolicies {
    fn from(policy: SecurityPolicy) -> Self {
        match policy {
            SecurityPolicy::Strict => oxide_core::plugin_security::SecurityPolicies {
                default_trust_level: oxide_core::plugin_security::PluginTrustLevel::FullyTrusted,
                default_resource_limits: oxide_core::plugin_security::ResourceLimits::conservative(
                ),
                max_violations_before_suspension: 3,
                allow_untrusted_plugins: false,
                require_code_signing: true,
            },
            SecurityPolicy::Untrusted => oxide_core::plugin_security::SecurityPolicies {
                default_trust_level: oxide_core::plugin_security::PluginTrustLevel::Untrusted,
                default_resource_limits: oxide_core::plugin_security::ResourceLimits::default(),
                max_violations_before_suspension: 5,
                allow_untrusted_plugins: true,
                require_code_signing: false,
            },
            SecurityPolicy::Dev => oxide_core::plugin_security::SecurityPolicies {
                default_trust_level:
                    oxide_core::plugin_security::PluginTrustLevel::PartiallyTrusted,
                default_resource_limits: oxide_core::plugin_security::ResourceLimits::relaxed(),
                max_violations_before_suspension: 10,
                allow_untrusted_plugins: true,
                require_code_signing: false,
            },
        }
    }
}

/// Log level configuration for the application
#[derive(Debug, Clone, ValueEnum)]
pub enum LogLevel {
    /// Only error messages
    Error,
    /// Warning and error messages
    Warn,
    /// Info, warning, and error messages
    Info,
    /// Debug and above (verbose)
    Debug,
    /// All messages including trace (very verbose)
    Trace,
}

impl From<LogLevel> for Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => Level::ERROR,
            LogLevel::Warn => Level::WARN,
            LogLevel::Info => Level::INFO,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Trace => Level::TRACE,
        }
    }
}

/// Admin UI modes
#[derive(Debug, Clone, ValueEnum)]
pub enum AdminMode {
    /// Use the embedded admin UI (default)
    Embedded,
    /// Serve admin UI from external filesystem path
    External,
    /// Disable admin UI completely
    Disabled,
}

/// CLI argument structures
#[derive(Parser)]
#[command(name = "oxidedb")]
#[command(about = "A hook-first database with plugin architecture")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the OxideDB server
    Start(StartArgs),
    /// Register a superuser account
    RegisterSuperuser(RegisterSuperuserArgs),
    /// Manage plugins (list, install, enable, disable, uninstall)
    ManagePlugins(ManagePluginsArgs),
    /// Issue signed commercial license documents
    License(LicenseArgs),
}

#[derive(Parser)]
pub struct StartArgs {
    /// Database file path
    #[arg(long, default_value = "oxidedb-sqlite")]
    pub db_path: PathBuf,

    /// Security policy for plugin execution
    #[arg(long, value_enum, default_value = "untrusted")]
    pub security_policy: SecurityPolicy,

    /// Plugin folder path
    #[arg(long, default_value = "oxide-plugins")]
    pub plugin_folder: PathBuf,

    /// API server port
    #[arg(long, default_value = "8080")]
    pub api_port: u16,

    /// Admin UI path prefix
    #[arg(long, default_value = "/admin")]
    pub admin_path: String,

    /// Enable admin UI (set to false to disable in production)
    #[arg(long, default_value = "true")]
    pub enable_admin: bool,

    /// Admin UI mode: embedded (use built-in UI) or external (serve from filesystem)
    #[arg(long, value_enum, default_value = "embedded")]
    pub admin_mode: AdminMode,

    /// Path to external admin UI files (only used when admin_mode=external)
    #[arg(long, default_value = "ui/dist")]
    pub admin_ui_path: PathBuf,

    /// Auto-populate database with sample data
    #[arg(long)]
    pub auto_populate_db: bool,

    /// Bind address for the API server
    #[arg(long, default_value = "127.0.0.1")]
    pub bind_address: String,

    /// Log level for the application
    #[arg(long, value_enum, default_value = "info")]
    pub log_level: LogLevel,

    /// Enable request logging
    #[arg(long, default_value = "true")]
    pub enable_request_logging: bool,

    /// Enable CORS (Cross-Origin Resource Sharing). Set OXIDEDB_CORS_ALLOWED_ORIGINS to allow non-localhost origins.
    #[arg(long, default_value = "false")]
    pub enable_cors: bool,

    /// Enable logging system
    #[arg(long, default_value = "true")]
    pub enable_logging: bool,

    /// Logging database path
    #[arg(long, default_value = "logs")]
    pub logging_db_path: PathBuf,
}

#[derive(Parser)]
pub struct RegisterSuperuserArgs {
    /// Database file path
    #[arg(long, default_value = "oxidedb-sqlite")]
    pub db_path: PathBuf,

    /// Email address for the superuser
    #[arg(long)]
    pub email: String,

    /// Password for the superuser
    #[arg(long)]
    pub password: String,

    /// Full name for the superuser
    #[arg(long)]
    pub name: Option<String>,

    /// Log level for the operation
    #[arg(long, value_enum, default_value = "info")]
    pub log_level: LogLevel,
}

/// License management command arguments
#[derive(Parser)]
pub struct LicenseArgs {
    /// Log level for the operation
    #[arg(long, value_enum, default_value = "info")]
    pub log_level: LogLevel,

    #[command(subcommand)]
    pub command: LicenseSubcommands,
}

/// License management subcommands
#[derive(Subcommand)]
pub enum LicenseSubcommands {
    /// Issue a signed commercial license document
    Issue(IssueLicenseArgs),
}

/// Arguments for issuing a signed license
#[derive(Parser)]
pub struct IssueLicenseArgs {
    /// Licensed edition
    #[arg(long, value_enum)]
    pub edition: LicenseEdition,

    /// Installation ID this license is bound to
    #[arg(long)]
    pub installation_id: Option<String>,

    /// License expiration as a Unix timestamp in seconds
    #[arg(long)]
    pub expires_at: i64,

    /// Earliest valid time as a Unix timestamp in seconds
    #[arg(long)]
    pub not_before: Option<i64>,

    /// Ed25519 private key material as base64, hex, or prefixed key text
    #[arg(long)]
    pub private_key: Option<String>,

    /// File containing Ed25519 private key material
    #[arg(long)]
    pub key_file: Option<PathBuf>,

    /// Write the license document to this file instead of stdout
    #[arg(long)]
    pub output: Option<PathBuf>,
}

/// Commercial editions that require signed licenses
#[derive(Debug, Clone, ValueEnum)]
pub enum LicenseEdition {
    Professional,
    Enterprise,
}

/// Plugin management command arguments
#[derive(Parser)]
pub struct ManagePluginsArgs {
    /// Database file path
    #[arg(long, default_value = "oxidedb-sqlite")]
    pub db_path: PathBuf,

    /// Plugin folder path
    #[arg(long, default_value = "oxide-plugins")]
    pub plugin_folder: PathBuf,

    /// Security policy for plugin execution
    #[arg(long, value_enum, default_value = "untrusted")]
    pub security_policy: SecurityPolicy,

    /// Log level for the operation
    #[arg(long, value_enum, default_value = "info")]
    pub log_level: LogLevel,

    #[command(subcommand)]
    pub command: PluginSubcommands,
}

/// Plugin management subcommands
#[derive(Subcommand)]
pub enum PluginSubcommands {
    /// List all installed plugins
    List,
    /// Install a plugin from a ZIP package
    Install(InstallPluginArgs),
    /// Enable a plugin
    Enable(PluginNameArgs),
    /// Disable a plugin
    Disable(PluginNameArgs),
    /// Uninstall a plugin
    Uninstall(PluginNameArgs),
    /// Show detailed information about a plugin
    Show(PluginNameArgs),
    /// Analyze a plugin package without installing
    Analyze(AnalyzePluginArgs),
    /// Sign a plugin package with an Ed25519 private key
    Sign(SignPluginArgs),
}

/// Arguments for plugin installation
#[derive(Parser)]
pub struct InstallPluginArgs {
    /// Path to the plugin ZIP package
    #[arg(value_name = "PACKAGE_PATH")]
    pub package_path: PathBuf,

    /// Trust level for the plugin
    #[arg(long, value_enum, default_value = "untrusted")]
    pub trust_level: PluginTrustLevel,

    /// Force installation even if the plugin already exists
    #[arg(long)]
    pub force: bool,

    /// Automatically grant all required capabilities
    #[arg(long)]
    pub auto_grant_capabilities: bool,
}

/// Arguments for plugin name-based operations
#[derive(Parser)]
pub struct PluginNameArgs {
    /// Name of the plugin
    #[arg(value_name = "PLUGIN_NAME")]
    pub plugin_name: String,
}

/// Arguments for plugin analysis
#[derive(Parser)]
pub struct AnalyzePluginArgs {
    /// Path to the plugin ZIP package
    #[arg(value_name = "PACKAGE_PATH")]
    pub package_path: PathBuf,
}

/// Arguments for plugin package signing
#[derive(Parser)]
pub struct SignPluginArgs {
    /// Path to the plugin ZIP package
    #[arg(value_name = "PACKAGE_PATH")]
    pub package_path: PathBuf,

    /// Signing private key material (32-byte seed or 64-byte keypair; base64 or hex)
    #[arg(long)]
    pub private_key: Option<String>,

    /// File containing signing private key material
    #[arg(long)]
    pub key_file: Option<PathBuf>,

    /// Output path for the signed package
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Overwrite the output package if it already exists
    #[arg(long)]
    pub force: bool,
}

/// Plugin trust level for CLI
#[derive(Debug, Clone, ValueEnum)]
pub enum PluginTrustLevel {
    /// Untrusted plugins with minimal capabilities
    Untrusted,
    /// Partially trusted plugins with limited capabilities
    PartiallyTrusted,
    /// Fully trusted plugins with most capabilities
    FullyTrusted,
    /// System-level plugins with all capabilities
    System,
}

impl From<PluginTrustLevel> for oxide_core::plugin_security::PluginTrustLevel {
    fn from(trust_level: PluginTrustLevel) -> Self {
        match trust_level {
            PluginTrustLevel::Untrusted => oxide_core::plugin_security::PluginTrustLevel::Untrusted,
            PluginTrustLevel::PartiallyTrusted => {
                oxide_core::plugin_security::PluginTrustLevel::PartiallyTrusted
            }
            PluginTrustLevel::FullyTrusted => {
                oxide_core::plugin_security::PluginTrustLevel::FullyTrusted
            }
            PluginTrustLevel::System => oxide_core::plugin_security::PluginTrustLevel::System,
        }
    }
}
