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

// Additional types that are not auto-generated from Rust
export interface AuthResponse {
  token: string;
  user_id: string;
}

export interface User {
  id: string;
  email: string;
  is_superuser: boolean;
  created_at: string;
} 