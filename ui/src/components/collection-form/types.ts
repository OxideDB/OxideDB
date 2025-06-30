import type { FieldType } from '../../types/api';

export interface RelationshipConfig {
  target_collection: string;
  multiple: boolean;
  cascade_delete: boolean;
  display_field?: string;
}

export interface FileConfig {
  multiple: boolean;
  allowed_mime_types: string[] | null;
  max_file_size: number | null;
  required: boolean;
}

export interface SelectConfig {
  options: string[];
  multiple: boolean;
  allow_empty: boolean;
}

export interface FieldFormData {
  name: string;
  field_type: FieldType;
  required: boolean;
  unique: boolean;
  default?: string;
  relationshipConfig?: RelationshipConfig;
  fileConfig?: FileConfig;
  selectConfig?: SelectConfig;
} 