import React from 'react';
import { Plus, Database, FileText, List, Lock, Type, Hash, Calendar, Mail } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import type { CollectionSchema } from '../../types/api';
import type { FieldFormData, RelationshipConfig, FileConfig, SelectConfig, ValidationConfig } from './types';
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
  onUpdateValidationConfig: (index: number, config: Partial<ValidationConfig>) => void;
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
  onUpdateValidationConfig,
  onRemoveField,
}) => {
  const getFieldTypeIcon = (field: FieldFormData) => {
    const type = typeof field.field_type === 'string' ? field.field_type : Object.keys(field.field_type)[0];
    switch (type) {
      case 'text': return <Type className="h-3 w-3" />;
      case 'number': return <Hash className="h-3 w-3" />;
      case 'email': return <Mail className="h-3 w-3" />;
      case 'date': return <Calendar className="h-3 w-3" />;
      case 'password': return <Lock className="h-3 w-3" />;
      case 'relationship': return <Database className="h-3 w-3" />;
      case 'file': return <FileText className="h-3 w-3" />;
      case 'select': return <List className="h-3 w-3" />;
      default: return <Type className="h-3 w-3" />;
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div className="space-y-1">
          <Label className="text-base font-medium">Schema Fields</Label>
          <p className="text-sm text-muted-foreground">
            Define the structure of your data with custom fields
          </p>
        </div>
        <Button 
          type="button" 
          onClick={onAddField} 
          size="sm" 
          className="w-fit"
        >
          <Plus className="h-4 w-4 mr-2" />
          Add Field
        </Button>
      </div>

      {/* Field Count Summary */}
      {fields.length > 0 && (
        <div className="flex items-center justify-between p-3 bg-muted/50 rounded-lg">
          <div className="flex items-center space-x-4">
            <div className="flex items-center space-x-2">
              <Badge variant="outline">
                {fields.length} field{fields.length !== 1 ? 's' : ''}
              </Badge>
              <Badge variant="secondary">
                {fields.filter(f => f.required).length} required
              </Badge>
              {fields.filter(f => f.unique).length > 0 && (
                <Badge variant="secondary">
                  {fields.filter(f => f.unique).length} unique
                </Badge>
              )}
            </div>
          </div>
          <div className="flex items-center space-x-1">
            {fields.slice(0, 5).map((field, index) => (
              <div key={index} className="flex items-center space-x-1">
                {getFieldTypeIcon(field)}
                <span className="text-xs text-muted-foreground">{field.name || 'unnamed'}</span>
                {index < Math.min(fields.length - 1, 4) && (
                  <span className="text-muted-foreground">•</span>
                )}
              </div>
            ))}
            {fields.length > 5 && (
              <span className="text-xs text-muted-foreground">+{fields.length - 5} more</span>
            )}
          </div>
        </div>
      )}

      {/* Empty State */}
      {fields.length === 0 && (
        <div className="text-center py-8 border-2 border-dashed rounded-lg bg-muted/20">
          <div className="space-y-3">
            <div className="flex justify-center">
              <Database className="h-12 w-12 text-muted-foreground" />
            </div>
            <div>
              <h3 className="text-lg font-medium">No fields defined yet</h3>
              <p className="text-sm text-muted-foreground mt-1">
                Start building your schema by adding some fields
              </p>
            </div>
            <div className="flex flex-wrap justify-center gap-2 mt-4">
              <Badge variant="outline" className="text-xs">Text fields</Badge>
              <Badge variant="outline" className="text-xs">Numbers</Badge>
              <Badge variant="outline" className="text-xs">Email</Badge>
              <Badge variant="outline" className="text-xs">Select</Badge>
            </div>
            <Button 
              type="button" 
              onClick={onAddField} 
              size="sm" 
              className="mt-4"
            >
              <Plus className="h-4 w-4 mr-2" />
              Add Your First Field
            </Button>
          </div>
        </div>
      )}

      {/* Fields List */}
      <div className="space-y-4">
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
            onUpdateValidationConfig={onUpdateValidationConfig}
            onRemove={onRemoveField}
          />
        ))}
      </div>

      {/* Quick Add Suggestions */}
      {fields.length > 0 && fields.length < 5 && (
        <div className="p-4 bg-blue-50 border border-blue-200 rounded-lg">
          <h4 className="text-sm font-medium text-blue-900 mb-2">
            Common field suggestions:
          </h4>
          <div className="flex flex-wrap gap-2">              <Button 
                type="button" 
                variant="outline" 
                size="sm" 
                onClick={onAddField}
                className="text-xs h-7"
              >
                + description (text)
              </Button>
              <Button 
                type="button" 
                variant="outline" 
                size="sm" 
                onClick={onAddField}
                className="text-xs h-7"
              >
                + created_at (date)
              </Button>
              <Button 
                type="button" 
                variant="outline" 
                size="sm" 
                onClick={onAddField}
                className="text-xs h-7"
              >
                + status (select)
              </Button>
          </div>
        </div>
      )}
    </div>
  );
};

export default FieldsList; 