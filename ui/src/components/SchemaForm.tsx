import React, { useState, useEffect } from 'react';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Switch } from '@/components/ui/switch';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from '@/components/ui/command';
import { Check, ChevronsUpDown, X, GripVertical, Eye, EyeOff } from 'lucide-react';
import { cn } from '@/lib/utils';
import { FileUpload } from '@/components/ui/file-upload';
import { apiService } from '../services/api';
import type { CollectionSchema, FieldDefinition, DbRecord, FileReference, FileFieldConfig } from '../types/api';
import type { FieldCustomization, FieldSize } from '../types/fieldCustomization';

interface SchemaFormProps {
  schema: CollectionSchema;
  initialData?: Record<string, Record<string, unknown>>;
  onSubmit: (data: Record<string, unknown>) => Promise<void>;
  onCancel: () => void;
  submitLabel?: string;
  isSubmitting?: boolean;
  // Field customization props
  customizationMode?: boolean;
  fieldCustomizations?: Record<string, FieldCustomization>;
  onFieldCustomizationChange?: (fieldName: string, customization: Partial<FieldCustomization>) => void;
  onDragStart?: (fieldName: string) => void;
  onDragOver?: (e: React.DragEvent) => void;
  onDrop?: (e: React.DragEvent, targetFieldName: string) => void;
  draggedField?: string | null;
}

interface FormData {
  [key: string]: unknown;
}

interface RelationshipFieldProps {
  fieldName: string;
  relationshipConfig: { target_collection: string; multiple: boolean; display_field?: string };
  value: unknown;
  onChange: (value: unknown) => void;
  error?: string;
  required?: boolean;
}

