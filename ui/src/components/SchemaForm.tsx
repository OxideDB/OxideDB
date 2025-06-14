import React, { useState, useEffect } from 'react';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Switch } from '@/components/ui/switch';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import type { CollectionSchema, FieldDefinition } from '../types/api';

interface SchemaFormProps {
  schema: CollectionSchema;
  initialData?: Record<string, any>;
  onSubmit: (data: Record<string, any>) => Promise<void>;
  onCancel: () => void;
  submitLabel?: string;
  isSubmitting?: boolean;
}

interface FormData {
  [key: string]: any;
}

export const SchemaForm: React.FC<SchemaFormProps> = ({
  schema,
  initialData = {},
  onSubmit,
  onCancel,
  submitLabel = 'Submit',
  isSubmitting = false,
}) => {
  const [formData, setFormData] = useState<FormData>({});
  const [errors, setErrors] = useState<Record<string, string>>({});

  useEffect(() => {
    // Initialize form data with defaults and initial values
    const data: FormData = {};
    
    Object.entries(schema.fields).forEach(([fieldName, fieldDef]) => {
      if (initialData[fieldName] !== undefined) {
        data[fieldName] = initialData[fieldName];
      } else if (fieldDef.default !== undefined && fieldDef.default !== null) {
        data[fieldName] = fieldDef.default;
      } else {
        // Set appropriate empty values based on field type
        switch (fieldDef.field_type) {
          case 'boolean':
            data[fieldName] = false;
            break;
          case 'number':
            data[fieldName] = '';
            break;
          case 'json':
            data[fieldName] = '{}';
            break;
          default:
            data[fieldName] = '';
        }
      }
    });
    
    setFormData(data);
  }, [schema, initialData]);

  const validateField = (fieldName: string, value: any, fieldDef: FieldDefinition): string | null => {
    // Check required fields
    if (fieldDef.required && (value === '' || value === null || value === undefined)) {
      return `${fieldName} is required`;
    }

    // Skip validation for empty optional fields
    if (!fieldDef.required && (value === '' || value === null || value === undefined)) {
      return null;
    }

    // Type-specific validation
    switch (fieldDef.field_type) {
      case 'email':
        if (typeof value === 'string' && value && !value.includes('@')) {
          return 'Please enter a valid email address';
        }
        break;
      case 'url':
        if (typeof value === 'string' && value && !value.match(/^https?:\/\/.+/)) {
          return 'Please enter a valid URL (http:// or https://)';
        }
        break;
      case 'number':
        if (value !== '' && isNaN(Number(value))) {
          return 'Please enter a valid number';
        }
        break;
      case 'json':
        if (typeof value === 'string' && value.trim()) {
          try {
            JSON.parse(value);
          } catch {
            return 'Please enter valid JSON';
          }
        }
        break;
      case 'password':
        if (typeof value === 'string' && value && value.length < 6) {
          return 'Password must be at least 6 characters long';
        }
        break;
    }

    return null;
  };

  const validateForm = (): boolean => {
    const newErrors: Record<string, string> = {};
    let isValid = true;

    Object.entries(schema.fields).forEach(([fieldName, fieldDef]) => {
      const error = validateField(fieldName, formData[fieldName], fieldDef);
      if (error) {
        newErrors[fieldName] = error;
        isValid = false;
      }
    });

    setErrors(newErrors);
    return isValid;
  };

  const handleFieldChange = (fieldName: string, value: any) => {
    setFormData(prev => ({ ...prev, [fieldName]: value }));
    
    // Clear error for this field if it exists
    if (errors[fieldName]) {
      setErrors(prev => ({ ...prev, [fieldName]: '' }));
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!validateForm()) {
      return;
    }

    // Convert form data to appropriate types
    const processedData: Record<string, any> = {};
    
    Object.entries(schema.fields).forEach(([fieldName, fieldDef]) => {
      const value = formData[fieldName];
      
      // Skip empty optional fields
      if (!fieldDef.required && (value === '' || value === null || value === undefined)) {
        return;
      }
      
      switch (fieldDef.field_type) {
        case 'number':
          processedData[fieldName] = value === '' ? null : Number(value);
          break;
        case 'boolean':
          processedData[fieldName] = Boolean(value);
          break;
        case 'json':
          try {
            processedData[fieldName] = value ? JSON.parse(value) : null;
          } catch {
            processedData[fieldName] = value;
          }
          break;
        default:
          processedData[fieldName] = value || null;
      }
    });

    await onSubmit(processedData);
  };

  const renderField = (fieldName: string, fieldDef: FieldDefinition) => {
    const value = formData[fieldName] ?? '';
    const error = errors[fieldName];
    const fieldId = `field-${fieldName}`;

    let fieldComponent;

    switch (fieldDef.field_type) {
      case 'boolean':
        fieldComponent = (
          <div className="flex items-center space-x-2">
            <Switch
              id={fieldId}
              checked={Boolean(value)}
              onCheckedChange={(checked) => handleFieldChange(fieldName, checked)}
            />
            <Label htmlFor={fieldId} className="text-sm">
              {value ? 'True' : 'False'}
            </Label>
          </div>
        );
        break;

      case 'json':
        fieldComponent = (
          <Textarea
            id={fieldId}
            value={typeof value === 'object' ? JSON.stringify(value, null, 2) : value}
            onChange={(e) => handleFieldChange(fieldName, e.target.value)}
            placeholder="Enter JSON data"
            className="min-h-[100px] font-mono text-sm"
          />
        );
        break;

      case 'number':
        fieldComponent = (
          <Input
            id={fieldId}
            type="number"
            step="any"
            value={value}
            onChange={(e) => handleFieldChange(fieldName, e.target.value)}
            placeholder="Enter a number"
          />
        );
        break;

      case 'email':
        fieldComponent = (
          <Input
            id={fieldId}
            type="email"
            value={value}
            onChange={(e) => handleFieldChange(fieldName, e.target.value)}
            placeholder="Enter email address"
          />
        );
        break;

      case 'url':
        fieldComponent = (
          <Input
            id={fieldId}
            type="url"
            value={value}
            onChange={(e) => handleFieldChange(fieldName, e.target.value)}
            placeholder="Enter URL (https://...)"
          />
        );
        break;

      case 'date':
        fieldComponent = (
          <Input
            id={fieldId}
            type="datetime-local"
            value={value}
            onChange={(e) => handleFieldChange(fieldName, e.target.value)}
            placeholder="Select date and time"
          />
        );
        break;

      case 'password':
        fieldComponent = (
          <Input
            id={fieldId}
            type="password"
            value={value}
            onChange={(e) => handleFieldChange(fieldName, e.target.value)}
            placeholder="Enter password"
          />
        );
        break;

      default: // text
        fieldComponent = (
          <Input
            id={fieldId}
            type="text"
            value={value}
            onChange={(e) => handleFieldChange(fieldName, e.target.value)}
            placeholder="Enter text"
          />
        );
    }

    return (
      <div key={fieldName} className="space-y-2">
        <div className="flex items-center space-x-2">
          <Label htmlFor={fieldId} className="font-medium">
            {fieldName}
          </Label>
          {fieldDef.required && (
            <Badge variant="destructive" className="text-xs">
              required
            </Badge>
          )}
          <Badge variant="outline" className="text-xs">
            {fieldDef.field_type}
          </Badge>
        </div>
        {fieldComponent}
        {error && (
          <p className="text-sm text-destructive">{error}</p>
        )}
        {fieldDef.default !== undefined && fieldDef.default !== null && (
          <p className="text-xs text-muted-foreground">
            Default: {typeof fieldDef.default === 'object' 
              ? JSON.stringify(fieldDef.default) 
              : String(fieldDef.default)}
          </p>
        )}
      </div>
    );
  };

  const sortedFields = Object.entries(schema.fields).sort(([, a], [, b]) => {
    // Sort required fields first, then by field name
    if (a.required && !b.required) return -1;
    if (!a.required && b.required) return 1;
    return 0;
  });

  return (
    <form onSubmit={handleSubmit} className="space-y-6">
      <div className="space-y-4">
        {sortedFields.length === 0 ? (
          <div className="text-center text-muted-foreground py-8 border-2 border-dashed rounded-lg">
            <p>No schema fields defined for this collection.</p>
            <p className="text-sm mt-1">You can still add data using the JSON editor.</p>
          </div>
        ) : (
          sortedFields.map(([fieldName, fieldDef]) => renderField(fieldName, fieldDef))
        )}
      </div>

      <div className="flex justify-end space-x-2 pt-4 border-t">
        <Button
          type="button"
          variant="outline"
          onClick={onCancel}
          disabled={isSubmitting}
        >
          Cancel
        </Button>
        <Button
          type="submit"
          disabled={isSubmitting}
        >
          {isSubmitting ? 'Saving...' : submitLabel}
        </Button>
      </div>
    </form>
  );
}; 