import React from 'react';
import { X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
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

  return (
    <Card className="p-4">
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="space-y-2">
          <Label htmlFor={`field-name-${index}`}>Field Name</Label>
          <Input
            id={`field-name-${index}`}
            value={field.name}
            onChange={(e) => onUpdate(index, { name: e.target.value })}
            placeholder="field_name"
            required
          />
        </div>

        <div className="space-y-2">
          <Label htmlFor={`field-type-${index}`}>Type</Label>
          <select 
            id={`field-type-${index}`}
            value={fieldTypeString} 
            onChange={(e) => onUpdateType(index, e.target.value)}
            className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <option value="text">Text</option>
            <option value="number">Number</option>
            <option value="boolean">Boolean</option>
            <option value="date">Date</option>
            <option value="json">JSON</option>
            <option value="email">Email</option>
            <option value="url">URL</option>
            <option value="password">Password</option>
            <option value="relationship">Relationship</option>
            <option value="file">File</option>
            <option value="select">Select</option>
          </select>
        </div>

        <div className="space-y-2">
          <Label htmlFor={`field-default-${index}`}>Default Value</Label>
          <Input
            id={`field-default-${index}`}
            value={field.default || ''}
            onChange={(e) => onUpdate(index, { default: e.target.value })}
            placeholder="JSON value"
          />
        </div>

        {/* Field Type Configurations */}
        {fieldTypeString === 'relationship' && (
          <RelationshipConfigComponent
            index={index}
            config={field.relationshipConfig}
            collections={collections}
            onUpdate={onUpdateRelationshipConfig}
          />
        )}

        {fieldTypeString === 'file' && (
          <FileConfigComponent
            index={index}
            config={field.fileConfig}
            onUpdate={onUpdateFileConfig}
          />
        )}

        {fieldTypeString === 'select' && (
          <SelectConfigComponent
            index={index}
            config={field.selectConfig}
            onUpdate={onUpdateSelectConfig}
          />
        )}

        {/* Validation Configuration - shown for applicable field types */}
        {['text', 'email', 'url', 'password', 'number'].includes(fieldTypeString) && (
          <ValidationConfigComponent
            index={index}
            fieldType={fieldTypeString}
            config={field.validation}
            onUpdate={onUpdateValidationConfig}
          />
        )}

        <div className="space-y-2">
          <div className="flex items-center space-x-4">
            <div className="flex items-center space-x-2">
              <input
                type="checkbox"
                id={`field-required-${index}`}
                checked={field.required}
                onChange={(e) => onUpdate(index, { required: e.target.checked })}
                className="h-4 w-4 rounded border border-input bg-background text-primary focus:ring-2 focus:ring-ring focus:ring-offset-2"
              />
              <Label htmlFor={`field-required-${index}`}>Required</Label>
            </div>
            <div className="flex items-center space-x-2">
              <input
                type="checkbox"
                id={`field-unique-${index}`}
                checked={field.unique}
                onChange={(e) => onUpdate(index, { unique: e.target.checked })}
                className="h-4 w-4 rounded border border-input bg-background text-primary focus:ring-2 focus:ring-ring focus:ring-offset-2"
              />
              <Label htmlFor={`field-unique-${index}`}>Unique</Label>
            </div>
          </div>
          <div className="flex justify-end">
            <Button
              type="button"
              onClick={() => onRemove(index)}
              size="sm"
              variant="ghost"
              className="text-destructive hover:text-destructive/80"
            >
              <X className="h-4 w-4" />
            </Button>
          </div>
        </div>
      </div>
    </Card>
  );
};

export default FieldItem; 