const RelationshipField: React.FC<RelationshipFieldProps> = ({
  fieldName,
  relationshipConfig,
  value,
  onChange,
  error,
  required = false,
}) => {
  const [open, setOpen] = useState(false);
  const [records, setRecords] = useState<DbRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [fetchError, setFetchError] = useState<string | null>(null);

  useEffect(() => {
    fetchRecords();
  }, [relationshipConfig.target_collection]);

  const fetchRecords = async () => {
    try {
      setLoading(true);
      setFetchError(null);
      const fetchedRecords = await apiService.getRecords(relationshipConfig.target_collection);
      setRecords(fetchedRecords);
    } catch (err) {
      setFetchError(err instanceof Error ? err.message : 'Failed to fetch records');
      setRecords([]);
    } finally {
      setLoading(false);
    }
  };

  const getDisplayValue = (record: DbRecord): string => {
    if (relationshipConfig.display_field && record.data[relationshipConfig.display_field]) {
      return `${record.data[relationshipConfig.display_field]} (${record.id})`;
    }
    return record.id;
  };

  const getPreviewData = (record: DbRecord): { primary: string; secondary: string; fields: Array<{ key: string; value: any }> } => {
    const data = record.data;
    const fields = Object.entries(data)
      .filter(([key, value]) => value !== null && value !== undefined && value !== '')
      .slice(0, 4) // Limit to first 4 fields for preview
      .map(([key, value]) => ({
        key,
        value: typeof value === 'object' ? JSON.stringify(value).slice(0, 50) + '...' : String(value).slice(0, 50)
      }));

    let primary = record.id;
    let secondary = '';

    if (relationshipConfig.display_field && data[relationshipConfig.display_field]) {
      primary = String(data[relationshipConfig.display_field]);
      secondary = record.id;
    } else {
      // Try to find a meaningful field for display
      const meaningfulFields = ['name', 'title', 'label', 'email', 'username'];
      const foundField = meaningfulFields.find(field => data[field]);
      if (foundField) {
        primary = String(data[foundField]);
        secondary = record.id;
      }
    }

    return { primary, secondary, fields };
  };

  const getSelectedRecords = (): DbRecord[] => {
    if (relationshipConfig.multiple) {
      const selectedIds = Array.isArray(value) ? value : [];
      return records.filter(record => selectedIds.includes(record.id));
    } else {
      return value ? records.filter(record => record.id === value) : [];
    }
  };

  const handleSelect = (record: DbRecord) => {
    if (relationshipConfig.multiple) {
      const currentIds = Array.isArray(value) ? value : [];
      const newIds = currentIds.includes(record.id)
        ? currentIds.filter(id => id !== record.id)
        : [...currentIds, record.id];
      onChange(newIds);
    } else {
      onChange(record.id);
      setOpen(false);
    }
  };

  const handleRemove = (recordId: string) => {
    if (relationshipConfig.multiple) {
      const currentIds = Array.isArray(value) ? value : [];
      onChange(currentIds.filter(id => id !== recordId));
    } else {
      onChange('');
    }
  };

  const selectedRecords = getSelectedRecords();

  if (fetchError) {
    return (
      <div className="space-y-2">
        <div className="text-sm text-destructive">
          Error loading {relationshipConfig.target_collection} records: {fetchError}
        </div>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={fetchRecords}
        >
          Retry
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-2">
      <div className="text-sm text-muted-foreground">
        {relationshipConfig.multiple ? 'Multiple' : 'Single'} relationship to {relationshipConfig.target_collection}
      </div>
      
      {/* Selected items display */}
      {selectedRecords.length > 0 && (
        <div className="space-y-2 mb-2">
          {selectedRecords.map((record) => {
            const previewData = getPreviewData(record);
            return (
              <div
                key={record.id}
                className="flex items-center justify-between p-3 bg-muted/50 rounded-md border"
              >
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <div className="font-medium text-sm truncate">
                      {previewData.primary}
                    </div>
                    {previewData.secondary && (
                      <Badge variant="outline" className="text-xs">
                        {previewData.secondary}
                      </Badge>
                    )}
                  </div>
                  {previewData.fields.length > 0 && (
                    <div className="mt-1 flex flex-wrap gap-2 text-xs text-muted-foreground">
                      {previewData.fields.slice(0, 3).map((field, index) => (
                        <span key={index} className="truncate">
                          <span className="font-medium">{field.key}:</span> {field.value}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-8 w-8 p-0 hover:bg-destructive hover:text-destructive-foreground shrink-0 ml-2"
                  onClick={() => handleRemove(record.id)}
                >
                  <X className="h-4 w-4" />
                </Button>
              </div>
            );
          })}
        </div>
      )}

      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button
            variant="outline"
            role="combobox"
            aria-expanded={open}
            className="w-full justify-between"
            disabled={loading}
          >
            {loading ? (
              "Loading records..."
            ) : selectedRecords.length > 0 ? (
              relationshipConfig.multiple
                ? `${selectedRecords.length} selected`
                : getDisplayValue(selectedRecords[0])
            ) : (
              `Select ${relationshipConfig.target_collection} record${relationshipConfig.multiple ? 's' : ''}...`
            )}
            <ChevronsUpDown className="ml-2 h-4 w-4 shrink-0 opacity-50" />
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-full p-0">
          <Command>
            <CommandInput placeholder={`Search ${relationshipConfig.target_collection} records...`} />
            <CommandList>
              <CommandEmpty>No records found.</CommandEmpty>
              <CommandGroup>
                {records.map((record) => {
                  const isSelected = relationshipConfig.multiple
                    ? Array.isArray(value) && value.includes(record.id)
                    : value === record.id;
                  
                  const previewData = getPreviewData(record);
                  
                  return (
                    <CommandItem
                      key={record.id}
                      value={getDisplayValue(record)}
                      onSelect={() => handleSelect(record)}
                      className="flex-col items-start p-3 h-auto"
                    >
                      <div className="flex items-center w-full">
                        <Check
                          className={cn(
                            "mr-2 h-4 w-4 shrink-0",
                            isSelected ? "opacity-100" : "opacity-0"
                          )}
                        />
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center justify-between">
                            <div className="font-medium text-sm truncate">
                              {previewData.primary}
                            </div>
                            {previewData.secondary && (
                              <div className="text-xs text-muted-foreground ml-2 shrink-0">
                                {previewData.secondary}
                              </div>
                            )}
                          </div>
                          {previewData.fields.length > 0 && (
                            <div className="mt-1 grid grid-cols-2 gap-1 text-xs text-muted-foreground">
                              {previewData.fields.map((field, index) => (
                                <div key={index} className="truncate">
                                  <span className="font-medium">{field.key}:</span> {field.value}
                                </div>
                              ))}
                            </div>
                          )}
                        </div>
                      </div>
                    </CommandItem>
                  );
                })}
              </CommandGroup>
            </CommandList>
          </Command>
        </PopoverContent>
      </Popover>
      
      {error && (
        <p className="text-sm text-destructive">{error}</p>
      )}
    </div>
  );
};

export const SchemaForm: React.FC<SchemaFormProps> = ({
  schema,
  initialData = {},
  onSubmit,
  onCancel,
  submitLabel = 'Submit',
  isSubmitting = false,
  customizationMode = false,
  fieldCustomizations = {},
  onFieldCustomizationChange,
  onDragStart,
  onDragOver,
  onDrop,
  draggedField,
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
        if (typeof fieldDef.field_type === 'object' && 'file' in fieldDef.field_type) {
          // File fields should be null when empty
          data[fieldName] = null;
        } else {
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

    // Handle file field validation
    if (typeof fieldDef.field_type === 'object' && 'file' in fieldDef.field_type) {
      if (fieldDef.required && !value) {
        return `${fieldName} is required`;
      }
      
      // If value exists, validate that it's a proper FileReference
      if (value && typeof value === 'object') {
        const missingFields = [];
        if (!value.file_id) missingFields.push('file_id');
        if (!value.name) missingFields.push('name');
        if (!value.mime_type) missingFields.push('mime_type');
        if (value.size === undefined) missingFields.push('size');
        if (!value.path) missingFields.push('path');
        
        if (missingFields.length > 0) {
          return `${fieldName} contains invalid file reference: missing ${missingFields.join(', ')}. Please re-upload the file.`;
        }
      }
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
        // Check if this is a file field - preserve the FileReference object as-is
        if (typeof fieldDef.field_type === 'object' && 'file' in fieldDef.field_type) {
          processedData[fieldName] = value; // FileReference objects should be preserved
        } else {
          processedData[fieldName] = value || null;
        }
    }
    });

    await onSubmit(processedData);
  };

  const renderField = (fieldName: string, fieldDef: FieldDefinition) => {
    // Use different default values for different field types
    const defaultValue = (typeof fieldDef.field_type === 'object' && 'file' in fieldDef.field_type) ? null : '';
    const value = formData[fieldName] ?? defaultValue;
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

      default:
        if (typeof fieldDef.field_type === 'object' && 'file' in fieldDef.field_type) {
          // Handle file field type
          const fileConfig: FileFieldConfig = (fieldDef.field_type as any).file || {
            multiple: false,
            allowed_mime_types: undefined,
            max_file_size: 10 * 1024 * 1024, // 10MB default
            required: fieldDef.required || false
          };

          fieldComponent = (
            <FileUpload
              value={value}
              onChange={(newValue) => handleFieldChange(fieldName, newValue)}
              config={fileConfig}
              collection={schema.name}
              error={error}
              disabled={false}
            />
          );
        } else if (typeof fieldDef.field_type === 'object' && 'relationship' in fieldDef.field_type) {
          // Handle relationship field
          const relationshipConfig = fieldDef.field_type.relationship;
          
          fieldComponent = (
            <RelationshipField
              fieldName={fieldName}
              relationshipConfig={relationshipConfig}
              value={value}
              onChange={(newValue) => handleFieldChange(fieldName, newValue)}
              error={error}
              required={fieldDef.required}
            />
          );
        } else if (typeof fieldDef.field_type === 'object' && 'select' in fieldDef.field_type) {
          // Handle select field
          const selectConfig = fieldDef.field_type.select;
          
          if (selectConfig.multiple) {
            // Multiple select (array of values)
            const selectedValues = Array.isArray(value) ? value : [];
            fieldComponent = (
              <div className="space-y-2">
                {selectConfig.options.map((option) => (
                  <div key={option} className="flex items-center space-x-2">
                    <input
                      type="checkbox"
                      id={`${fieldId}-${option}`}
                      checked={selectedValues.includes(option)}
                      onChange={(e) => {
                        const newValues = e.target.checked
                          ? [...selectedValues, option]
                          : selectedValues.filter(v => v !== option);
                        handleFieldChange(fieldName, newValues);
                      }}
                      className="h-4 w-4 rounded border border-input bg-background text-primary focus:ring-2 focus:ring-ring focus:ring-offset-2"
                    />
                    <Label htmlFor={`${fieldId}-${option}`}>{option}</Label>
                  </div>
                ))}
              </div>
            );
          } else {
            // Single select (dropdown)
            fieldComponent = (
              <select
                id={fieldId}
                value={value || ''}
                onChange={(e) => handleFieldChange(fieldName, e.target.value || null)}
                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {selectConfig.allow_empty && (
                  <option value="">-- Select an option --</option>
                )}
                {selectConfig.options.map((option) => (
                  <option key={option} value={option}>
                    {option}
                  </option>
                ))}
              </select>
            );
          }
        } else {
          // Default text field
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
            {typeof fieldDef.field_type === 'string' 
              ? fieldDef.field_type 
              : 'relationship' in fieldDef.field_type 
                ? 'relationship'
                : 'file' in fieldDef.field_type
                  ? 'file'
                  : 'select' in fieldDef.field_type
                    ? 'select'
                    : 'unknown'}
          </Badge>
        </div>
        {fieldComponent}
        {error && (
          <div className="space-y-2">
            <p className="text-sm text-destructive">{error}</p>
            {error.includes('contains invalid file reference') && (
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => handleFieldChange(fieldName, null)}
                className="text-xs"
              >
                Clear Invalid File
              </Button>
            )}
          </div>
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