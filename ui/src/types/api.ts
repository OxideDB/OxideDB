import type { 
  CollectionType,
  FieldDefinition,
  IndexDefinition,
  DashboardStats,
  SystemStats
} from './generated';

// Import auto-generated types from the generated types file
export type {
  DbRecord,
  CollectionStats,
  HealthStatus,
  ApiError,
  CollectionType,
  FieldType,
  FieldDefinition,
  IndexDefinition,
  CollectionSchema,
  DashboardStats,
  SystemStats,
  ActivityEntry,
  SystemHealth,
  StorageUsage,
  UserStats,
  ApiStats
} from './generated';

// Additional request types that are not auto-generated from Rust
export interface CreateCollectionRequest {
  name: string;
  collection_type: CollectionType;
  fields: Record<string, FieldDefinition>;
  indexes: IndexDefinition[];
  auth_config?: AuthCollectionConfig;
}

// Permission system types (will be auto-generated when types are regenerated)
export type UserRole = 'user' | 'superuser';

export type CrudOperation =
  | 'create'
  | 'read'
  | 'update'
  | 'delete'
  | 'list';

export type AuthOperation =
  | 'login'
  | 'register'
  | 'token_validation'
  | 'token_refresh'
  | 'logout'
  | 'get_current_user'
  | 'list_auth_collections';

export type PermissionLevel =
  | 'none'
  | 'superuseronly'
  | 'authenticatedonly'
  | 'public'
  | { rule: string };

export interface CrudOperationRule {
  operation: CrudOperation;
  permission: PermissionLevel;
  filter?: string;
}

export interface AuthOperationRule {
  operation: AuthOperation;
  permission: PermissionLevel;
  filter?: string;
}

export interface CollectionPermissions {
  collection: string;
  crud_rules: Record<CrudOperation, CrudOperationRule>;
  auth_rules: Record<AuthOperation, AuthOperationRule>;
  auth_required: boolean;
  created_at: number;
  updated_at: number;
}

// Additional types that are not auto-generated from Rust
export interface AuthResponse {
  token: string;
  refresh_token?: string;
  user_id: string;
  email: string;
  role: string;
  auth_collection: string;
  expires_in: number;
  refresh_expires_in?: number;
  custom_claims?: unknown;
}

export interface TokenValidationResponse {
  valid: boolean;
  user_id?: string;
  email?: string;
  role?: string;
  auth_collection?: string;
  expires_at?: number;
  custom_claims?: unknown;
}

export interface LoginRequest {
  email: string;
  password: string;
}

export interface RegisterRequest {
  email: string;
  password: string;
  is_superuser?: boolean;
}

export interface User {
  id: string;
  email: string;
  is_superuser: boolean;
  created_at: string;
}

export interface CollectionPermissionsInfo {
  collection_name: string;
  collection_type: CollectionType;
  permissions: CollectionPermissions;
  has_custom_rules: boolean;
}

export type PermissionPresetType = 'public' | 'authenticated_only' | 'superuser_only' | 'read_only';

// API Response wrapper
export interface ApiResponse<T> {
  data: T;
  success: boolean;
  status?: string;
  message?: string;
}

export interface ComponentVersions {
  api: string;
  database: string;
  vfs: string;
  plugin_runtime: string;
}

export interface ApiHealthStatus {
  status: string;
  database: string;
  version?: string;
  uptime?: number;
  versions?: ComponentVersions;
}

// Logging system types
export interface LogQueryParams {
  level?: string;
  start_time?: string;
  end_time?: string;
  correlation_id?: string;
  module?: string;
  user_id?: string;
  collection?: string;
  search?: string;
  limit?: number;
  offset?: number;
  sort?: string;
}

export interface AuditQueryParams {
  severity?: string;
  start_time?: string;
  end_time?: string;
  event_type?: string;
  actor?: string;
  target?: string;
  correlation_id?: string;
  min_risk_score?: number;
  limit?: number;
  offset?: number;
  sort?: string;
}

export interface LogResponse<T> {
  data: T[];
  pagination: PaginationInfo;
  metadata: QueryMetadata;
}

export interface PaginationInfo {
  offset: number;
  limit: number;
  total?: number;
  has_more: boolean;
}

export interface QueryMetadata {
  execution_time_ms: number;
  executed_at: string;
  filters_applied: string[];
}

export interface LogContext {
  user_id?: string;
  session_id?: string;
  collection?: string;
  record_id?: string;
  operation?: string;
  client_ip?: string;
  user_agent?: string;
  metadata: Record<string, unknown>;
}

