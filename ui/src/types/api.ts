export interface DbRecord {
  id: string;
  data: Record<string, any>;
  created_at: string;
  updated_at: string;
}

export interface CollectionStats {
  name: string;
  record_count: number;
  created_at: string;
  updated_at: string;
}

export interface HealthStatus {
  status: string;
  database: string;
  version: string;
}

export interface CreateCollectionRequest {
  id: string;
  name: string;
  collection_type: CollectionType;
  fields: Record<string, FieldDefinition>;
  indexes: IndexDefinition[];
  created_at: number;
  updated_at: number;
}

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

export interface ApiError {
  error: string;
  message?: string;
}

// Collection Schema Types
export type CollectionType = 'base' | 'auth';

export type FieldType = 'text' | 'number' | 'boolean' | 'date' | 'json' | 'email' | 'url' | 'password';

export interface FieldDefinition {
  field_type: FieldType;
  required: boolean;
  unique: boolean;
  default?: any;
  validation?: any;
}

export interface IndexDefinition {
  name: string;
  fields: string[];
  unique: boolean;
}

export interface CollectionSchema {
  id: string;
  name: string;
  collection_type: CollectionType;
  fields: Record<string, FieldDefinition>;
  indexes: IndexDefinition[];
  created_at: number;
  updated_at: number;
} 