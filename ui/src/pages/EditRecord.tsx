import React, { useState, useEffect, useCallback } from 'react';
import { useParams, useNavigate, Link } from 'react-router-dom';
import { ChevronLeft, Save, Code, FileText, Plus, Settings } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { CustomizableSchemaForm } from '@/components/CustomizableSchemaForm';
import PageLayout from '@/components/PageLayout';
import ActionDropdown from '@/components/ActionDropdown';
import { apiService } from '../services/api';
import type { DbRecord, CollectionSchema } from '../types/api';
import { Tooltip, TooltipTrigger, TooltipContent, TooltipProvider } from '@/components/ui/tooltip';
import { useFieldCustomization } from '../hooks/useFieldCustomization';

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

  // Field customization hook
  const fieldCustomization = useFieldCustomization(collection || '', schema);

  const fetchData = useCallback(async () => {
    if (!collection) return;

    try {
      setLoading(true);
      setError(null);
      
      if (isCreateMode) {
        // For create mode, only fetch schema
        const schemaData = await apiService.getCollectionSchema(collection).catch(() => null);

        if (schemaData?.collection_type === 'single') {
          const existingRecords = await apiService.getRecords(collection, { limit: 1 });
          const existingRecord = existingRecords[0];

          if (existingRecord) {
            navigate(
              `/collections/${encodeURIComponent(collection)}/edit/${existingRecord.id}`,
              { replace: true }
            );
            return;
          }
        }

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
  }, [collection, isCreateMode, navigate, recordId]);

  useEffect(() => {
    if (collection) {
      fetchData();
    }
  }, [collection, fetchData]);

  const handleSchemaFormSave = async (data: Record<string, unknown>) => {
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
      <PageLayout title="Error" description="Collection parameter is required">
        <Card className="border-destructive">
          <CardContent className="p-4 sm:p-6">
            <div className="text-destructive">Collection parameter is required</div>
          </CardContent>
        </Card>
      </PageLayout>
    );
  }

  if (loading) {
    return (
      <PageLayout title="Loading..." description={`Loading ${isCreateMode ? 'schema' : 'record'}...`}>
        <div className="flex items-center justify-center h-32 sm:h-64">
          <div className="text-muted-foreground text-sm sm:text-base">Loading {isCreateMode ? 'schema' : 'record'}...</div>
        </div>
      </PageLayout>
    );
  }

  if (!isCreateMode && !record) {
    return (
      <PageLayout title="Error" description="Record not found">
        <Card className="border-destructive">
          <CardContent className="p-4 sm:p-6">
            <div className="text-destructive">Record not found</div>
          </CardContent>
        </Card>
      </PageLayout>
    );
  }

  const leftActions = (
    <Button
      asChild
      variant="ghost"
      size="sm"
      className="h-9 px-2.5 sm:h-8 sm:px-3"
    >
      <Link to={`/collections/${encodeURIComponent(collection)}`}>
        <ChevronLeft className="h-4 w-4 mr-1.5 sm:mr-2" />
        <span className="hidden xs:inline">Back to Collection</span>
        <span className="xs:hidden">Back</span>
      </Link>
    </Button>
  );

  const headerActions = schema && Object.keys(schema.fields).length > 0 ? (
    <ActionDropdown
      actions={[
        {
          id: "form-view",
          label: "Form View",
          icon: <FileText className="h-4 w-4" />,
          onClick: () => setUseSchemaForm(true),
          active: useSchemaForm,
        },
        {
          id: "json-editor",
          label: "JSON Editor", 
          icon: <Code className="h-4 w-4" />,
          onClick: () => setUseSchemaForm(false),
          active: !useSchemaForm,
        },
        {
          id: "customize-fields",
          label: fieldCustomization.customizationMode ? "Exit Customization" : "Customize Fields",
          icon: <Settings className="h-4 w-4" />,
          onClick: fieldCustomization.toggleCustomizationMode,
          active: fieldCustomization.customizationMode,
        }
      ]}
      showDropdownOn="mobile"
    />
  ) : undefined;

  const isSingleCollection = schema?.collection_type === 'single';

  return (
    <PageLayout 
      title={
        isCreateMode
          ? `${isSingleCollection ? 'Create Entry' : 'Create Record'} in "${collection}"`
          : `${isSingleCollection ? 'Edit Entry' : 'Edit Record'} in "${collection}"`
      }
      description={isCreateMode ? `Add ${isSingleCollection ? 'the content entry' : 'a new record'} to this collection` : `Record ID: ${record?.id}`}
      leftActions={leftActions}
      headerActions={headerActions}
    >

      {/* Error Display */}
      {error && (
        <Card className="border-destructive">
          <CardContent className="p-3 sm:p-4">
            <div className="text-destructive text-sm sm:text-base">{error}</div>
            <Button
              onClick={() => setError(null)}
              variant="ghost"
              size="sm"
              className="text-destructive text-xs sm:text-sm mt-2 hover:text-destructive/80 p-0 h-auto"
            >
              Dismiss
            </Button>
          </CardContent>
        </Card>
      )}

      {/* Record Metadata - Only show in edit mode */}
      {!isCreateMode && record && (
        <Card>
          <CardHeader className="pb-3 sm:pb-6">
            <CardTitle className="text-base sm:text-lg">Record Information</CardTitle>
          </CardHeader>
          <CardContent className="pt-0">
            <TooltipProvider>
              <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 sm:gap-4 text-sm">
                <div className="space-y-1.5">
                  <Label className="font-medium text-xs sm:text-sm">Record ID</Label>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <p className="text-muted-foreground text-xs sm:text-sm break-all cursor-pointer leading-relaxed" tabIndex={0}>
                        {record.id}
                      </p>
                    </TooltipTrigger>
                    <TooltipContent className="max-w-xs break-all">{record.id}</TooltipContent>
                  </Tooltip>
                </div>
                <div className="space-y-1.5">
                  <Label className="font-medium text-xs sm:text-sm">Created</Label>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <p className="text-muted-foreground text-xs sm:text-sm cursor-pointer leading-relaxed" tabIndex={0}>
                        {new Date(Number(record.created_at) * 1000).toLocaleString()}
                      </p>
                    </TooltipTrigger>
                    <TooltipContent>{new Date(Number(record.created_at) * 1000).toLocaleString()}</TooltipContent>
                  </Tooltip>
                </div>
                <div className="space-y-1.5 sm:col-span-2 lg:col-span-1">
                  <Label className="font-medium text-xs sm:text-sm">Last Updated</Label>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <p className="text-muted-foreground text-xs sm:text-sm cursor-pointer leading-relaxed" tabIndex={0}>
                        {new Date(Number(record.updated_at) * 1000).toLocaleString()}
                      </p>
                    </TooltipTrigger>
                    <TooltipContent>{new Date(Number(record.updated_at) * 1000).toLocaleString()}</TooltipContent>
                  </Tooltip>
                </div>
              </div>
            </TooltipProvider>
          </CardContent>
        </Card>
      )}

      {/* Form */}
      <Card>
        <CardHeader className="pb-3 sm:pb-6">
          <CardTitle className="text-base sm:text-lg">
            {isSingleCollection ? 'Entry Data' : isCreateMode ? 'Record Data' : 'Edit Record Data'}
          </CardTitle>
        </CardHeader>
        <CardContent className="pt-0">
          {useSchemaForm && schema && Object.keys(schema.fields).length > 0 ? (
            <CustomizableSchemaForm
              schema={schema}
              initialData={isCreateMode ? {} : record?.data}
              onSubmit={handleSchemaFormSave}
              onCancel={handleCancel}
              submitLabel={isCreateMode ? (isSingleCollection ? "Create Entry" : "Create Record") : "Save Changes"}
              isSubmitting={saving}
              customizationMode={fieldCustomization.customizationMode}
              fieldCustomizations={fieldCustomization.settings.fieldCustomizations}
              onFieldCustomizationChange={fieldCustomization.updateFieldCustomization}
              onDragStart={fieldCustomization.handleDragStart}
              onDragOver={fieldCustomization.handleDragOver}
              onDrop={fieldCustomization.handleDrop}
              draggedField={fieldCustomization.draggedField}
              onToggleCustomizationMode={fieldCustomization.toggleCustomizationMode}
              onResetToDefault={fieldCustomization.resetToDefault}
              onShowAllFields={fieldCustomization.showAllFields}
              onHideAllFields={fieldCustomization.hideAllFields}
            />
          ) : (
            <form onSubmit={handleJsonSave} className="space-y-4 sm:space-y-6">
              <div className="space-y-2 sm:space-y-3">
                <Label htmlFor="recordData" className="text-sm sm:text-base font-medium">
                  {isSingleCollection ? 'Entry Data (JSON)' : 'Record Data (JSON)'}
                </Label>
                <Textarea
                  id="recordData"
                  value={jsonData}
                  onChange={(e) => setJsonData(e.target.value)}
                  className="min-h-[300px] sm:min-h-[400px] font-mono text-xs sm:text-sm leading-relaxed"
                  required
                />
              </div>
              <div className="flex flex-col sm:flex-row justify-end gap-2 sm:gap-3 pt-2">
                <Button
                  type="button"
                  variant="outline"
                  onClick={handleCancel}
                  disabled={saving}
                  className="h-10 px-4 sm:h-9 sm:px-3 order-2 sm:order-1"
                >
                  Cancel
                </Button>
                <Button
                  type="submit"
                  disabled={saving || !jsonData.trim()}
                  className="h-10 px-4 sm:h-9 sm:px-3 order-1 sm:order-2"
                >
                  {isCreateMode ? <Plus className="h-4 w-4 mr-2" /> : <Save className="h-4 w-4 mr-2" />}
                  {saving
                    ? (isCreateMode ? 'Creating...' : 'Saving...')
                    : (isCreateMode ? (isSingleCollection ? 'Create Entry' : 'Create Record') : 'Save Changes')}
                </Button>
              </div>
            </form>
          )}
        </CardContent>
      </Card>
    </PageLayout>
  );
};

export default EditRecord; 
