import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import PageLayout from '@/components/PageLayout';
import { apiService } from '../services/api';
import type { CollectionSchema, FieldDefinition, CollectionType, CreateCollectionRequest } from '../types/api';
import { useFieldManagement, FieldsList } from '@/components/collection-form';

const CreateCollection: React.FC = () => {
  const navigate = useNavigate();
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [collections, setCollections] = useState<CollectionSchema[]>([]);

  // Schema form state
  const [collectionName, setCollectionName] = useState('');
  const [collectionType, setCollectionType] = useState<CollectionType>('base');
  
  // Use the field management hook
  const fieldManagement = useFieldManagement();

  // Load collections for relationship field options
  React.useEffect(() => {
    const loadCollections = async () => {
      try {
        const collections = await apiService.getCollections();
        setCollections(collections);
      } catch (err) {
        console.error('Failed to load collections:', err);
      }
    };
    loadCollections();
  }, []);

  const handleCreateCollection = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!collectionName.trim()) return;

    // Validate collection name
    if (collectionName.trim().startsWith('_')) {
      setError('Collection names cannot start with underscore (reserved for system collections)');
      return;
    }

    try {
      setCreating(true);
      
      // Convert fields to the required format
      const fieldsMap: Record<string, FieldDefinition> = {};
      fieldManagement.fields.forEach(field => {
        if (field.name.trim()) {
          fieldsMap[field.name.trim()] = {
            field_type: field.field_type,
            required: field.required,
            unique: field.unique,
            index: false,
            default: field.default ? JSON.parse(field.default) : null,
            validation: field.validation ? { 
              regex: field.validation.regex ?? null,
              min: field.validation.min ?? null,
              max: field.validation.max ?? null,
              message: field.validation.message ?? null,
              allow_empty: field.validation.allow_empty !== undefined ? field.validation.allow_empty : null
            } : null
          };
        }
      });

      const schema: CreateCollectionRequest = {
        name: collectionName.trim(),
        collection_type: collectionType,
        fields: fieldsMap,
        indexes: [], // Start with no custom indexes
      };

      await apiService.createCollection(schema);
      navigate('/collections');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create collection');
    } finally {
      setCreating(false);
    }
  };

  const leftActions = (
    <Button
      onClick={() => navigate('/collections')}
      variant="ghost"
      size="sm"
    >
      <ArrowLeft className="h-4 w-4 mr-2" />
      Back to Collections
    </Button>
  );

  return (
    <PageLayout 
      title="Create New Collection" 
      description="Define a new collection with schema"
      leftActions={leftActions}
    >
      <div className="max-w-4xl mx-auto">

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
            <FieldsList
              fields={fieldManagement.fields}
              collections={collections}
              onAddField={fieldManagement.addField}
              onUpdateField={fieldManagement.updateField}
              onUpdateFieldType={fieldManagement.updateFieldType}
              onUpdateRelationshipConfig={fieldManagement.updateRelationshipConfig}
              onUpdateFileConfig={fieldManagement.updateFileConfig}
              onUpdateSelectConfig={fieldManagement.updateSelectConfig}
              onUpdateValidationConfig={fieldManagement.updateValidationConfig}
              onRemoveField={fieldManagement.removeField}
            />

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
    </PageLayout>
  );
};

export default CreateCollection; 