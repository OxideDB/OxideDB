import React, { useState, useEffect } from 'react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Switch } from '@/components/ui/switch';
import { GripVertical, Eye, EyeOff, RotateCcw } from 'lucide-react';
import { cn } from '@/lib/utils';
import type { CollectionSchema, FieldDefinition, FileFieldConfig, FileReference } from '../types/api';
import type { FieldCustomization, FieldSize } from '../types/fieldCustomization';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { FileUpload } from '@/components/ui/file-upload';

interface CustomizableSchemaFormProps {
  schema: CollectionSchema;
  initialData?: Record<string, unknown>;
  onSubmit: (data: Record<string, unknown>) => Promise<void>;
  onCancel: () => void;
  submitLabel?: string;
  isSubmitting?: boolean;
  // Customization props
  customizationMode: boolean;
  fieldCustomizations: Record<string, FieldCustomization>;
  onFieldCustomizationChange: (fieldName: string, customization: Partial<FieldCustomization>) => void;
  onDragStart: (fieldName: string) => void;
  onDragOver: (e: React.DragEvent) => void;
  onDrop: (e: React.DragEvent, targetFieldName: string) => void;
  draggedField: string | null;
  onToggleCustomizationMode: () => void;
  onResetToDefault: () => void;
  onShowAllFields: () => void;
  onHideAllFields: () => void;
}

const getFieldSizeClass = (size: FieldSize): string => {
  switch (size) {
    case 'full':
      return 'col-span-full';
    case 'half':
      return 'col-span-6';
    case 'third':
      return 'col-span-4';
    case 'quarter':
      return 'col-span-3';
    default:
      return 'col-span-full';
  }
};

const getDefaultFieldValue = (fieldDef: FieldDefinition): unknown => {
  if (fieldDef.default !== undefined && fieldDef.default !== null) {
    return fieldDef.default;
  }

  if (typeof fieldDef.field_type === 'object' && 'file' in fieldDef.field_type) {
    return null;
  }

  switch (fieldDef.field_type) {
    case 'boolean':
      return false;
    case 'json':
      return '{}';
    default:
      return '';
  }
};

const FieldCustomizationControls: React.FC<{
  fieldName: string;
  customization: FieldCustomization;
  onCustomizationChange: (fieldName: string, customization: Partial<FieldCustomization>) => void;
}> = ({ fieldName, customization, onCustomizationChange }) => {
  const handleSizeChange = (newSize: FieldSize) => {
    onCustomizationChange(fieldName, { size: newSize });
  };

  const toggleVisibility = () => {
    onCustomizationChange(fieldName, { visible: !customization.visible });
  };

  return (
    <div className="flex items-center gap-2 p-2 bg-muted/50 rounded border">
      <div className="h-8 w-8 flex items-center justify-center text-muted-foreground">
        <GripVertical className="h-4 w-4" />
      </div>
      
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className="h-8 w-8 p-0"
        onClick={toggleVisibility}
      >
        {customization.visible ? (
          <Eye className="h-4 w-4" />
        ) : (
          <EyeOff className="h-4 w-4 text-muted-foreground" />
        )}
      </Button>

      <Select value={customization.size} onValueChange={handleSizeChange}>
        <SelectTrigger className="h-8 w-20">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="full">Full</SelectItem>
          <SelectItem value="half">Half</SelectItem>
          <SelectItem value="third">Third</SelectItem>
          <SelectItem value="quarter">Quarter</SelectItem>
        </SelectContent>
      </Select>

      <Badge variant="outline" className="text-xs">
        Order: {customization.order}
      </Badge>
      
      <span className="text-sm font-medium text-muted-foreground ml-2">
        {fieldName}
      </span>
    </div>
  );
};