export interface LogEntry {
  id: string;
  correlation_id: string;
  timestamp: string;
  level: LogLevel;
  message: string;
  module: string;
  location?: string;
  context: LogContext;
  error?: string;
  stack_trace?: string;
  metrics?: Record<string, number>;
}

export type LogLevel = 'ERROR' | 'WARN' | 'INFO' | 'DEBUG' | 'TRACE';

export type AuditEventType = 
  | 'Authentication'
  | 'Authorization' 
  | 'DataAccess'
  | 'DataModification'
  | 'ConfigurationChange'
  | 'SecurityViolation'
  | 'PluginEvent'
  | 'SystemEvent';

export interface SecurityAuditEvent {
  id: string;
  correlation_id: string;
  timestamp: string;
  event_type: AuditEventType;
  severity: LogLevel;
  description: string;
  actor: string;
  target?: string;
  action: string;
  result: string;
  context: LogContext;
  risk_score?: number;
  integrity_hash?: string;
}

export interface LogMetrics {
  total_entries: number;
  entries_by_level: Record<LogLevel, number>;
  audit_events_by_type: Record<AuditEventType, number>;
  storage_size_bytes: number;
  avg_entries_per_day: number;
  top_users: [string, number][];
  top_collections: [string, number][];
  error_rate_24h: number;
}

export interface ErrorTrendPoint {
  timestamp: string;
  error_count: number;
  total_count: number;
  error_rate: number;
}

export interface ErrorSource {
  module: string;
  error_count: number;
  latest_error?: string;
  latest_timestamp?: string;
}

export interface UserActivity {
  user_id: string;
  action_count: number;
  error_count: number;
  last_activity: string;
}

export interface LogCollectionStats {
  name: string;
  access_count: number;
  modification_count: number;
  last_accessed: string;
}

export interface HealthIndicators {
  current_error_rate: number;
  ingestion_rate: number;
  storage_utilization: number;
  avg_query_time_ms: number;
  active_users: number;
}

export interface DashboardMetrics {
  log_metrics: LogMetrics;
  error_trends: ErrorTrendPoint[];
  top_error_sources: ErrorSource[];
  user_activity: UserActivity[];
  collection_stats: LogCollectionStats[];
  health_indicators: HealthIndicators;
}

export interface RetentionPolicy {
  standard_retention_days: number;
  audit_retention_days: number;
  error_retention_days: number;
  enable_compression: boolean;
  archive_directory?: string;
  delete_after_archive: boolean;
  min_free_space_bytes?: number;
}

export interface RetentionStats {
  total_log_entries: number;
  storage_size_bytes: number;
  estimated_cleanup_candidates: number;
  last_cleanup_time?: string;
  policy: RetentionPolicy;
}

export interface CreateLogRequest {
  level: string;
  message: string;
  module: string;
  context?: LogContext;
}

export interface CreateAuditRequest {
  event_type: string;
  severity: string;
  description: string;
  actor: string;
  target?: string;
  action: string;
  result: string;
  context?: LogContext;
  risk_score?: number;
}

export interface CreateLogResponse {
  id: string;
  message: string;
}

export interface LoggingHealthResponse {
  status: string;
  total_entries: number;
  storage_size_mb: number;
  error_rate_24h: number;
}

// Plugin-specific CRUD operations for capabilities (different from API CrudOperation)
export type PluginCrudOperation = "Create" | "Read" | "Update" | "Delete";

// PluginCapability types matching Rust enum
export type PluginCapability = 
  | "LogInfo"
  | "LogError"
  | "ReadEventData"
  | "ModifyEventData"
  | "BlockOperations"
  | "ScheduleTasks"
  | "HandleHttpRequests"
  | { ReadConfig: { keys: string[] } }
  | { AccessCollection: { collection: string; operations: PluginCrudOperation[] } }
  | { HttpRequest: { allowed_urls: string[]; rate_limit: number } }
  | { PersistentStorage: { max_size: number; key_prefixes: string[] } }
  | { EmitEvents: { event_types: string[] } }
  | { RegisterHttpRoutes: { path_patterns: string[]; methods: string[] } }
  | { CreateRecords: { collections: string[] } }
  | { ReadRecords: { collections: string[] } }
  | { UpdateRecords: { collections: string[] } }
  | { DeleteRecords: { collections: string[] } };

