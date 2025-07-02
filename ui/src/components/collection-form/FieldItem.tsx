import React, { useState } from 'react';
import { X, ChevronDown, ChevronRight, Settings, Database, FileText, List, Lock } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Checkbox } from '@/components/ui/checkbox';
import { Badge } from '@/components/ui/badge';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';
import { Separator } from '@/components/ui/separator';
import type { CollectionSchema } from '../../types/api';
import type { FieldFormData, RelationshipConfig, FileConfig, SelectConfig, ValidationConfig } from './types';
import { getFieldTypeString } from './fieldUtils';
import RelationshipConfigComponent from './RelationshipConfig';
import FileConfigComponent from './FileConfig';
import SelectConfigComponent from './SelectConfig';
import ValidationConfigComponent from './ValidationConfig';

interface FieldItemProps {
  field: FieldFormData;
  index: number;
  collections: CollectionSchema[];
  onUpdate: (index: number, field: Partial<FieldFormData>) => void;
  onUpdateType: (index: number, newType: string) => void;
  onUpdateRelationshipConfig: (index: number, config: Partial<RelationshipConfig>) => void;
  onUpdateFileConfig: (index: number, config: Partial<FileConfig>) => void;
  onUpdateSelectConfig: (index: number, config: Partial<SelectConfig>) => void;
  onUpdateValidationConfig: (index: number, config: Partial<ValidationConfig>) => void;
  onRemove: (index: number) => void;
}