const CustomizableField: React.FC<{
  fieldName: string;
  fieldDef: FieldDefinition;
  customization: FieldCustomization;
  children: React.ReactNode;
  onDragStart: (fieldName: string) => void;
  onDragOver: (e: React.DragEvent) => void;
  onDrop: (e: React.DragEvent, targetFieldName: string) => void;
  draggedField: string | null;
  customizationMode: boolean;
  onCustomizationChange: (fieldName: string, customization: Partial<FieldCustomization>) => void;
}> = ({
  fieldName,
  customization,
  children,
  onDragStart,
  onDragOver,
  onDrop,
  draggedField,
  customizationMode,
  onCustomizationChange,
}) => {
  const isDragging = draggedField === fieldName;
  const sizeClass = getFieldSizeClass(customization.size);
  const [isDragOver, setIsDragOver] = useState(false);

  const handleDragStart = (e: React.DragEvent) => {
    e.dataTransfer.setData('text/plain', fieldName);
    e.dataTransfer.effectAllowed = 'move';
    onDragStart(fieldName);
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    if (draggedField && draggedField !== fieldName) {
      setIsDragOver(true);
    }
    onDragOver(e);
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
    onDrop(e, fieldName);
  };

  if (!customization.visible && !customizationMode) {
    return null;
  }

  return (
    <div
      className={cn(
        sizeClass,
        'space-y-3',
        customizationMode && 'border rounded-lg p-3 transition-all duration-200',
        isDragging && 'opacity-50 scale-95',
        isDragOver && 'border-primary bg-primary/5 shadow-lg',
        !customization.visible && customizationMode && 'bg-muted/30 border-dashed',
        customizationMode && 'cursor-grab active:cursor-grabbing hover:shadow-md hover:border-primary/50',
        customizationMode && !isDragging && 'hover:bg-muted/20'
      )}
      draggable={customizationMode}
      onDragStart={handleDragStart}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      {customizationMode && (
        <FieldCustomizationControls
          fieldName={fieldName}
          customization={customization}
          onCustomizationChange={onCustomizationChange}
        />
      )}
      
      {(customization.visible || customizationMode) && (
        <div className={cn(!customization.visible && customizationMode && 'opacity-50')}>
          {children}
        </div>
      )}
    </div>
  );
};

