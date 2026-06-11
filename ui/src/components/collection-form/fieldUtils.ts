import type { FieldDefinition, FieldType } from '../../types/api';
import type { FieldFormData, RelationshipConfig, FileConfig, SelectConfig } from './types';

export const getFieldTypeString = (fieldType: FieldType): string => {
  if (typeof fieldType === 'string') {
    return fieldType;
  } else if (typeof fieldType === 'object' && 'relationship' in fieldType) {
    return 'relationship';
  } else if (typeof fieldType === 'object' && 'file' in fieldType) {
    return 'file';
  } else if (typeof fieldType === 'object' && 'select' in fieldType) {
    return 'select';
  }
  return 'text';
};

export const createRelationshipFieldType = (config: RelationshipConfig): FieldType => ({
  relationship: {
    target_collection: config.target_collection || '',
    multiple: config.multiple || false,
    cascade_delete: config.cascade_delete || false,
    display_field: config.display_field,
  }
});

export const createFileFieldType = (config: FileConfig): FieldType => ({
  file: {
    multiple: config.multiple || false,
    allowed_mime_types: config.allowed_mime_types,
    max_file_size: config.max_file_size ? BigInt(config.max_file_size) : null,
    required: config.required || false,
  }
});

export const createSelectFieldType = (config: SelectConfig): FieldType => ({
  select: {
    options: config.options || [],
    multiple: config.multiple || false,
    allow_empty: config.allow_empty !== undefined ? config.allow_empty : true,
  }
});

export const createDefaultField = (): FieldFormData => ({
  name: '',
  field_type: 'text',
  required: false,
  unique: false,
});

export const createRelationshipField = (): FieldFormData => ({
  ...createDefaultField(),
  field_type: createRelationshipFieldType({
    target_collection: '',
    multiple: false,
    cascade_delete: false,
    display_field: undefined,
  }),
  relationshipConfig: {
    target_collection: '',
    multiple: false,
    cascade_delete: false,
    display_field: undefined,
  }
});

export const createFileField = (): FieldFormData => ({
  ...createDefaultField(),
  field_type: createFileFieldType({
    multiple: false,
    allowed_mime_types: null,
    max_file_size: 10 * 1024 * 1024, // 10MB default
    required: false,
  }),
  fileConfig: {
    multiple: false,
    allowed_mime_types: null,
    max_file_size: 10 * 1024 * 1024, // 10MB default
    required: false,
  }
});

export const createSelectField = (): FieldFormData => ({
  ...createDefaultField(),
  field_type: createSelectFieldType({
    options: [],
    multiple: false,
    allow_empty: true,
  }),
  selectConfig: {
    options: [],
    multiple: false,
    allow_empty: true,
  }
});

export const updateFieldType = (field: FieldFormData, newType: string): FieldFormData => {
  const baseField = { ...field, relationshipConfig: undefined, fileConfig: undefined, selectConfig: undefined };
  
  switch (newType) {
    case 'relationship':
      return {
        ...baseField,
        field_type: createRelationshipFieldType({
          target_collection: '',
          multiple: false,
          cascade_delete: false,
          display_field: undefined,
        }),
        relationshipConfig: {
          target_collection: '',
          multiple: false,
          cascade_delete: false,
          display_field: undefined,
        }
      };
    case 'file':
      return {
        ...baseField,
        field_type: createFileFieldType({
          multiple: false,
          allowed_mime_types: null,
          max_file_size: 10 * 1024 * 1024,
          required: false,
        }),
        fileConfig: {
          multiple: false,
          allowed_mime_types: null,
          max_file_size: 10 * 1024 * 1024,
          required: false,
        }
      };
    case 'select':
      return {
        ...baseField,
        field_type: createSelectFieldType({
          options: [],
          multiple: false,
          allow_empty: true,
        }),
        selectConfig: {
          options: [],
          multiple: false,
          allow_empty: true,
        }
      };
    default:
      return {
        ...baseField,
        field_type: newType as FieldType,
      };
  }
};

export const convertSchemaFieldsToFormData = (schemaFields: Record<string, FieldDefinition>): FieldFormData[] => {
  return Object.entries(schemaFields).map(([name, fieldDef]) => {
    const field: FieldFormData = {
      name,
      field_type: fieldDef.field_type,
      required: fieldDef.required,
      unique: fieldDef.unique,
      default: fieldDef.default ? JSON.stringify(fieldDef.default) : undefined,
      validation: fieldDef.validation ? {
        regex: fieldDef.validation.regex,
        min: fieldDef.validation.min,
        max: fieldDef.validation.max,
        message: fieldDef.validation.message,
        allow_empty: fieldDef.validation.allow_empty
      } : undefined,
    };

    // If it's a relationship field, extract the configuration
    if (typeof fieldDef.field_type === 'object' && 'relationship' in fieldDef.field_type) {
      field.relationshipConfig = {
        target_collection: fieldDef.field_type.relationship.target_collection,
        multiple: fieldDef.field_type.relationship.multiple,
        cascade_delete: fieldDef.field_type.relationship.cascade_delete,
        display_field: fieldDef.field_type.relationship.display_field,
      };
    }

    // If it's a file field, extract the configuration
    if (typeof fieldDef.field_type === 'object' && 'file' in fieldDef.field_type) {
      field.fileConfig = {
        multiple: fieldDef.field_type.file.multiple,
        allowed_mime_types: fieldDef.field_type.file.allowed_mime_types,
        max_file_size: fieldDef.field_type.file.max_file_size ? Number(fieldDef.field_type.file.max_file_size) : null,
        required: fieldDef.field_type.file.required,
      };
    }

    // If it's a select field, extract the configuration
    if (typeof fieldDef.field_type === 'object' && 'select' in fieldDef.field_type) {
      field.selectConfig = {
        options: fieldDef.field_type.select.options,
        multiple: fieldDef.field_type.select.multiple,
        allow_empty: fieldDef.field_type.select.allow_empty,
      };
    }

    return field;
  });
}; 
