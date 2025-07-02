import React from 'react';
import { Label } from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import { Checkbox } from '@/components/ui/checkbox';
import { Badge } from '@/components/ui/badge';
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
    <div className="space-y-4">
      <div className="space-y-3">
        <div className="space-y-2">
          <Label htmlFor={`field-target-${index}`} className="text-sm font-medium">
            Target Collection*
          </Label>
          <select
            id={`field-target-${index}`}
            value={config?.target_collection || ''}
            onChange={(e) => onUpdate(index, { target_collection: e.target.value })}
            className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
            required
          >
            <option value="">Choose a collection to link to...</option>
            {collections.map((col) => (
              <option key={col.id} value={col.name}>
                {col.name} {col.collection_type === 'auth' && '(Auth)'}
              </option>
            ))}
          </select>
          <p className="text-xs text-muted-foreground">
            The collection this field will reference
          </p>
        </div>
        
        <div className="space-y-2">
          <Label htmlFor={`field-display-${index}`} className="text-sm font-medium">
            Display Field
          </Label>
          <Input
            id={`field-display-${index}`}
            value={config?.display_field || ''}
            onChange={(e) => onUpdate(index, { display_field: e.target.value || undefined })}
            placeholder="e.g., name, title, email"
            className="text-sm"
          />
          <p className="text-xs text-muted-foreground">
            Field to show in dropdowns (leave empty to use ID)
          </p>
        </div>

        <div className="space-y-3">
          <Label className="text-sm font-medium">Relationship Options</Label>
          <div className="space-y-3">
            <div className="flex items-center space-x-2">
              <Checkbox
                id={`field-multiple-${index}`}
                checked={config?.multiple || false}
                onCheckedChange={(checked) => onUpdate(index, { multiple: !!checked })}
              />
              <div className="space-y-1">
                <Label htmlFor={`field-multiple-${index}`} className="text-sm font-normal">
                  Allow multiple selections
                </Label>
                <p className="text-xs text-muted-foreground">
                  Enable selecting multiple related records
                </p>
              </div>
            </div>
            
            <div className="flex items-center space-x-2">
              <Checkbox
                id={`field-cascade-${index}`}
                checked={config?.cascade_delete || false}
                onCheckedChange={(checked) => onUpdate(index, { cascade_delete: !!checked })}
              />
              <div className="space-y-1">
                <Label htmlFor={`field-cascade-${index}`} className="text-sm font-normal flex items-center space-x-2">
                  <span>Cascade delete</span>
                  <Badge variant="destructive" className="text-xs">Caution</Badge>
                </Label>
                <p className="text-xs text-muted-foreground">
                  Delete this record when the related record is deleted
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default RelationshipConfig; 