export const CustomizableSchemaForm: React.FC<CustomizableSchemaFormProps> = ({
  schema,
  initialData,
  onSubmit,
  onCancel,
  submitLabel,
  isSubmitting,
  customizationMode,
  fieldCustomizations,
  onFieldCustomizationChange,
  onDragStart,
  onDragOver,
  onDrop,
  draggedField,
  onToggleCustomizationMode,
  onResetToDefault,
  onShowAllFields,
  onHideAllFields,
}) => {
  // Form state hooks - must be at top level
  const [formData, setFormData] = useState<Record<string, unknown>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});

  // Initialize form data
  useEffect(() => {
    const data: Record<string, unknown> = {};

    Object.entries(schema.fields).forEach(([fieldName, fieldDef]) => {
      if (initialData && initialData[fieldName] !== undefined) {
        data[fieldName] = initialData[fieldName];
      } else {
        data[fieldName] = getDefaultFieldValue(fieldDef);
      }
    });

    setFormData(data);
  }, [schema, initialData]);

  // Get ordered fields based on customization settings
  const getOrderedFields = () => {
    return Object.entries(schema.fields)
      .map(([fieldName, fieldDef]) => ({
        fieldName,
        fieldDef,
        customization: fieldCustomizations[fieldName] || {
          visible: true,
          order: 0,
          size: 'full' as FieldSize
        }
      }))
      .sort((a, b) => a.customization.order - b.customization.order);
  };

  const orderedFields = getOrderedFields();
  const visibleFieldsCount = orderedFields.filter(f => f.customization.visible).length;
  const totalFieldsCount = orderedFields.length;

  if (customizationMode) {
    return (
      <div className="space-y-6">
        {/* Customization Header */}
        <div className="bg-blue-50 border border-blue-200 rounded-lg p-4">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h3 className="font-semibold text-blue-900">Field Customization Mode</h3>
              <p className="text-sm text-blue-700">
                Drag fields to reorder, toggle visibility, and adjust sizes. Changes are saved automatically.
              </p>
            </div>
            <Button
              type="button"
              variant="outline"
              onClick={onToggleCustomizationMode}
            >
              Exit Customization
            </Button>
          </div>
          
          <div className="flex items-center gap-2 text-sm text-blue-700">
            <span>
              Showing {visibleFieldsCount} of {totalFieldsCount} fields
            </span>
            <span>•</span>
            <Button
              type="button"
              variant="link"
              size="sm"
              className="h-auto p-0 text-blue-700"
              onClick={onShowAllFields}
            >
              Show All
            </Button>
            <span>•</span>
            <Button
              type="button"
              variant="link"
              size="sm"
              className="h-auto p-0 text-blue-700"
              onClick={onHideAllFields}
            >
              Hide All
            </Button>
            <span>•</span>
            <Button
              type="button"
              variant="link"
              size="sm"
              className="h-auto p-0 text-blue-700"
              onClick={onResetToDefault}
            >
              <RotateCcw className="h-3 w-3 mr-1" />
              Reset
            </Button>
          </div>
        </div>

        {/* Fields Grid in Customization Mode */}
        <div className="grid grid-cols-12 gap-4">
          {orderedFields.map(({ fieldName, fieldDef, customization }) => (
            <CustomizableField
              key={fieldName}
              fieldName={fieldName}
              fieldDef={fieldDef}
              customization={customization}
              onDragStart={onDragStart}
              onDragOver={onDragOver}
              onDrop={onDrop}
              draggedField={draggedField}
              customizationMode={customizationMode}
              onCustomizationChange={onFieldCustomizationChange}
            >
              <div className="space-y-2">
                <div className="flex items-center space-x-2">
                  <span className="font-medium text-sm">{fieldName}</span>
                  {fieldDef.required && (
                    <Badge variant="destructive" className="text-xs">
                      required
                    </Badge>
                  )}
                  <Badge variant="outline" className="text-xs">
                    {typeof fieldDef.field_type === 'string' 
                      ? fieldDef.field_type 
                      : 'complex'}
                  </Badge>
                </div>
                <div className="h-10 bg-muted rounded border border-dashed flex items-center justify-center text-xs text-muted-foreground">
                  Field Preview
                </div>
              </div>
            </CustomizableField>
          ))}
        </div>
      </div>
    );
  }

  // Normal form mode - render as a customized form

  const handleFieldChange = (fieldName: string, value: unknown) => {
    setFormData(prev => ({
      ...prev,
      [fieldName]: value
    }));
    
    // Clear error when field is changed
    if (errors[fieldName]) {
      setErrors(prev => ({
        ...prev,
        [fieldName]: ''
      }));
    }
  };

  const validateForm = (): boolean => {
    const newErrors: Record<string, string> = {};
    
    orderedFields
      .filter(({ customization }) => customization.visible)
      .forEach(({ fieldName, fieldDef }) => {
        const value = formData[fieldName];
        
        if (fieldDef.required && (value === undefined || value === null || value === '')) {
          newErrors[fieldName] = `${fieldName} is required`;
        }
      });
    
    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!validateForm()) {
      return;
    }
    
    // Process form data according to field types
    const processedData: Record<string, unknown> = {};
    
    orderedFields
      .filter(({ customization }) => customization.visible)
      .forEach(({ fieldName, fieldDef }) => {
        const value = formData[fieldName];
        
        switch (fieldDef.field_type) {
          case 'number':
            processedData[fieldName] = value ? Number(value) : null;
            break;
          case 'boolean':
            processedData[fieldName] = Boolean(value);
            break;
          case 'json':
            try {
              processedData[fieldName] = value ? JSON.parse(value as string) : null;
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
            value={typeof value === 'object' ? JSON.stringify(value, null, 2) : value as string}
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
            value={value as string}
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
            value={value as string}
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
            value={value as string}
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
            value={value as string}
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
            value={value as string}
            onChange={(e) => handleFieldChange(fieldName, e.target.value)}
            placeholder="Enter password"
          />
        );
        break;

      default:
        if (typeof fieldDef.field_type === 'object' && 'file' in fieldDef.field_type) {
          const fileConfig: FileFieldConfig = ((fieldDef.field_type as unknown) as { file: FileFieldConfig }).file || {
            multiple: false,
            allowed_mime_types: undefined,
            max_file_size: 10 * 1024 * 1024,
            required: fieldDef.required || false
          };

          fieldComponent = (
            <FileUpload
              value={value as FileReference | FileReference[] | null}
              onChange={(newValue) => handleFieldChange(fieldName, newValue)}
              config={fileConfig}
              collection={schema.name}
              error={error}
              disabled={false}
            />
          );
        } else if (typeof fieldDef.field_type === 'object' && 'select' in fieldDef.field_type) {
          const selectConfig = fieldDef.field_type.select;
          
          if (selectConfig.multiple) {
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
                      className="h-4 w-4"
                    />
                    <Label htmlFor={`${fieldId}-${option}`}>{option}</Label>
                  </div>
                ))}
              </div>
            );
          } else {
            fieldComponent = (
              <select
                id={fieldId}
                value={value as string || ''}
                onChange={(e) => handleFieldChange(fieldName, e.target.value || null)}
                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
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
          // Default to text input
          fieldComponent = (
            <Input
              id={fieldId}
              type="text"
              value={value as string}
              onChange={(e) => handleFieldChange(fieldName, e.target.value)}
              placeholder={`Enter ${fieldName}`}
            />
          );
        }
    }

    return (
      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Label htmlFor={fieldId} className="text-sm font-medium">
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
              : 'complex'}
          </Badge>
        </div>
        {fieldComponent}
        {error && (
          <p className="text-sm text-destructive">{error}</p>
        )}
      </div>
    );
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-6">
      <div className="grid grid-cols-12 gap-4">
        {orderedFields
          .filter(({ customization }) => customization.visible)
          .map(({ fieldName, fieldDef, customization }) => (
            <div key={fieldName} className={getFieldSizeClass(customization.size)}>
              {renderField(fieldName, fieldDef)}
            </div>
          ))
        }
      </div>

      <div className="flex items-center space-x-4 pt-6 border-t">
        <Button
          type="submit"
          disabled={isSubmitting}
        >
          {isSubmitting ? 'Saving...' : submitLabel}
        </Button>
        <Button
          type="button"
          variant="outline"
          onClick={onCancel}
          disabled={isSubmitting}
        >
          Cancel
        </Button>
      </div>
    </form>
  );
};
