import React from 'react';
import { Label } from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import type { CollectionSchema } from '../../types/api';
import type { RelationshipConfig as RelationshipConfigType } from './types';

interface RelationshipConfigProps {
  index: number;
  config: RelationshipConfigType | undefined;
  collections: CollectionSchema[];
  onUpdate: (index: number, config: Partial<RelationshipConfigType>) => void;
}

const RelationshipConfig: React.FC<RelationshipConfigProps> = ({
  index,
  config,
  collections,
  onUpdate,
}) => {
  return (
    <div className="col-span-full space-y-4 p-4 border rounded-md bg-muted/50">
      <h4 className="font-medium text-sm">Relationship Configuration</h4>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        <div className="space-y-2">
          <Label htmlFor={`field-target-${index}`}>Target Collection</Label>
          <select
            id={`field-target-${index}`}
            value={config?.target_collection || ''}
            onChange={(e) => onUpdate(index, { target_collection: e.target.value })}
            className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
          >
            <option value="">Select collection...</option>
            {collections.map((col) => (
              <option key={col.id} value={col.name}>
                {col.name}
              </option>
            ))}
          </select>
        </div>
        
        <div className="space-y-2">
          <Label htmlFor={`field-display-${index}`}>Display Field (optional)</Label>
          <Input
            id={`field-display-${index}`}
            value={config?.display_field || ''}
            onChange={(e) => onUpdate(index, { display_field: e.target.value || undefined })}
            placeholder="name, title, etc."
          />
        </div>

        <div className="space-y-2">
          <Label>Relationship Options</Label>
          <div className="space-y-2">
            <div className="flex items-center space-x-2">
              <input
                type="checkbox"
                id={`field-multiple-${index}`}
                checked={config?.multiple || false}
                onChange={(e) => onUpdate(index, { multiple: e.target.checked })}
                className="h-4 w-4 rounded border border-input bg-background text-primary focus:ring-2 focus:ring-ring focus:ring-offset-2"
              />
              <Label htmlFor={`field-multiple-${index}`}>Multiple (many-to-many)</Label>
            </div>
            <div className="flex items-center space-x-2">
              <input
                type="checkbox"
                id={`field-cascade-${index}`}
                checked={config?.cascade_delete || false}
                onChange={(e) => onUpdate(index, { cascade_delete: e.target.checked })}
                className="h-4 w-4 rounded border border-input bg-background text-primary focus:ring-2 focus:ring-ring focus:ring-offset-2"
              />
              <Label htmlFor={`field-cascade-${index}`}>Cascade Delete</Label>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default RelationshipConfig; 