import React from 'react';
import { Plus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import type { CollectionSchema } from '../../types/api';
import type { FieldFormData, RelationshipConfig, FileConfig, SelectConfig } from './types';
import FieldItem from './FieldItem';

interface FieldsListProps {
  fields: FieldFormData[];
  collections: CollectionSchema[];
  onAddField: () => void;
  onUpdateField: (index: number, field: Partial<FieldFormData>) => void;
  onUpdateFieldType: (index: number, newType: string) => void;
  onUpdateRelationshipConfig: (index: number, config: Partial<RelationshipConfig>) => void;
  onUpdateFileConfig: (index: number, config: Partial<FileConfig>) => void;
  onUpdateSelectConfig: (index: number, config: Partial<SelectConfig>) => void;
  onRemoveField: (index: number) => void;
}

const FieldsList: React.FC<FieldsListProps> = ({
  fields,
  collections,
  onAddField,
  onUpdateField,
  onUpdateFieldType,
  onUpdateRelationshipConfig,
  onUpdateFileConfig,
  onUpdateSelectConfig,
  onRemoveField,
}) => {
  return (
    <div className="space-y-4">
      <div className="flex justify-between items-center">
        <Label>Schema Fields</Label>
        <Button type="button" onClick={onAddField} size="sm" variant="outline">
          <Plus className="h-4 w-4 mr-2" />
          Add Field
        </Button>
      </div>

      {fields.length === 0 && (
        <div className="text-center text-muted-foreground py-4 border-2 border-dashed rounded-lg">
          No fields defined. Click "Add Field" to start building your schema.
        </div>
      )}

      {fields.map((field, index) => (
        <FieldItem
          key={index}
          field={field}
          index={index}
          collections={collections}
          onUpdate={onUpdateField}
          onUpdateType={onUpdateFieldType}
          onUpdateRelationshipConfig={onUpdateRelationshipConfig}
          onUpdateFileConfig={onUpdateFileConfig}
          onUpdateSelectConfig={onUpdateSelectConfig}
          onRemove={onRemoveField}
        />
      ))}
    </div>
  );
};

export default FieldsList; 