const FieldItem: React.FC<FieldItemProps> = ({
  field,
  index,
  collections,
  onUpdate,
  onUpdateType,
  onUpdateRelationshipConfig,
  onUpdateFileConfig,
  onUpdateSelectConfig,
  onUpdateValidationConfig,
  onRemove,
}) => {
  const fieldTypeString = getFieldTypeString(field.field_type);
  const [showAdvanced, setShowAdvanced] = useState(false);

  const getFieldTypeIcon = (type: string) => {
    switch (type) {
      case 'relationship': return <Database className="h-4 w-4" />;
      case 'file': return <FileText className="h-4 w-4" />;
      case 'select': return <List className="h-4 w-4" />;
      case 'password': return <Lock className="h-4 w-4" />;
      default: return null;
    }
  };

  const getFieldTypeLabel = (type: string) => {
    const labels: Record<string, string> = {
      text: 'Text',
      number: 'Number',
      boolean: 'Boolean',
      date: 'Date',
      json: 'JSON',
      email: 'Email',
      url: 'URL',
      password: 'Password',
      relationship: 'Relationship',
      file: 'File',
      select: 'Select'
    };
    return labels[type] || type;
  };

  const hasAdvancedFeatures = () => {
    return fieldTypeString === 'relationship' || 
           fieldTypeString === 'file' || 
           ['text', 'email', 'url', 'password', 'number'].includes(fieldTypeString);
  };

  const hasEssentialConfiguration = () => {
    return fieldTypeString === 'select';
  };

  return (
    <Card className="p-4 hover:shadow-md transition-shadow">
      {/* Main Field Configuration */}
      <div className="space-y-4">
        
        {/* Header with field name and type */}
        <div className="flex items-start justify-between">
          <div className="flex-1 space-y-3">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div className="space-y-2">
                <Label htmlFor={`field-name-${index}`} className="text-sm font-medium">
                  Field Name*
                </Label>
                <Input
                  id={`field-name-${index}`}
                  value={field.name}
                  onChange={(e) => onUpdate(index, { name: e.target.value })}
                  placeholder="e.g., name, email, age"
                  required
                  className="text-base"
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor={`field-type-${index}`} className="text-sm font-medium">
                  Field Type*
                </Label>
                <div className="relative">
                  <select 
                    id={`field-type-${index}`}
                    value={fieldTypeString} 
                    onChange={(e) => onUpdateType(index, e.target.value)}
                    className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 pr-8"
                  >
                    <optgroup label="Basic Types">
                      <option value="text">Text</option>
                      <option value="number">Number</option>
                      <option value="boolean">Boolean</option>
                      <option value="date">Date</option>
                    </optgroup>
                    <optgroup label="Special Types">
                      <option value="email">Email</option>
                      <option value="url">URL</option>
                      <option value="password">Password</option>
                      <option value="json">JSON</option>
                    </optgroup>
                    <optgroup label="Advanced Types">
                      <option value="relationship">Relationship</option>
                      <option value="file">File</option>
                      <option value="select">Select</option>
                    </optgroup>
                  </select>
                </div>
              </div>
            </div>

            {/* Field Options */}
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-4">
                <div className="flex items-center space-x-2">
                  <Checkbox
                    id={`field-required-${index}`}
                    checked={field.required}
                    onCheckedChange={(checked) => onUpdate(index, { required: !!checked })}
                  />
                  <Label htmlFor={`field-required-${index}`} className="text-sm font-normal">
                    Required
                  </Label>
                </div>
                <div className="flex items-center space-x-2">
                  <Checkbox
                    id={`field-unique-${index}`}
                    checked={field.unique}
                    onCheckedChange={(checked) => onUpdate(index, { unique: !!checked })}
                  />
                  <Label htmlFor={`field-unique-${index}`} className="text-sm font-normal">
                    Unique
                  </Label>
                </div>
              </div>

              <div className="flex items-center space-x-2">
                {field.required && <Badge variant="destructive" className="text-xs">Required</Badge>}
                {field.unique && <Badge variant="secondary" className="text-xs">Unique</Badge>}
                {getFieldTypeIcon(fieldTypeString) && (
                  <Badge variant="outline" className="text-xs flex items-center space-x-1">
                    {getFieldTypeIcon(fieldTypeString)}
                    <span>{getFieldTypeLabel(fieldTypeString)}</span>
                  </Badge>
                )}
              </div>
            </div>
          </div>

          {/* Remove Button */}
          <Button
            type="button"
            onClick={() => onRemove(index)}
            size="sm"
            variant="ghost"
            className="text-destructive hover:text-destructive/80 ml-2"
            aria-label="Remove field"
          >
            <X className="h-4 w-4" />
          </Button>
        </div>

        {/* Essential Configuration (Always Visible) */}
        {hasEssentialConfiguration() && (
          <div className="space-y-4 p-4 bg-muted/30 rounded-md border">
            {fieldTypeString === 'select' && (
              <div className="space-y-3">
                <h4 className="text-sm font-medium flex items-center space-x-2">
                  <List className="h-4 w-4" />
                  <span>Select Options*</span>
                </h4>
                <SelectConfigComponent
                  index={index}
                  config={field.selectConfig}
                  onUpdate={onUpdateSelectConfig}
                />
              </div>
            )}
          </div>
        )}

        {/* Advanced Configuration (Collapsible) */}
        {hasAdvancedFeatures() && (
          <Collapsible open={showAdvanced} onOpenChange={setShowAdvanced}>
            <div className="mt-4">
              <Separator />
              <CollapsibleTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="w-full justify-between mt-3 p-2 hover:bg-muted/50"
                >
                  <div className="flex items-center space-x-2">
                    <Settings className="h-4 w-4" />
                    <span>Advanced Settings</span>
                    <Badge variant="secondary" className="text-xs">Optional</Badge>
                  </div>
                  {showAdvanced ? (
                    <ChevronDown className="h-4 w-4" />
                  ) : (
                    <ChevronRight className="h-4 w-4" />
                  )}
                </Button>
              </CollapsibleTrigger>
              
              <CollapsibleContent className="mt-3 space-y-4 p-4 bg-muted/30 rounded-md">
                
                {/* Default Value */}
                <div className="space-y-2">
                  <Label htmlFor={`field-default-${index}`} className="text-sm font-medium">
                    Default Value
                  </Label>
                  <Input
                    id={`field-default-${index}`}
                    value={field.default || ''}
                    onChange={(e) => onUpdate(index, { default: e.target.value })}
                    placeholder="Optional default value"
                    className="text-sm"
                  />
                  <p className="text-xs text-muted-foreground">
                    Leave empty for no default value
                  </p>
                </div>

                {/* Type-specific configurations */}
                {fieldTypeString === 'relationship' && (
                  <div className="space-y-3">
                    <h4 className="text-sm font-medium flex items-center space-x-2">
                      <Database className="h-4 w-4" />
                      <span>Relationship Configuration</span>
                    </h4>
                    <RelationshipConfigComponent
                      index={index}
                      config={field.relationshipConfig}
                      collections={collections}
                      onUpdate={onUpdateRelationshipConfig}
                    />
                  </div>
                )}

                {fieldTypeString === 'file' && (
                  <div className="space-y-3">
                    <h4 className="text-sm font-medium flex items-center space-x-2">
                      <FileText className="h-4 w-4" />
                      <span>File Upload Configuration</span>
                    </h4>
                    <FileConfigComponent
                      index={index}
                      config={field.fileConfig}
                      onUpdate={onUpdateFileConfig}
                    />
                  </div>
                )}

                {/* Validation Configuration */}
                {['text', 'email', 'url', 'password', 'number'].includes(fieldTypeString) && (
                  <div className="space-y-3">
                    <h4 className="text-sm font-medium flex items-center space-x-2">
                      <Settings className="h-4 w-4" />
                      <span>Validation Rules</span>
                    </h4>
                    <ValidationConfigComponent
                      index={index}
                      fieldType={fieldTypeString}
                      config={field.validation}
                      onUpdate={onUpdateValidationConfig}
                    />
                  </div>
                )}
              </CollapsibleContent>
            </div>
          </Collapsible>
        )}

        {/* Quick Summary for simple fields */}
        {!hasAdvancedFeatures() && !hasEssentialConfiguration() && field.default && (
          <div className="pt-2 border-t">
            <p className="text-xs text-muted-foreground">
              Default: <span className="font-mono">{field.default}</span>
            </p>
          </div>
        )}
      </div>
    </Card>
  );
};

export default FieldItem; 