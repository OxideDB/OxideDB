import type { CollectionType } from './generated';

// Import auto-generated types from the generated types file
export type {
  DbRecord,
  CollectionStats,
  HealthStatus,
  CreateCollectionRequest,
  ApiError,
  CollectionType,
  FieldType,
  FieldDefinition,
  IndexDefinition,
  CollectionSchema
} from './generated';

// Permission system types (will be auto-generated when types are regenerated)
export type UserRole = 'user' | 'superuser';

export type CrudOperation = 'create' | 'read' | 'update' | 'delete' | 'list';

export type PermissionLevel = 
  | 'none'
  | 'superuseronly' 
  | 'authenticatedonly'
  | 'public'
  | { rule: string };

export interface OperationRule {
  operation: CrudOperation;
  permission: PermissionLevel;
  filter?: string;
}

export interface CollectionPermissions {
  collection: string;
  rules: Record<CrudOperation, OperationRule>;
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
  metadata: Record<string, any>;
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
  switch (capabilityName) {
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
  } else {
    return Object.keys(capability)[0];
  }
} 