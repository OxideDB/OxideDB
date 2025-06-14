import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { ArrowLeft, Plus, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { apiService } from '../services/api';
import type { CreateCollectionRequest, FieldDefinition, FieldType, CollectionType } from '../types/api';

interface FieldFormData {
  name: string;
  field_type: FieldType;
  required: boolean;
  unique: boolean;
  default?: string;
}

const CreateCollection: React.FC = () => {
  const navigate = useNavigate();
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Schema form state
  const [collectionName, setCollectionName] = useState('');
  const [collectionType, setCollectionType] = useState<CollectionType>('base');
  const [fields, setFields] = useState<FieldFormData[]>([]);

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

  const handleCreateCollection = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!collectionName.trim()) return;

    try {
      setCreating(true);
      const now = Date.now();
      
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

      const schema: CreateCollectionRequest = {
        id: crypto.randomUUID(),
        name: collectionName.trim(),
        collection_type: collectionType,
        fields: fieldsMap,
        indexes: [], // Start with no custom indexes
        created_at: now,
        updated_at: now,
      };

      await apiService.createCollection(schema);
      navigate('/collections');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create collection');
    } finally {
      setCreating(false);
    }
  };

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
          <h1 className="text-2xl font-bold text-foreground">Create New Collection</h1>
          <p className="text-muted-foreground mt-1">Define a new collection with schema</p>
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
          <CardTitle>Collection Schema</CardTitle>
        </CardHeader>
        <CardContent>
          <form onSubmit={handleCreateCollection} className="space-y-6">
            {/* Collection basic info */}
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="collectionName">Collection Name</Label>
                <Input
                  id="collectionName"
                  value={collectionName}
                  onChange={(e) => setCollectionName(e.target.value)}
                  placeholder="Enter collection name"
                  autoFocus
                  required
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="collectionType">Collection Type</Label>
                <select 
                  id="collectionType"
                  value={collectionType} 
                  onChange={(e) => setCollectionType(e.target.value as CollectionType)}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  <option value="base">Base Collection</option>
                  <option value="auth">Auth Collection</option>
                </select>
                <p className="text-sm text-muted-foreground">
                  Base collections are for application data, Auth collections are for user management
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
                        value={field.field_type} 
                        onChange={(e) => updateField(index, { field_type: e.target.value as FieldType })}
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
                onClick={() => navigate('/collections')}
                disabled={creating}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={creating || !collectionName.trim()}
              >
                {creating ? 'Creating...' : 'Create Collection'}
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
};

export default CreateCollection; 