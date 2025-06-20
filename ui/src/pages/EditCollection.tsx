import React, { useState, useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { ArrowLeft, Plus, X, Save } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { apiService } from '../services/api';
import type { CollectionSchema, FieldDefinition, FieldType } from '../types/api';
import type { RelationshipConfig } from '../types/generated';

interface FieldFormData {
  name: string;
  field_type: FieldType;
  required: boolean;
  unique: boolean;
  default?: string;
  // Relationship configuration
  relationshipConfig?: {
    target_collection: string;
    multiple: boolean;
    cascade_delete: boolean;
    display_field?: string;
  };
}

const EditCollection: React.FC = () => {
  const { collection } = useParams<{ collection: string }>();
  const navigate = useNavigate();
  const [schema, setSchema] = useState<CollectionSchema | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [collections, setCollections] = useState<CollectionSchema[]>([]);

  // Schema form state
  const [fields, setFields] = useState<FieldFormData[]>([]);

  useEffect(() => {
    if (collection) {
      fetchSchema();
    }
    loadCollections();
  }, [collection]);

  const loadCollections = async () => {
    try {
      const collections = await apiService.getCollections();
      setCollections(collections);
    } catch (err) {
      console.error('Failed to load collections:', err);
    }
  };

  const fetchSchema = async () => {
    if (!collection) return;

    try {
      setLoading(true);
      const schemaData = await apiService.getCollectionSchema(collection);
      setSchema(schemaData);
      
      // Convert schema fields to form data
      const formFields: FieldFormData[] = Object.entries(schemaData.fields).map(([name, fieldDef]) => {
        const field: FieldFormData = {
          name,
          field_type: fieldDef.field_type,
          required: fieldDef.required,
          unique: fieldDef.unique,
          default: fieldDef.default ? JSON.stringify(fieldDef.default) : undefined,
        };

        // If it's a relationship field, extract the configuration
        if (typeof fieldDef.field_type === 'object' && 'relationship' in fieldDef.field_type) {
          field.relationshipConfig = {
            target_collection: fieldDef.field_type.relationship.target_collection,
            multiple: fieldDef.field_type.relationship.multiple,
            cascade_delete: fieldDef.field_type.relationship.cascade_delete,
            display_field: fieldDef.field_type.relationship.display_field,
          };
        }

        return field;
      });
      
      setFields(formFields);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch collection schema');
    } finally {
      setLoading(false);
    }
  };

  const addField = () => {
    setFields([...fields, {
      name: '',
      field_type: 'text',
      required: false,
      unique: false,
    }]);
  };

  const updateField = (index: number, field: Partial<FieldFormData>) => {
    setFields(fields.map((f, i) => i === index ? { ...f, ...field } : f));
  };

  const removeField = (index: number) => {
    setFields(fields.filter((_, i) => i !== index));
  };

  const getFieldTypeString = (fieldType: FieldType): string => {
    if (typeof fieldType === 'string') {
      return fieldType;
    } else if (typeof fieldType === 'object' && 'relationship' in fieldType) {
      return 'relationship';
    }
    return 'text';
  };

  const updateFieldType = (index: number, newType: string) => {
    const field = fields[index];
    if (newType === 'relationship') {
      updateField(index, {
        field_type: {
          relationship: {
            target_collection: '',
            multiple: false,
            cascade_delete: false,
            display_field: undefined,
          }
        },
        relationshipConfig: {
          target_collection: '',
          multiple: false,
          cascade_delete: false,
          display_field: undefined,
        }
      });
    } else {
      updateField(index, {
        field_type: newType as FieldType,
        relationshipConfig: undefined
      });
    }
  };

  const updateRelationshipConfig = (index: number, config: Partial<RelationshipConfig>) => {
    const field = fields[index];
    const newConfig = { ...field.relationshipConfig, ...config };
    updateField(index, {
      relationshipConfig: newConfig,
      field_type: {
        relationship: {
          target_collection: newConfig.target_collection || '',
          multiple: newConfig.multiple || false,
          cascade_delete: newConfig.cascade_delete || false,
          display_field: newConfig.display_field,
        }
      }
    });
  };

  const handleUpdateSchema = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!schema || !collection) return;

    try {
      setSaving(true);
      
      // Convert fields to the required format
      const fieldsMap: Record<string, FieldDefinition> = {};
      fields.forEach(field => {
        if (field.name.trim()) {
          fieldsMap[field.name.trim()] = {
            field_type: field.field_type,
            required: field.required,
            unique: field.unique,
            default: field.default ? JSON.parse(field.default) : null,
            validation: null
          };
        }
      });

      const updatedSchema: CollectionSchema = {
        ...schema,
        fields: fieldsMap,
        updated_at: Date.now(),
      };

      await apiService.updateCollectionSchema(collection, updatedSchema);
      navigate(`/collections/${encodeURIComponent(collection)}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update collection schema');
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-muted-foreground">Loading collection schema...</div>
      </div>
    );
  }

  if (!schema) {
    return (
      <div className="max-w-4xl mx-auto">
        <div className="flex items-center mb-8">
          <Button
            onClick={() => navigate('/collections')}
            variant="ghost"
            size="sm"
            className="mr-4"
          >
            <ArrowLeft className="h-4 w-4 mr-2" />
            Back to Collections
          </Button>
          <div>
            <h1 className="text-2xl font-bold text-foreground">Collection Not Found</h1>
            <p className="text-muted-foreground mt-1">The requested collection could not be found</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-4xl mx-auto">
      <div className="flex items-center mb-8">
        <Button
          onClick={() => navigate(`/collections/${encodeURIComponent(collection!)}`)}
          variant="ghost"
          size="sm"
          className="mr-4"
        >
          <ArrowLeft className="h-4 w-4 mr-2" />
          Back to Records
        </Button>
        <div>
          <h1 className="text-2xl font-bold text-foreground">Edit Collection Schema</h1>
          <p className="text-muted-foreground mt-1">Update schema for "{collection}"</p>
        </div>
      </div>

      {error && (
        <Card className="mb-6 border-destructive">
          <CardContent className="p-4">
            <div className="text-destructive">{error}</div>
            <Button
              onClick={() => setError(null)}
              variant="ghost"
              size="sm"
              className="text-destructive text-sm mt-2 hover:text-destructive/80 p-0 h-auto"
            >
              Dismiss
            </Button>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center space-x-2">
            <span>Collection Schema</span>
            <span className="text-sm text-muted-foreground font-normal">({schema.collection_type})</span>
          </CardTitle>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleUpdateSchema} className="space-y-6">
            {/* Basic collection info (read-only) */}
            <div className="space-y-4">
              <div className="space-y-2">
                <Label>Collection Name</Label>
                <Input
                  value={schema.name}
                  disabled
                  className="bg-muted"
                />
                <p className="text-sm text-muted-foreground">
                  Collection name cannot be changed after creation
                </p>
              </div>
            </div>

            {/* Schema fields */}
            <div className="space-y-4">
              <div className="flex justify-between items-center">
                <Label>Schema Fields</Label>
                <Button type="button" onClick={addField} size="sm" variant="outline">
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
                <Card key={index} className="p-4">
                  <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                    <div className="space-y-2">
                      <Label htmlFor={`field-name-${index}`}>Field Name</Label>
                      <Input
                        id={`field-name-${index}`}
                        value={field.name}
                        onChange={(e) => updateField(index, { name: e.target.value })}
                        placeholder="field_name"
                        required
                      />
                    </div>

                    <div className="space-y-2">
                      <Label htmlFor={`field-type-${index}`}>Type</Label>
                      <select 
                        id={`field-type-${index}`}
                        value={getFieldTypeString(field.field_type)} 
                        onChange={(e) => updateFieldType(index, e.target.value)}
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
                      </select>
                    </div>

                    <div className="space-y-2">
                      <Label htmlFor={`field-default-${index}`}>Default Value</Label>
                      <Input
                        id={`field-default-${index}`}
                        value={field.default || ''}
                        onChange={(e) => updateField(index, { default: e.target.value })}
                        placeholder="JSON value"
                      />
                    </div>

                    {/* Relationship Configuration */}
                    {getFieldTypeString(field.field_type) === 'relationship' && (
                      <div className="col-span-full space-y-4 p-4 border rounded-md bg-muted/50">
                        <h4 className="font-medium text-sm">Relationship Configuration</h4>
                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                          <div className="space-y-2">
                            <Label htmlFor={`field-target-${index}`}>Target Collection</Label>
                            <select
                              id={`field-target-${index}`}
                              value={field.relationshipConfig?.target_collection || ''}
                              onChange={(e) => updateRelationshipConfig(index, { target_collection: e.target.value })}
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
                              value={field.relationshipConfig?.display_field || ''}
                              onChange={(e) => updateRelationshipConfig(index, { display_field: e.target.value || undefined })}
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
                                  checked={field.relationshipConfig?.multiple || false}
                                  onChange={(e) => updateRelationshipConfig(index, { multiple: e.target.checked })}
                                  className="h-4 w-4 rounded border border-input bg-background text-primary focus:ring-2 focus:ring-ring focus:ring-offset-2"
                                />
                                <Label htmlFor={`field-multiple-${index}`}>Multiple (many-to-many)</Label>
                              </div>
                              <div className="flex items-center space-x-2">
                                <input
                                  type="checkbox"
                                  id={`field-cascade-${index}`}
                                  checked={field.relationshipConfig?.cascade_delete || false}
                                  onChange={(e) => updateRelationshipConfig(index, { cascade_delete: e.target.checked })}
                                  className="h-4 w-4 rounded border border-input bg-background text-primary focus:ring-2 focus:ring-ring focus:ring-offset-2"
                                />
                                <Label htmlFor={`field-cascade-${index}`}>Cascade Delete</Label>
                              </div>
                            </div>
                          </div>
                        </div>
                      </div>
                    )}

                    <div className="space-y-2">
                      <div className="flex items-center space-x-4">
                        <div className="flex items-center space-x-2">
                          <input
                            type="checkbox"
                            id={`field-required-${index}`}
                            checked={field.required}
                            onChange={(e) => updateField(index, { required: e.target.checked })}
                            className="h-4 w-4 rounded border border-input bg-background text-primary focus:ring-2 focus:ring-ring focus:ring-offset-2"
                          />
                          <Label htmlFor={`field-required-${index}`}>Required</Label>
                        </div>
                        <div className="flex items-center space-x-2">
                          <input
                            type="checkbox"
                            id={`field-unique-${index}`}
                            checked={field.unique}
                            onChange={(e) => updateField(index, { unique: e.target.checked })}
                            className="h-4 w-4 rounded border border-input bg-background text-primary focus:ring-2 focus:ring-ring focus:ring-offset-2"
                          />
                          <Label htmlFor={`field-unique-${index}`}>Unique</Label>
                        </div>
                      </div>
                      <div className="flex justify-end">
                        <Button
                          type="button"
                          onClick={() => removeField(index)}
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
              ))}
            </div>

            <div className="flex justify-end space-x-2">
              <Button
                type="button"
                variant="outline"
                onClick={() => navigate(`/collections/${encodeURIComponent(collection!)}`)}
                disabled={saving}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={saving}
              >
                {saving ? 'Saving...' : (
                  <>
                    <Save className="h-4 w-4 mr-2" />
                    Save Schema
                  </>
                )}
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
};

export default EditCollection; 