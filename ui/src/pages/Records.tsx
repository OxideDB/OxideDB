import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useParams, Link, useNavigate } from 'react-router-dom';
import { ArrowLeft, Plus, Download, Upload, Settings, Database, Search, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { RecordTable } from '@/components/RecordTable';
import PageLayout from '@/components/PageLayout';
import { apiService } from '../services/api';
import type { RecordFilterOp, RecordQueryParams } from '../services/api';
import { useFieldCustomization } from '../hooks/useFieldCustomization';
import { useRecordsData } from '../hooks/useRecordsData';
import { usePermissions } from '../hooks/usePermissions';
import { PermissionRuleEditor } from '../components/PermissionRuleEditor';
import { CollectionOverview } from '../components/CollectionOverview';
import { CollectionInfo } from '../components/CollectionInfo';
import { AccessRules } from '../components/AccessRules';
import { SchemaDisplay } from '../components/SchemaDisplay';
import { FieldCustomizationPanel } from '../components/FieldCustomizationPanel';

const ALL_FILTER_FIELDS = '__all__';
const VALUELESS_FILTER_OPS = new Set<RecordFilterOp>(['exists', 'not_exists']);
const FILTER_OPERATIONS: { value: RecordFilterOp; label: string }[] = [
  { value: 'eq', label: '=' },
  { value: 'ne', label: '!=' },
  { value: 'contains', label: 'Contains' },
  { value: 'gt', label: '>' },
  { value: 'gte', label: '>=' },
  { value: 'lt', label: '<' },
  { value: 'lte', label: '<=' },
  { value: 'exists', label: 'Exists' },
  { value: 'not_exists', label: 'Missing' },
];

/**
 * Records page component for managing collection records and settings
 * Handles data fetching, permissions, schema display, and record management
 */
const Records: React.FC = () => {
  const { collection } = useParams<{ collection: string }>();
  const navigate = useNavigate();
  const [searchText, setSearchText] = useState('');
  const [debouncedSearchText, setDebouncedSearchText] = useState('');
  const [filterField, setFilterField] = useState(ALL_FILTER_FIELDS);
  const [filterOp, setFilterOp] = useState<RecordFilterOp>('eq');
  const [filterValue, setFilterValue] = useState('');
  const [filtersCollection, setFiltersCollection] = useState(collection);
  const filterRequiresValue = !VALUELESS_FILTER_OPS.has(filterOp);
  const filtersMatchCollection = filtersCollection === collection;

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setDebouncedSearchText(searchText);
    }, 250);

    return () => window.clearTimeout(timer);
  }, [searchText]);

  useEffect(() => {
    setSearchText('');
    setDebouncedSearchText('');
    setFilterField(ALL_FILTER_FIELDS);
    setFilterOp('eq');
    setFilterValue('');
    setFiltersCollection(collection);
  }, [collection]);

  const recordQuery = useMemo<RecordQueryParams>(() => {
    const params: RecordQueryParams = {};
    const search = filtersMatchCollection ? debouncedSearchText.trim() : '';
    const value = filterValue.trim();

    if (search) {
      params.search = search;
    }

    if (filtersMatchCollection && filterField !== ALL_FILTER_FIELDS && (!filterRequiresValue || value)) {
      params.filter_field = filterField;
      params.filter_op = filterOp;

      if (filterRequiresValue) {
        params.filter_value = value;
      }
    }

    return params;
  }, [debouncedSearchText, filterField, filterOp, filterRequiresValue, filterValue, filtersMatchCollection]);
  
  // Custom hooks for data management
  const { 
    records, 
    schema, 
    stats,
    loading, 
    error, 
    refetch,
    clearError
  } = useRecordsData(collection, recordQuery);
  
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

  const filterFields = useMemo(() => {
    const seen = new Set<string>();
    return [
      { value: 'id', label: 'id' },
      { value: 'created_at', label: 'created_at' },
      { value: 'updated_at', label: 'updated_at' },
      ...schemaFields.map((field) => ({ value: field.name, label: field.name })),
    ].filter((field) => {
      if (seen.has(field.value)) return false;
      seen.add(field.value);
      return true;
    });
  }, [schemaFields]);

  const hasActiveRecordFilters = useMemo(() => (
    filtersMatchCollection &&
    (searchText.trim().length > 0 ||
      (filterField !== ALL_FILTER_FIELDS && (!filterRequiresValue || filterValue.trim().length > 0)))
  ), [filterField, filterRequiresValue, filterValue, filtersMatchCollection, searchText]);

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

  const handleFilterFieldChange = useCallback((value: string) => {
    setFiltersCollection(collection);
    setFilterField(value);
    if (value === ALL_FILTER_FIELDS) {
      setFilterValue('');
    }
  }, [collection]);

  const handleFilterOpChange = useCallback((value: string) => {
    const nextOp = value as RecordFilterOp;
    setFiltersCollection(collection);
    setFilterOp(nextOp);
    if (VALUELESS_FILTER_OPS.has(nextOp)) {
      setFilterValue('');
    }
  }, [collection]);

  const handleClearRecordFilters = useCallback(() => {
    setSearchText('');
    setDebouncedSearchText('');
    setFilterField(ALL_FILTER_FIELDS);
    setFilterOp('eq');
    setFilterValue('');
    setFiltersCollection(collection);
  }, [collection]);

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
          <div className="grid gap-2 pt-2 md:grid-cols-[minmax(180px,1fr)_minmax(140px,180px)_minmax(112px,140px)_minmax(160px,1fr)_40px]">
            <div className="relative">
              <Search className="pointer-events-none absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
              <Input
                value={searchText}
                onChange={(event) => {
                  setFiltersCollection(collection);
                  setSearchText(event.target.value);
                }}
                placeholder="Search records"
                className="pl-8"
              />
            </div>
            <Select value={filterField} onValueChange={handleFilterFieldChange}>
              <SelectTrigger>
                <SelectValue placeholder="Field" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={ALL_FILTER_FIELDS}>All fields</SelectItem>
                {filterFields.map((field) => (
                  <SelectItem key={field.value} value={field.value}>
                    {field.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Select
              value={filterOp}
              onValueChange={handleFilterOpChange}
              disabled={filterField === ALL_FILTER_FIELDS}
            >
              <SelectTrigger>
                <SelectValue placeholder="Op" />
              </SelectTrigger>
              <SelectContent>
                {FILTER_OPERATIONS.map((operation) => (
                  <SelectItem key={operation.value} value={operation.value}>
                    {operation.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Input
              value={filterValue}
              onChange={(event) => setFilterValue(event.target.value)}
              placeholder={filterRequiresValue ? 'Value' : 'No value'}
              disabled={filterField === ALL_FILTER_FIELDS || !filterRequiresValue}
            />
            <Button
              type="button"
              variant="ghost"
              size="icon"
              onClick={handleClearRecordFilters}
              disabled={!hasActiveRecordFilters}
              aria-label="Clear record filters"
              title="Clear record filters"
              className="h-9 w-9"
            >
              <X className="h-4 w-4" />
            </Button>
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
              <CardTitle className="mt-4 text-lg">
                {hasActiveRecordFilters ? 'No matching records' : 'No records'}
              </CardTitle>
              <CardDescription className="mt-2">
                {hasActiveRecordFilters ? 'Try another search or filter.' : 'Get started by creating a new record.'}
              </CardDescription>
              <div className="mt-6">
                {hasActiveRecordFilters ? (
                  <Button variant="outline" onClick={handleClearRecordFilters}>
                    Clear filters
                  </Button>
                ) : (
                  <Button onClick={handleCreateRecord}>
                    Create Record
                  </Button>
                )}
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
