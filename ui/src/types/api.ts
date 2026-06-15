import type { 
  CollectionSchema,
  CollectionType,
  DbRecord,
  FieldDefinition,
  IndexDefinition
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
  token?: string;
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

export type ApiKeyOperationType = 'crud' | 'auth';

export interface ApiKeyCollectionOption {
  name: string;
  collection_type: CollectionType;
}

export interface ApiKeyRuleInfo {
  collection: string;
  collection_type: CollectionType;
  operation_type: ApiKeyOperationType;
  operation: CrudOperation | AuthOperation;
  rule: string;
  key_hash?: string | null;
  key_preview?: string | null;
  hashed: boolean;
  legacy_plaintext: boolean;
}

export interface ApiKeyRulesResponse {
  rules: ApiKeyRuleInfo[];
  collections: ApiKeyCollectionOption[];
  total_rules: number;
  protected_collections: number;
  exact_key_rules: number;
  hashed_key_rules: number;
  wildcard_key_rules: number;
}

export interface UpsertApiKeyRuleRequest {
  collection: string;
  operation_type: ApiKeyOperationType;
  operation: CrudOperation | AuthOperation;
  key?: string;
}

export interface UpsertApiKeyRuleResponse {
  rule: ApiKeyRuleInfo;
  api_key: string;
  summary: ApiKeyRulesResponse;
}

export interface RevokeApiKeyRuleRequest {
  collection: string;
  operation_type: ApiKeyOperationType;
  operation: CrudOperation | AuthOperation;
  fallback_permission?: PermissionLevel;
}

export interface PluginAdminPage {
  plugin_name: string;
  plugin_version: string;
  slug: string;
  title: string;
  description?: string;
  icon?: string;
  nav_group: string;
  admin_path: string;
  source_url: string;
  enabled: boolean;
}

export interface BackupCollectionSummary {
  name: string;
  collection_type: CollectionType;
  schema_version: number;
  record_count: number;
  size_kb: number;
  included: boolean;
  excluded_reason?: string | null;
}

export interface BackupManifestResponse {
  generated_at: string;
  total_collections: number;
  included_collections: number;
  total_records: number;
  total_size_kb: number;
  include_system: boolean;
  include_vfs: boolean;
  total_vfs_files: number;
  total_vfs_size_bytes: number;
  collections: BackupCollectionSummary[];
  vfs_namespaces: BackupVfsNamespaceSummary[];
}

export interface BackupCollectionExport {
  schema: CollectionSchema;
  records: DbRecord[];
  record_count: number;
}

export interface BackupVfsNamespaceSummary {
  namespace: string;
  file_count: number;
  total_size_bytes: number;
}

export interface BackupVfsNamespaceConfig {
  namespace: string;
  quota_bytes?: number | null;
  enable_compression: boolean;
  allowed_mime_types?: string[] | null;
  max_file_size?: number | null;
  enable_deduplication: boolean;
  backup_config?: unknown | null;
}

export interface BackupVfsFileMetadata {
  id: string;
  name: string;
  path: string;
  mime_type: string;
  size: number;
  content_hash: string;
  created_at: number;
  modified_at: number;
  custom_metadata: Record<string, string>;
  compressed: boolean;
  compression_type?: string | null;
  tags: string[];
}

export interface BackupVfsFileExport {
  metadata: BackupVfsFileMetadata;
  content_encoding: string;
  content_base64: string;
}

export interface BackupVfsNamespaceExport {
  config: BackupVfsNamespaceConfig;
  files: BackupVfsFileExport[];
  file_count: number;
  total_size_bytes: number;
}

export interface BackupExportResponse {
  format_version: number;
  generated_at: string;
  include_system: boolean;
  include_vfs?: boolean;
  total_collections: number;
  total_records: number;
  total_vfs_files?: number;
  total_vfs_size_bytes?: number;
  collections: BackupCollectionExport[];
  vfs_namespaces?: BackupVfsNamespaceExport[];
}

export interface BackupQueryOptions {
  include_system?: boolean;
  include_vfs?: boolean;
  collections?: string[];
}

export interface BackupRestoreRequest {
  snapshot: BackupExportResponse;
  dry_run?: boolean;
  include_system?: boolean;
  replace_existing?: boolean;
  include_vfs?: boolean;
}

export interface BackupRestoreCollectionResult {
  name: string;
  collection_type: CollectionType;
  status: string;
  source_records: number;
  records_created: number;
  records_updated: number;
  records_skipped: number;
  warnings: string[];
}

export interface BackupVfsRestoreNamespaceResult {
  namespace: string;
  status: string;
  source_files: number;
  files_created: number;
  files_skipped: number;
  warnings: string[];
}

export interface BackupRestoreResponse {
  dry_run: boolean;
  generated_at: string;
  source_generated_at: string;
  include_system: boolean;
  include_vfs: boolean;
  replace_existing: boolean;
  total_collections: number;
  total_records: number;
  total_vfs_namespaces: number;
  total_vfs_files: number;
  created_collections: number;
  replaced_collections: number;
  skipped_collections: number;
  created_records: number;
  updated_records: number;
  skipped_records: number;
  created_vfs_namespaces: number;
  replaced_vfs_namespaces: number;
  skipped_vfs_namespaces: number;
  created_vfs_files: number;
  skipped_vfs_files: number;
  collections: BackupRestoreCollectionResult[];
  vfs_namespaces: BackupVfsRestoreNamespaceResult[];
  warnings: string[];
}

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
  entries_24h: number;
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
export type PluginCrudOperation = "Create" | "Read" | "Update" | "Delete" | "List";
export type PluginCollectionOperation =
  | "Create"
  | "Read"
  | "Update"
  | "Delete"
  | "List"
  | "Exists"
  | "Stats";
export type PluginVfsOperation = "Write" | "Read" | "Move" | "Delete" | "List" | "Usage";

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
  | { AccessVfs: { namespaces: string[]; operations: PluginVfsOperation[] } }
  | { RegisterHttpRoutes: { path_patterns: string[]; methods: string[] } }
  | { CreateRecords: { collections: string[] } }
  | { ReadRecords: { collections: string[] } }
  | { UpdateRecords: { collections: string[] } }
  | { DeleteRecords: { collections: string[] } }
  | { ManageCollections: { collections: string[]; operations: PluginCollectionOperation[] } };

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
  AccessVfs: (namespaces: string[] = ["*"], operations: PluginVfsOperation[] = ["Read", "List", "Usage"]): PluginCapability =>
    ({ AccessVfs: { namespaces, operations } }),
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
  ManageCollections: (
    collections: string[] = ["*"],
    operations: PluginCollectionOperation[] = ["Read", "List"]
  ): PluginCapability => ({ ManageCollections: { collections, operations } }),
};