// Utility functions to create capability objects
export const createCapability = {
  LogInfo: (): PluginCapability => "LogInfo",
  LogError: (): PluginCapability => "LogError",
  ReadEventData: (): PluginCapability => "ReadEventData",
  ModifyEventData: (): PluginCapability => "ModifyEventData",
  BlockOperations: (): PluginCapability => "BlockOperations",
  ScheduleTasks: (): PluginCapability => "ScheduleTasks",
  HandleHttpRequests: (): PluginCapability => "HandleHttpRequests",
  ReadConfig: (keys: string[] = ["*"]): PluginCapability => ({ ReadConfig: { keys } }),
  AccessCollection: (collection: string = "*", operations: PluginCrudOperation[] = ["Read"]): PluginCapability => 
    ({ AccessCollection: { collection, operations } }),
  HttpRequest: (allowed_urls: string[] = ["*"], rate_limit: number = 60): PluginCapability => 
    ({ HttpRequest: { allowed_urls, rate_limit } }),
  PersistentStorage: (max_size: number = 1024 * 1024, key_prefixes: string[] = ["plugin_*"]): PluginCapability => 
    ({ PersistentStorage: { max_size, key_prefixes } }),
  EmitEvents: (event_types: string[] = ["custom.*"]): PluginCapability => 
    ({ EmitEvents: { event_types } }),
  RegisterHttpRoutes: (path_patterns: string[] = ["*"], methods: string[] = ["GET", "POST"]): PluginCapability => 
    ({ RegisterHttpRoutes: { path_patterns, methods } }),
  CreateRecords: (collections: string[] = ["*"]): PluginCapability => 
    ({ CreateRecords: { collections } }),
  ReadRecords: (collections: string[] = ["*"]): PluginCapability => 
    ({ ReadRecords: { collections } }),
  UpdateRecords: (collections: string[] = ["*"]): PluginCapability => 
    ({ UpdateRecords: { collections } }),
  DeleteRecords: (collections: string[] = ["*"]): PluginCapability => 
    ({ DeleteRecords: { collections } }),
};

// Function to convert capability names to capability objects
export function capabilityNameToObject(capabilityName: string): PluginCapability {
  const normalizedCapability = capabilityName.trim().split('(')[0];

  switch (normalizedCapability) {
    case "LogInfo": return createCapability.LogInfo();
    case "LogError": return createCapability.LogError();
    case "ReadEventData": return createCapability.ReadEventData();
    case "ModifyEventData": return createCapability.ModifyEventData();
    case "BlockOperations": return createCapability.BlockOperations();
    case "ScheduleTasks": return createCapability.ScheduleTasks();
    case "HandleHttpRequests": return createCapability.HandleHttpRequests();
    case "ReadConfig": return createCapability.ReadConfig();
    case "AccessCollection": return createCapability.AccessCollection();
    case "HttpRequest": return createCapability.HttpRequest();
    case "PersistentStorage": return createCapability.PersistentStorage();
    case "EmitEvents": return createCapability.EmitEvents();
    case "RegisterHttpRoutes": return createCapability.RegisterHttpRoutes();
    case "CreateRecords": return createCapability.CreateRecords();
    case "ReadRecords": return createCapability.ReadRecords();
    case "UpdateRecords": return createCapability.UpdateRecords();
    case "DeleteRecords": return createCapability.DeleteRecords();
    default:
      throw new Error(`Unknown capability: ${capabilityName}`);
  }
}

// Function to extract capability name from capability object for display
export function getCapabilityName(capability: PluginCapability): string {
  if (typeof capability === "string") {
    return capability;
  }
  return Object.keys(capability)[0];
}

// VFS and File Field Types
export interface FileReference {
  file_id: string;
  name: string;
  mime_type: string;
  size: number;
  path: string;
}

export interface FileFieldConfig {
  multiple: boolean;
  allowed_mime_types?: string[];
  max_file_size?: number;
  required: boolean;
}

export interface FileMetadata {
  file_id: string;
  name: string;
  path: string;
  mime_type: string;
  size: number;
  content_hash: string;
  created_at?: number;
  modified_at?: number;
  custom_metadata?: Record<string, string>;
  compressed?: boolean;
  compression_type?: string;
  tags?: string[];
}

export interface FileWriteRequest {
  path: string;
  mime_type?: string;
  custom_metadata?: Record<string, string>;
  tags?: string[];
  overwrite: boolean;
}

export interface FileReadResponse {
  metadata: FileMetadata;
  content?: ArrayBuffer;
}

export interface FileListRequest {
  directory: string;
  recursive: boolean;
  mime_filter?: string;
  tag_filter?: string[];
  offset?: number;
  limit?: number;
}

export interface FileListResponse {
  files: FileMetadata[];
  total_count: number;
  has_more: boolean;
}

export interface VfsUsageStats {
  namespace: string;
  file_count: number;
  storage_used: number;
  storage_quota?: number;
  directory_count: number;
  last_updated: number;
}

