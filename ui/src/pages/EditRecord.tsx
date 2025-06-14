import React, { useState, useEffect } from 'react';
import { useParams, useNavigate, Link } from 'react-router-dom';
import { ChevronLeft, Save, Code, FileText, Plus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { SchemaForm } from '@/components/SchemaForm';
import { apiService } from '../services/api';
import type { DbRecord, CollectionSchema } from '../types/api';

const EditRecord: React.FC = () => {
  const { collection, recordId } = useParams<{ collection: string; recordId: string }>();
  const navigate = useNavigate();
  
  // Determine if we're in create mode (no recordId) or edit mode
  const isCreateMode = !recordId;
  
  const [record, setRecord] = useState<DbRecord | null>(null);
  const [schema, setSchema] = useState<CollectionSchema | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [useSchemaForm, setUseSchemaForm] = useState(true);
  const [jsonData, setJsonData] = useState('{}');

  useEffect(() => {
    if (collection) {
      fetchData();
    }
  }, [collection, recordId]);

  const fetchData = async () => {
    if (!collection) return;

    try {
      setLoading(true);
      setError(null);
      
      if (isCreateMode) {
        // For create mode, only fetch schema
        const schemaData = await apiService.getCollectionSchema(collection).catch(() => null);
        setSchema(schemaData);
        setJsonData('{}');
        setUseSchemaForm(!!schemaData && Object.keys(schemaData.fields).length > 0);
      } else {
        // For edit mode, fetch both record and schema
        const [recordData, schemaData] = await Promise.all([
          apiService.getRecord(collection, recordId!),
          apiService.getCollectionSchema(collection).catch(() => null)
        ]);
        
        setRecord(recordData);
        setSchema(schemaData);
        setJsonData(JSON.stringify(recordData.data, null, 2));
        setUseSchemaForm(!!schemaData && Object.keys(schemaData.fields).length > 0);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : `Failed to fetch ${isCreateMode ? 'schema' : 'record'}`);
    } finally {
      setLoading(false);
    }
  };

  const handleSchemaFormSave = async (data: Record<string, any>) => {
    if (!collection) return;

    try {
      setSaving(true);
      if (isCreateMode) {
        await apiService.createRecord(collection, data);
      } else {
        await apiService.updateRecord(collection, recordId!, data);
      }
      navigate(`/collections/${encodeURIComponent(collection)}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : `Failed to ${isCreateMode ? 'create' : 'update'} record`);
    } finally {
      setSaving(false);
    }
  };

  const handleJsonSave = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!collection || !jsonData.trim()) return;

    try {
      setSaving(true);
      const data = JSON.parse(jsonData);
      if (isCreateMode) {
        await apiService.createRecord(collection, data);
      } else {
        await apiService.updateRecord(collection, recordId!, data);
      }
      navigate(`/collections/${encodeURIComponent(collection)}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : `Failed to ${isCreateMode ? 'create' : 'update'} record`);
    } finally {
      setSaving(false);
    }
  };

  const handleCancel = () => {
    navigate(`/collections/${encodeURIComponent(collection || '')}`);
  };

  if (!collection) {
    return (
      <Card className="border-destructive">
        <CardContent className="p-6">
          <div className="text-destructive">Collection parameter is required</div>
        </CardContent>
      </Card>
    );
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-muted-foreground">Loading {isCreateMode ? 'schema' : 'record'}...</div>
      </div>
    );
  }

  if (!isCreateMode && !record) {
    return (
      <Card className="border-destructive">
        <CardContent className="p-6">
          <div className="text-destructive">Record not found</div>
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center space-x-4">
        <Button
          asChild
          variant="ghost"
          size="icon"
        >
          <Link to={`/collections/${encodeURIComponent(collection)}`}>
            <ChevronLeft className="h-4 w-4" />
          </Link>
        </Button>
        <div className="flex-1">
          <h1 className="text-2xl font-bold text-foreground">
            {isCreateMode ? `Create Record in "${collection}"` : `Edit Record in "${collection}"`}
          </h1>
          <p className="text-muted-foreground mt-1">
            {isCreateMode ? 'Add a new record to this collection' : `Record ID: ${record?.id}`}
          </p>
        </div>
        {schema && Object.keys(schema.fields).length > 0 && (
          <div className="flex space-x-2">
            <Button
              variant={useSchemaForm ? "default" : "outline"}
              size="sm"
              onClick={() => setUseSchemaForm(true)}
            >
              <FileText className="h-4 w-4 mr-2" />
              Form View
            </Button>
            <Button
              variant={!useSchemaForm ? "default" : "outline"}
              size="sm"
              onClick={() => setUseSchemaForm(false)}
            >
              <Code className="h-4 w-4 mr-2" />
              JSON Editor
            </Button>
          </div>
        )}
      </div>

      {/* Error Display */}
      {error && (
        <Card className="border-destructive">
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

      {/* Record Metadata - Only show in edit mode */}
      {!isCreateMode && record && (
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">Record Information</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4 text-sm">
              <div>
                <Label className="font-medium">Record ID</Label>
                <p className="text-muted-foreground">{record.id}</p>
              </div>
              <div>
                <Label className="font-medium">Created</Label>
                <p className="text-muted-foreground">
                  {new Date(record.created_at).toLocaleString()}
                </p>
              </div>
              <div>
                <Label className="font-medium">Last Updated</Label>
                <p className="text-muted-foreground">
                  {new Date(record.updated_at).toLocaleString()}
                </p>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Form */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">
            {isCreateMode ? 'Record Data' : 'Edit Record Data'}
          </CardTitle>
        </CardHeader>
        <CardContent>
          {useSchemaForm && schema && Object.keys(schema.fields).length > 0 ? (
            <SchemaForm
              schema={schema}
              initialData={isCreateMode ? {} : record?.data}
              onSubmit={handleSchemaFormSave}
              onCancel={handleCancel}
              submitLabel={isCreateMode ? "Create Record" : "Save Changes"}
              isSubmitting={saving}
            />
          ) : (
            <form onSubmit={handleJsonSave} className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="recordData">Record Data (JSON)</Label>
                <Textarea
                  id="recordData"
                  value={jsonData}
                  onChange={(e) => setJsonData(e.target.value)}
                  className="min-h-[400px] font-mono text-sm"
                  required
                />
              </div>
              <div className="flex justify-end space-x-2">
                <Button
                  type="button"
                  variant="outline"
                  onClick={handleCancel}
                  disabled={saving}
                >
                  Cancel
                </Button>
                <Button
                  type="submit"
                  disabled={saving || !jsonData.trim()}
                >
                  {isCreateMode ? <Plus className="h-4 w-4 mr-2" /> : <Save className="h-4 w-4 mr-2" />}
                  {saving ? (isCreateMode ? 'Creating...' : 'Saving...') : (isCreateMode ? 'Create Record' : 'Save Changes')}
                </Button>
              </div>
            </form>
          )}
        </CardContent>
      </Card>
    </div>
  );
};

export default EditRecord; 