function splitCapabilityArgs(input: string): string[] {
  const parts: string[] = [];
  let current = '';
  let bracketDepth = 0;
  let braceDepth = 0;
  let parenDepth = 0;
  let quote: string | null = null;
  let escaped = false;

  for (const char of input) {
    if (escaped) {
      current += char;
      escaped = false;
      continue;
    }

    if (char === '\\') {
      current += char;
      escaped = true;
      continue;
    }

    if (quote) {
      if (char === quote) quote = null;
      current += char;
      continue;
    }

    if (char === '"' || char === "'") {
      quote = char;
      current += char;
    } else if (char === '[') {
      bracketDepth += 1;
      current += char;
    } else if (char === ']') {
      bracketDepth -= 1;
      current += char;
    } else if (char === '{') {
      braceDepth += 1;
      current += char;
    } else if (char === '}') {
      braceDepth -= 1;
      current += char;
    } else if (char === '(') {
      parenDepth += 1;
      current += char;
    } else if (char === ')') {
      parenDepth -= 1;
      current += char;
    } else if (char === ',' && bracketDepth === 0 && braceDepth === 0 && parenDepth === 0) {
      if (current.trim()) parts.push(current.trim());
      current = '';
    } else {
      current += char;
    }
  }

  if (current.trim()) parts.push(current.trim());
  return parts;
}

