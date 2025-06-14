import React, { useState, useEffect } from 'react';
import { useParams, Link, useNavigate } from 'react-router-dom';
import { Plus, ChevronLeft, Database, Info } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardTitle } from '@/components/ui/card';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { RecordTable } from '@/components/RecordTable';
import { apiService } from '../services/api';
import type { DbRecord, CollectionSchema, FieldDefinition } from '../types/api';

const Records: React.FC = () => {
  const { collection } = useParams<{ collection: string }>();
  const navigate = useNavigate();
  const [records, setRecords] = useState<DbRecord[]>([]);
  const [schema, setSchema] = useState<CollectionSchema | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showSchemaModal, setShowSchemaModal] = useState(false);

  useEffect(() => {
    if (collection) {
      fetchData();
    }
  }, [collection]);

  const fetchData = async () => {
    if (!collection) return;

    try {
      setLoading(true);
      setError(null);
      
      // Fetch both records and schema in parallel
      const [recordsData, schemaData] = await Promise.all([
        apiService.getRecords(collection),
        apiService.getCollectionSchema(collection).catch(() => null) // Don't fail if schema doesn't exist
      ]);
      
      setRecords(recordsData);
      setSchema(schemaData);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch data');
    } finally {
      setLoading(false);
    }
  };

  const handleDeleteRecord = async (recordId: string) => {
    if (!collection || !confirm('Are you sure you want to delete this record?')) return;

    try {
      await apiService.deleteRecord(collection, recordId);
      await fetchData();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete record');
    }
  };

  const handleCreateRecord = () => {
    if (collection) {
      navigate(`/collections/${encodeURIComponent(collection)}/new`);
    }
  };

  const getFieldTypeBadgeVariant = (fieldType: string) => {
    switch (fieldType) {
      case 'text':
      case 'email':
      case 'url':
        return 'default';
      case 'number':
        return 'secondary';
      case 'boolean':
        return 'outline';
      case 'date':
        return 'secondary';
      case 'json':
        return 'outline';
      case 'password':
        return 'destructive';
      default:
        return 'outline';
    }
  };

  if (!collection) {
    return (
      <Card className="border-destructive">
        <CardContent className="p-6">
          <div className="text-destructive">Collection parameter is missing</div>
        </CardContent>
      </Card>
    );
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-muted-foreground">Loading records...</div>
      </div>
    );
  }

  return (
    <div>
      <div className="flex justify-between items-center mb-8">
        <div className="flex items-center space-x-4">
          <Button
            onClick={() => navigate('/collections')}
            variant="ghost"
            size="sm"
          >
            <ChevronLeft className="h-4 w-4 mr-2" />
            Collections
          </Button>
          <div>
            <h1 className="text-2xl font-bold text-foreground">{collection}</h1>
            <p className="text-muted-foreground">Manage records in this collection</p>
          </div>
        </div>
        <div className="flex space-x-2">
          {schema && (
            <Button
              onClick={() => setShowSchemaModal(true)}
              variant="outline"
            >
              <Info className="h-4 w-4 mr-2" />
              Schema Info
            </Button>
          )}
          <Button asChild variant="outline">
            <Link to={`/collections/${encodeURIComponent(collection!)}/edit`}>
              <Database className="h-4 w-4 mr-2" />
              Edit Schema
            </Link>
          </Button>
          <Button onClick={handleCreateRecord}>
            <Plus className="h-4 w-4 mr-2" />
            New Record
          </Button>
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

      {records.length === 0 ? (
        <Card>
          <CardContent className="text-center py-12">
            <Database className="mx-auto h-12 w-12 text-muted-foreground" />
            <CardTitle className="mt-4 text-lg">No records</CardTitle>
            <CardDescription className="mt-2">Get started by creating a new record.</CardDescription>
            <div className="mt-6">
              <Button onClick={handleCreateRecord}>
                Create Record
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <RecordTable
            records={records}
            schema={schema}
            collection={collection || ''}
            onDelete={handleDeleteRecord}
          />
        </Card>
      )}

      {/* Schema Modal */}
      {schema && (
        <Dialog open={showSchemaModal} onOpenChange={setShowSchemaModal}>
          <DialogContent className="sm:max-w-[600px]">
            <DialogHeader>
              <DialogTitle>Collection Schema: {schema.name}</DialogTitle>
            </DialogHeader>
            <div className="space-y-4">
              <div className="grid grid-cols-2 gap-4 text-sm">
                <div>
                  <Label className="font-medium">Type</Label>
                  <p className="text-muted-foreground capitalize">{schema.collection_type}</p>
                </div>
                <div>
                  <Label className="font-medium">Created</Label>
                  <p className="text-muted-foreground">
                    {new Date(schema.created_at * 1000).toLocaleString()}
                  </p>
                </div>
              </div>
              
              <div>
                <Label className="font-medium mb-2 block">Fields</Label>
                {Object.keys(schema.fields).length === 0 ? (
                  <p className="text-muted-foreground text-sm">No schema fields defined</p>
                ) : (
                  <div className="space-y-2">
                    {Object.entries(schema.fields).map(([fieldName, fieldDef]: [string, FieldDefinition]) => (
                      <div key={fieldName} className="flex items-center justify-between p-2 border rounded">
                        <div className="flex items-center space-x-2">
                          <span className="font-medium text-sm">{fieldName}</span>
                          {fieldDef.required && (
                            <Badge variant="destructive" className="text-xs">required</Badge>
                          )}
                        </div>
                        <Badge variant={getFieldTypeBadgeVariant(fieldDef.field_type)} className="text-xs">
                          {fieldDef.field_type}
                        </Badge>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          </DialogContent>
        </Dialog>
      )}
    </div>
  );
};

export default Records; 