import React, { useCallback, useMemo } from 'react';
import { useParams, Link, useNavigate } from 'react-router-dom';
import { ArrowLeft, Plus, Download, Upload, Settings, Database } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { RecordTable } from '@/components/RecordTable';
import PageLayout from '@/components/PageLayout';
import { apiService } from '../services/api';
import { useFieldCustomization } from '../hooks/useFieldCustomization';
import { useRecordsData } from '../hooks/useRecordsData';
import { usePermissions } from '../hooks/usePermissions';
import { PermissionRuleEditor } from '../components/PermissionRuleEditor';
import { CollectionOverview } from '../components/CollectionOverview';
import { CollectionInfo } from '../components/CollectionInfo';
import { AccessRules } from '../components/AccessRules';
import { SchemaDisplay } from '../components/SchemaDisplay';
import { FieldCustomizationPanel } from '../components/FieldCustomizationPanel';

/**
 * Records page component for managing collection records and settings
 * Handles data fetching, permissions, schema display, and record management
 */
const Records: React.FC = () => {
  const { collection } = useParams<{ collection: string }>();
  const navigate = useNavigate();
  
  // Custom hooks for data management
  const { 
    records, 
    schema, 
    stats,
    loading, 
    error, 
    refetch,
    clearError
  } = useRecordsData(collection);
  
  const {
    permissions,
    editingPermissions,
    setEditingPermissions,
    updatePermissions,
    resetPermissions,
    applyPreset
  } = usePermissions(collection);

  // Field customization hook
  const fieldCustomization = useFieldCustomization(collection || '', schema);

  // Helper function to format size in KB to human readable format
  const formatSize = (sizeKb: number): string => {
    if (sizeKb < 1024) {
      return `${sizeKb.toFixed(1)} KB`;
    } else if (sizeKb < 1024 * 1024) {
      return `${(sizeKb / 1024).toFixed(1)} MB`;
    } else {
      return `${(sizeKb / (1024 * 1024)).toFixed(1)} GB`;
    }
  };

  // Memoized collection info
  const collectionInfo = useMemo(() => ({
    name: collection || '',
    description: "User account information and profiles",
    recordCount: stats?.record_count || records.length,
    size: stats ? formatSize(stats.size_kb) : "Unknown",
    created: stats?.created_at || (schema ? new Date(Number(schema.created_at) * 1000).toLocaleDateString() : "Unknown"),
    lastModified: stats?.updated_at || "Unknown",
    status: "active",
  }), [collection, records.length, schema, stats]);

  // Helper function to get display string for field type
  const getFieldTypeDisplay = (fieldType: unknown): string => {
    if (typeof fieldType === 'string') {
      return fieldType;
    } else if (typeof fieldType === 'object' && fieldType !== null) {
      if ('relationship' in fieldType) {
        return 'relationship';
      }
      if ('file' in fieldType) {
        return 'file';
      }
      return 'unknown';
    }
    return 'unknown';
  };

  // Memoized schema fields for easier rendering
  const schemaFields = useMemo(() => {
    if (!schema) return [];
    
    return Object.entries(schema.fields).map(([name, field]) => ({
      name,
      type: getFieldTypeDisplay(field.field_type),
      required: field.required || false,
      unique: false, // Placeholder - not in current schema
      indexed: false, // Placeholder - not in current schema
    }));
  }, [schema]);

  // Event handlers
  const handleDeleteRecord = useCallback(async (recordId: string) => {
    if (!collection || !confirm('Are you sure you want to delete this record?')) return;

    try {
      await apiService.deleteRecord(collection, recordId);
      await refetch();
    } catch (err) {
      console.error('Failed to delete record:', err);
      // Error will be handled by the useRecordsData hook
    }
  }, [collection, refetch]);

  const handleCreateRecord = useCallback(() => {
    if (collection) {
      navigate(`/collections/${encodeURIComponent(collection)}/new`);
    }
  }, [collection, navigate]);

  const handleDismissError = useCallback(() => {
    clearError();
  }, [clearError]);

  // Early returns for edge cases
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
      <PageLayout title="Loading..." description="Loading collection data...">
        <div className="flex items-center justify-center h-64">
          <div className="text-muted-foreground">Loading records...</div>
        </div>
      </PageLayout>
    );
  }

  const leftActions = (
    <Link to="/collections">
      <Button variant="ghost" size="sm">
        <ArrowLeft className="h-4 w-4 mr-2" />
        <span className="hidden sm:inline">Back to Collections</span>
        <span className="sm:hidden">Back</span>
      </Button>
    </Link>
  );

  return (
    <PageLayout 
      title={collection || 'Collection'} 
      description="Manage records and collection settings"
      leftActions={leftActions}
    >
      {/* Error Display */}
      {error && (
        <Card className="border-destructive">
          <CardContent className="p-4">
            <div className="text-destructive">{error}</div>
            <Button
              onClick={handleDismissError}
              variant="ghost"
              size="sm"
              className="text-destructive text-sm mt-2 hover:text-destructive/80 p-0 h-auto"
            >
              Dismiss
            </Button>
          </CardContent>
        </Card>
      )}

      {/* Collection Overview */}
      <CollectionOverview collectionInfo={collectionInfo} schemaFields={schemaFields} />

      {/* Collection Info */}
      <CollectionInfo 
        collectionInfo={collectionInfo}
        collection={collection}
        schema={schema}
      />

      {/* Access Rules */}
      {editingPermissions ? (
        <Card>
          <CardHeader>
            <CardTitle>Edit Permissions: {collection}</CardTitle>
            <CardDescription>
              Configure access control rules for each operation
            </CardDescription>
          </CardHeader>
          <CardContent>
            <PermissionRuleEditor
              permissions={editingPermissions}
              onChange={setEditingPermissions}
              schema={schema}
            />
            <div className="flex space-x-2 mt-6">
              <Button 
                onClick={() => updatePermissions(editingPermissions)}
              >
                Save Changes
              </Button>
              <Button 
                variant="outline" 
                onClick={() => setEditingPermissions(null)}
              >
                Cancel
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : (
        <AccessRules
          permissions={permissions}
          collection={collection}
          schema={schema}
          onEdit={() => permissions && setEditingPermissions(permissions)}
          onReset={resetPermissions}
          onApplyPreset={applyPreset}
        />
      )}

      {/* Schema Display */}
      <SchemaDisplay 
        schemaFields={schemaFields}
        collection={collection}
      />

      {/* Recent Records */}
      <Card>
        <CardHeader>
          <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
            <div>
              <CardTitle>Recent Records</CardTitle>
              <CardDescription>Latest records in this collection</CardDescription>
            </div>
            <div className="flex flex-col sm:flex-row gap-2">
              <Button 
                variant="outline" 
                size="sm" 
                onClick={fieldCustomization.toggleCustomizationMode}
                className="w-full sm:w-auto"
              >
                <Settings className="h-4 w-4 mr-2" />
                {fieldCustomization.customizationMode ? 'Exit Customization' : 'Customize Fields'}
              </Button>
              <Button variant="outline" size="sm" className="w-full sm:w-auto" disabled>
                <Download className="h-4 w-4 mr-2" />
                Export
              </Button>
              <Button variant="outline" size="sm" className="w-full sm:w-auto" disabled>
                <Upload className="h-4 w-4 mr-2" />
                Import
              </Button>
              <Button onClick={handleCreateRecord} size="sm" className="w-full sm:w-auto">
                <Plus className="h-4 w-4 mr-2" />
                Add Record
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          {/* Field Customization Controls */}
          {fieldCustomization.customizationMode && schema && (
            <FieldCustomizationPanel 
              fieldCustomization={fieldCustomization}
              schema={schema}
            />
          )}

          {records.length === 0 ? (
            <div className="text-center py-12">
              <Database className="mx-auto h-12 w-12 text-muted-foreground" />
              <CardTitle className="mt-4 text-lg">No records</CardTitle>
              <CardDescription className="mt-2">Get started by creating a new record.</CardDescription>
              <div className="mt-6">
                <Button onClick={handleCreateRecord}>
                  Create Record
                </Button>
              </div>
            </div>
          ) : (
            <RecordTable
              records={records}
              schema={schema}
              collection={collection || ''}
              onDelete={handleDeleteRecord}
              orderedFields={fieldCustomization.getOrderedVisibleFields()}
            />
          )}
        </CardContent>
      </Card>
    </PageLayout>
  );
};

export default Records; 