function parseCapabilityValue(raw: string): unknown {
  const value = raw.trim();

  try {
    return JSON.parse(value);
  } catch {
    // Continue with the small manifest shorthand parser below.
  }

  if (value.startsWith('[') && value.endsWith(']')) {
    const inner = value.slice(1, -1);
    return inner.trim() ? splitCapabilityArgs(inner).map(parseCapabilityValue) : [];
  }

  if (/^\d+$/.test(value)) return Number(value);
  if (value === 'true') return true;
  if (value === 'false') return false;
  return value.replace(/^['"]|['"]$/g, '');
}

function parseCapabilityInvocation(input: string): { name: string; args: Record<string, unknown> } {
  const trimmed = input.trim();
  const openParen = trimmed.indexOf('(');

  if (openParen === -1 || !trimmed.endsWith(')')) {
    return { name: trimmed.split('(')[0], args: {} };
  }

  const args: Record<string, unknown> = {};
  for (const segment of splitCapabilityArgs(trimmed.slice(openParen + 1, -1))) {
    const equalIndex = segment.indexOf('=');
    if (equalIndex === -1) continue;

    const key = segment.slice(0, equalIndex).trim();
    const value = segment.slice(equalIndex + 1);
    if (key) args[key] = parseCapabilityValue(value);
  }

  return { name: trimmed.slice(0, openParen).trim(), args };
}

function stringArrayArg(args: Record<string, unknown>, key: string, fallback: string[]): string[] {
  const value = args[key];
  if (value === undefined) return fallback;
  if (Array.isArray(value)) return value.map(String);
  return [String(value)];
}

function numberArg(args: Record<string, unknown>, key: string, fallback: number): number {
  const value = args[key];
  if (value === undefined) return fallback;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function normalizeVfsOperation(operation: string): PluginVfsOperation {
  switch (operation.trim().toLowerCase()) {
    case "write": return "Write";
    case "read": return "Read";
    case "move":
    case "rename": return "Move";
    case "delete": return "Delete";
    case "list": return "List";
    case "usage":
    case "stats":
    case "usage_stats": return "Usage";
    default:
      return operation as PluginVfsOperation;
  }
}

function normalizeCollectionOperation(operation: string): PluginCollectionOperation {
  switch (operation.trim().toLowerCase()) {
    case "create": return "Create";
    case "read":
    case "schema":
    case "get_schema": return "Read";
    case "update":
    case "update_schema": return "Update";
    case "delete": return "Delete";
    case "list": return "List";
    case "exists":
    case "collection_exists": return "Exists";
    case "stats":
    case "statistics":
    case "get_stats":
    case "get_collection_stats": return "Stats";
    default:
      return operation as PluginCollectionOperation;
  }
}

// Function to convert capability names to capability objects
export function capabilityNameToObject(capabilityName: string): PluginCapability {
  try {
    const parsed = JSON.parse(capabilityName);
    if (typeof parsed === 'string' || (parsed && typeof parsed === 'object')) {
      return parsed as PluginCapability;
    }
  } catch {
    // Continue with named capability parsing.
  }

  const { name: normalizedCapability, args } = parseCapabilityInvocation(capabilityName);

  switch (normalizedCapability) {
    case "LogInfo": return createCapability.LogInfo();
    case "LogError": return createCapability.LogError();
    case "ReadEventData": return createCapability.ReadEventData();
    case "ModifyEventData": return createCapability.ModifyEventData();
    case "BlockOperations": return createCapability.BlockOperations();
    case "ScheduleTasks": return createCapability.ScheduleTasks();
    case "HandleHttpRequests": return createCapability.HandleHttpRequests();
    case "ReadConfig": return createCapability.ReadConfig(stringArrayArg(args, "keys", ["*"]));
    case "AccessCollection": return createCapability.AccessCollection(
      String(args.collection ?? "*"),
      stringArrayArg(args, "operations", ["Read"]) as PluginCrudOperation[]
    );
    case "HttpRequest": return createCapability.HttpRequest(
      stringArrayArg(args, "allowed_urls", ["*"]),
      numberArg(args, "rate_limit", 60)
    );
    case "PersistentStorage": return createCapability.PersistentStorage(
      numberArg(args, "max_size", 1024 * 1024),
      stringArrayArg(args, "key_prefixes", ["plugin_*"])
    );
    case "EmitEvents": return createCapability.EmitEvents(stringArrayArg(args, "event_types", ["custom.*"]));
    case "AccessVfs": return createCapability.AccessVfs(
      stringArrayArg(args, "namespaces", ["*"]),
      stringArrayArg(args, "operations", ["Read", "List", "Usage"]).map(normalizeVfsOperation)
    );
    case "RegisterHttpRoutes": return createCapability.RegisterHttpRoutes(
      stringArrayArg(args, "path_patterns", ["*"]),
      stringArrayArg(args, "methods", ["GET", "POST"]).map((method) => method.toUpperCase())
    );
    case "CreateRecords": return createCapability.CreateRecords(stringArrayArg(args, "collections", ["*"]));
    case "ReadRecords": return createCapability.ReadRecords(stringArrayArg(args, "collections", ["*"]));
    case "UpdateRecords": return createCapability.UpdateRecords(stringArrayArg(args, "collections", ["*"]));
    case "DeleteRecords": return createCapability.DeleteRecords(stringArrayArg(args, "collections", ["*"]));
    case "ManageCollections": return createCapability.ManageCollections(
      stringArrayArg(args, "collections", ["*"]),
      stringArrayArg(args, "operations", ["Read", "List"]).map(normalizeCollectionOperation)
    );
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

export interface FileMoveRequest {
  path: string;
  overwrite?: boolean;
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

export interface PublicSiteSettings {
  branding: BrandingSettings;
  system_info: PublicSystemInfo;
}

export interface PublicSystemInfo {
  oxidedb_version: string;
  environment: DeploymentEnvironment;
  instance_name?: string;
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
  allow_public_api: boolean;
  api_rate_limit: number;
  maintenance: MaintenanceSettings;
  backup: BackupSettings;
}

export interface SecuritySettings {
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
