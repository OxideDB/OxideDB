import { useState, useCallback } from 'react';
import type { FieldFormData, RelationshipConfig, FileConfig, SelectConfig, ValidationConfig } from './types';
import { 
  createDefaultField, 
  updateFieldType as updateFieldTypeUtil,
  createRelationshipFieldType,
  createFileFieldType,
  createSelectFieldType
} from './fieldUtils';

export const useFieldManagement = (initialFields: FieldFormData[] = []) => {
  const [fields, setFields] = useState<FieldFormData[]>(initialFields);

  const addField = useCallback(() => {
    setFields(prev => [...prev, createDefaultField()]);
  }, []);

  const removeField = useCallback((index: number) => {
    setFields(prev => prev.filter((_, i) => i !== index));
  }, []);

  const updateField = useCallback((index: number, field: Partial<FieldFormData>) => {
    setFields(prev => prev.map((f, i) => i === index ? { ...f, ...field } : f));
  }, []);

  const updateFieldType = useCallback((index: number, newType: string) => {
    setFields(prev => prev.map((field, i) => 
      i === index ? updateFieldTypeUtil(field, newType) : field
    ));
  }, []);

  const updateRelationshipConfig = useCallback((index: number, config: Partial<RelationshipConfig>) => {
    setFields(prev => prev.map((field, i) => {
      if (i !== index) return field;
      
      const newConfig = { ...field.relationshipConfig, ...config };
      return {
        ...field,
        relationshipConfig: newConfig,
        field_type: createRelationshipFieldType({
          target_collection: newConfig.target_collection || '',
          multiple: newConfig.multiple || false,
          cascade_delete: newConfig.cascade_delete || false,
          display_field: newConfig.display_field,
        })
      };
    }));
  }, []);

  const updateFileConfig = useCallback((index: number, config: Partial<FileConfig>) => {
    setFields(prev => prev.map((field, i) => {
      if (i !== index) return field;
      
      const newConfig = { ...field.fileConfig, ...config };
      return {
        ...field,
        fileConfig: newConfig,
        field_type: createFileFieldType({
          multiple: newConfig.multiple || false,
          allowed_mime_types: newConfig.allowed_mime_types,
          max_file_size: newConfig.max_file_size,
          required: newConfig.required || false,
        })
      };
    }));
  }, []);

  const updateSelectConfig = useCallback((index: number, config: Partial<SelectConfig>) => {
    setFields(prev => prev.map((field, i) => {
      if (i !== index) return field;
      
      const newConfig = { ...field.selectConfig, ...config };
      return {
        ...field,
        selectConfig: newConfig,
        field_type: createSelectFieldType({
          options: newConfig.options || [],
          multiple: newConfig.multiple || false,
          allow_empty: newConfig.allow_empty !== undefined ? newConfig.allow_empty : true,
        })
      };
    }));
  }, []);

  const updateValidationConfig = useCallback((index: number, config: Partial<ValidationConfig>) => {
    setFields(prev => prev.map((field, i) => {
      if (i !== index) return field;
      
      const newConfig = { ...field.validation, ...config };
      return {
        ...field,
        validation: newConfig
      };
    }));
  }, []);

  const setFieldsFromSchema = useCallback((schemaFields: FieldFormData[]) => {
    setFields(schemaFields);
  }, []);

  return {
    fields,
    addField,
    removeField,
    updateField,
    updateFieldType,
    updateRelationshipConfig,
    updateFileConfig,
    updateSelectConfig,
    updateValidationConfig,
    setFieldsFromSchema,
  };
}; 