export interface FileUploadProgress {
  fileId: string;
  fileName: string;
  progress: number;
  status: 'uploading' | 'completed' | 'error';
  error?: string;
}

export interface AuthCollectionConfig {
  identifierField: string;
  credentialField: string;
  registrationEnabled: boolean;
  emailVerificationRequired: boolean;
  refreshTokensEnabled: boolean;
  refreshTokensRequired: boolean;
  customClaimFields: string[];
}

// Site Settings types (matching oxide-core structure)
export interface SiteSettings {
  branding: BrandingSettings;
  email: EmailSettings;
  system_info: SystemInfoSettings;
  general: GeneralSettings;
  security: SecuritySettings;
  created_at: number;
  updated_at: number;
}

export interface SiteSettingsResponse {
  success: boolean;
  message?: string | null;
  settings?: SiteSettings | null;
}

export interface SettingsHealthStatus {
  healthy: boolean;
  email_config_valid: boolean;
  license_valid: boolean;
  warnings: string[];
  last_validated_at: number;
}

export interface BrandingSettings {
  site_title: string;
  site_description?: string;
  logo_url?: string;
  favicon_url?: string;
  primary_color?: string;
  secondary_color?: string;
  custom_css?: string;
  footer_text?: string;
}

export interface EmailSettings {
  enabled: boolean;
  smtp_host?: string;
  smtp_port?: number;
  smtp_username?: string;
  smtp_password?: string;
  smtp_tls: boolean;
  smtp_starttls: boolean;
  from_email?: string;
  from_name?: string;
  reply_to_email?: string;
  templates: EmailTemplateSettings;
}

export interface EmailTemplateSettings {
  verification_template?: string;
  password_reset_template?: string;
  welcome_template?: string;
  signature?: string;
}

export interface SystemInfoSettings {
  oxidedb_version: string;
  oxidedb_edition: OxideDbEdition;
  installation_id: string;
  environment: DeploymentEnvironment;
  instance_name?: string;
  license_key?: string;
  license_expires_at?: number;
}

export interface GeneralSettings {
  default_timezone: string;
  default_locale: string;
  max_upload_size: number;
  allow_user_registration: boolean;
  allow_public_api: boolean;
  api_rate_limit: number;
  maintenance: MaintenanceSettings;
  backup: BackupSettings;
}

export interface SecuritySettings {
  require_email_verification: boolean;
  password_min_length: number;
  password_require_complexity: boolean;
  session_timeout_minutes: number;
  max_login_attempts: number;
  lockout_duration_minutes: number;
  enable_2fa: boolean;
  force_2fa_admin: boolean;
  enable_audit_logging: boolean;
}

export interface MaintenanceSettings {
  enabled: boolean;
  message?: string;
  estimated_completion?: number;
  allow_admin_access: boolean;
}

export interface BackupSettings {
  enabled: boolean;
  frequency_hours: number;
  retention_count: number;
  storage_location?: string;
  enable_compression: boolean;
  include_user_data: boolean;
}

export type OxideDbEdition = 'Community' | 'Professional' | 'Enterprise';
export type DeploymentEnvironment = 'Development' | 'Staging' | 'Production';

export interface UpdateSiteSettingsRequest {
  branding?: BrandingSettings;
  email?: EmailSettings;
  system_info?: SystemInfoUpdateRequest;
  general?: GeneralSettings;
  security?: SecuritySettings;
}

export interface SystemInfoUpdateRequest {
  oxidedb_edition?: OxideDbEdition;
  environment?: DeploymentEnvironment;
  instance_name?: string;
  license_key?: string;
}

// Dashboard API functions
export const dashboardApi = {
  // Get comprehensive dashboard statistics
  getDashboardStats: async (): Promise<DashboardStats> => {
    const response = await fetch('/api/dashboard/stats', {
      headers: {
        'Authorization': `Bearer ${localStorage.getItem('authToken')}`,
        'Content-Type': 'application/json',
      },
    });
    
    if (!response.ok) {
      throw new Error(`Failed to fetch dashboard stats: ${response.statusText}`);
    }
    
    const result = await response.json();
    return result.data;
  },

  // Get basic system statistics
  getSystemStats: async (): Promise<SystemStats> => {
    const response = await fetch('/api/dashboard/system', {
      headers: {
        'Authorization': `Bearer ${localStorage.getItem('authToken')}`,
        'Content-Type': 'application/json',
      },
    });
    
    if (!response.ok) {
      throw new Error(`Failed to fetch system stats: ${response.statusText}`);
    }
    
    const result = await response.json();
    return result.data;
  },
};
