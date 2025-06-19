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