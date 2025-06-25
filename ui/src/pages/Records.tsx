import React, { useState, useEffect } from 'react';
import { useParams, Link, useNavigate } from 'react-router-dom';
import { ArrowLeft, Edit, Database, Trash2, Plus, Download, Upload, Shield, Lock, Unlock, Users, Settings, Eye, Edit3, RotateCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Separator } from '@/components/ui/separator';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Textarea } from '@/components/ui/textarea';
import { Input } from '@/components/ui/input';
import { RecordTable } from '@/components/RecordTable';
import PageLayout from '@/components/PageLayout';
import { apiService } from '../services/api';
import type { 
  DbRecord, 
  CollectionSchema, 
  FieldDefinition, 
  CollectionPermissions, 
  CrudOperation, 
  PermissionLevel, 
  PermissionPresetType,
  FileReference 
} from '../types/api';

const Records: React.FC = () => {
  const { collection } = useParams<{ collection: string }>();
  const navigate = useNavigate();
  const [records, setRecords] = useState<DbRecord[]>([]);
  const [schema, setSchema] = useState<CollectionSchema | null>(null);
  const [permissions, setPermissions] = useState<CollectionPermissions | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isRuleDialogOpen, setIsRuleDialogOpen] = useState(false);
  const [editingPermissions, setEditingPermissions] = useState<CollectionPermissions | null>(null);

  // Mock data for features not yet implemented
  const collectionInfo = {
    name: collection || '',
    description: "User account information and profiles",
    recordCount: records.length,
    size: "2.4 MB", // Placeholder
    created: schema ? new Date(Number(schema.created_at) * 1000).toLocaleDateString() : "2024-01-15",
    lastModified: "2 hours ago", // Placeholder
    status: "active",
  };



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
      
      // Fetch records, schema, and permissions in parallel
      const [recordsData, schemaData, permissionsData] = await Promise.all([
        apiService.getRecords(collection),
        apiService.getCollectionSchema(collection).catch(() => null), // Don't fail if schema doesn't exist
        apiService.getPermissions().then(response => 
          response.data.find(p => p.collection_name === collection)?.permissions || null
        ).catch(() => null) // Don't fail if permissions don't exist
      ]);
      
      setRecords(recordsData);
      setSchema(schemaData);
      setPermissions(permissionsData);
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

  // Permission helper functions
  const getPermissionLevelDisplay = (level: PermissionLevel): { text: string; color: string; icon: React.ReactNode } => {
    if (typeof level === 'object' && 'rule' in level) {
      return { text: 'Custom Rule', color: 'bg-purple-100 text-purple-800', icon: <Settings className="w-3 h-3" /> };
    }
    
    switch (level) {
      case 'none':
        return { text: 'None', color: 'bg-gray-100 text-gray-800', icon: <Lock className="w-3 h-3" /> };
      case 'public':
        return { text: 'Public', color: 'bg-green-100 text-green-800', icon: <Unlock className="w-3 h-3" /> };
      case 'authenticatedonly':
        return { text: 'Authenticated', color: 'bg-blue-100 text-blue-800', icon: <Users className="w-3 h-3" /> };
      case 'superuseronly':
        return { text: 'Superuser Only', color: 'bg-red-100 text-red-800', icon: <Shield className="w-3 h-3" /> };
      default:
        return { text: 'Unknown', color: 'bg-gray-100 text-gray-800', icon: <Lock className="w-3 h-3" /> };
    }
  };

  const getOperationIcon = (operation: CrudOperation) => {
    switch (operation) {
      case 'create': return <Plus className="w-4 h-4" />;
      case 'read': return <Eye className="w-4 h-4" />;
      case 'update': return <Edit3 className="w-4 h-4" />;
      case 'delete': return <Trash2 className="w-4 h-4" />;
      case 'list': return <Eye className="w-4 h-4" />;
      default: return <Settings className="w-4 h-4" />;
    }
  };

  // Permission management functions
  const updateCollectionPermissions = async (updatedPermissions: CollectionPermissions) => {
    if (!collection) return;
    
    try {
      await apiService.updateCollectionPermissions(collection, updatedPermissions);
      await fetchData(); // Reload to get updated data
      setEditingPermissions(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update permissions');
    }
  };

  const resetPermissions = async () => {
    if (!collection) return;
    
    try {
      await apiService.resetCollectionPermissions(collection);
      await fetchData();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to reset permissions');
    }
  };

  const applyPreset = async (preset: PermissionPresetType) => {
    if (!collection) return;
    
    try {
      await apiService.applyPermissionPreset(collection, preset);
      await fetchData();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to apply preset');
    }
  };

  // Permission Rule Editor Component
  const PermissionRuleEditor: React.FC<{
    permissions: CollectionPermissions;
    onChange: (permissions: CollectionPermissions) => void;
  }> = ({ permissions, onChange }) => {
    const operations: CrudOperation[] = ['create', 'read', 'update', 'delete', 'list'];

    const updateOperation = (operation: CrudOperation, level: PermissionLevel, filter?: string) => {
      const newRules = { ...permissions.rules };
      newRules[operation] = {
        operation,
        permission: level,
        filter: filter || undefined,
      };
      
      onChange({
        ...permissions,
        rules: newRules,
      });
    };

    return (
      <div className="space-y-4">
        <div className="grid gap-4">
          {operations.map((operation) => {
            const rule = permissions.rules[operation];
            return (
              <div key={operation} className="flex items-center space-x-4 p-4 border rounded-lg">
                <div className="flex items-center space-x-2 min-w-0 flex-1">
                  {getOperationIcon(operation)}
                  <span className="font-medium capitalize">{operation}</span>
                </div>
                
                <div className="flex-1">
                  <Select
                    value={typeof rule?.permission === 'object' ? 'rule' : rule?.permission || 'none'}
                    onValueChange={(value: string) => {
                      if (value === 'rule') {
                        updateOperation(operation, { rule: '@request.auth.id != null' });
                      } else {
                        updateOperation(operation, value as PermissionLevel);
                      }
                    }}
                  >
                    <SelectTrigger>
                      <SelectValue placeholder="Select permission" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="none">None</SelectItem>
                      <SelectItem value="public">Public</SelectItem>
                      <SelectItem value="authenticatedonly">Authenticated Only</SelectItem>
                      <SelectItem value="superuseronly">Superuser Only</SelectItem>
                      <SelectItem value="rule">Custom Rule</SelectItem>
                    </SelectContent>
                  </Select>
                </div>

                {typeof rule?.permission === 'object' && 'rule' in rule.permission && (
                  <div className="flex-1">
                    <Input
                      placeholder="Custom rule expression"
                      value={rule.permission.rule}
                      onChange={(e) => updateOperation(operation, { rule: e.target.value })}
                    />
                  </div>
                )}

                {rule?.filter !== undefined && (
                  <div className="flex-1">
                    <Input
                      placeholder="Filter expression (optional)"
                      value={rule.filter || ''}
                      onChange={(e) => updateOperation(operation, rule.permission, e.target.value)}
                    />
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    );
  };

  // Helper function to get display string for field type
  const getFieldTypeDisplay = (fieldType: any): string => {
    if (typeof fieldType === 'string') {
      return fieldType;
    } else if (typeof fieldType === 'object' && fieldType !== null) {
      if ('relationship' in fieldType) {
        return 'relationship';
      }
      if ('file' in fieldType) {
        return 'file';
      }
      // Handle other object field types if needed
      return 'unknown';
    }
    return 'unknown';
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
      <PageLayout title="Loading..." description="Loading collection data...">
        <div className="flex items-center justify-center h-64">
          <div className="text-muted-foreground">Loading records...</div>
        </div>
      </PageLayout>
    );
  }

  // Convert schema fields to array format for easier rendering
  const schemaFields = schema ? Object.entries(schema.fields).map(([name, field]) => ({
    name,
    type: getFieldTypeDisplay(field.field_type),
    required: field.required || false,
    unique: false, // Placeholder - not in current schema
    indexed: false, // Placeholder - not in current schema
  })) : [];

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

      {/* Collection Overview */}
      <div className="grid gap-3 md:gap-4 grid-cols-2 md:grid-cols-4">
        <Card>
          <CardContent className="p-4">
            <div className="text-xl md:text-2xl font-bold">{collectionInfo.recordCount.toLocaleString()}</div>
            <p className="text-xs text-muted-foreground">Total Records</p>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-xl md:text-2xl font-bold">{collectionInfo.size}</div>
            <p className="text-xs text-muted-foreground">Storage Size</p>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-xl md:text-2xl font-bold">{schemaFields.length}</div>
            <p className="text-xs text-muted-foreground">Fields</p>
          </CardContent>
        </Card>
        <Card>
          <CardContent className="p-4">
            <div className="text-xl md:text-2xl font-bold">{schemaFields.filter((f) => f.indexed).length}</div>
            <p className="text-xs text-muted-foreground">Indexes</p>
          </CardContent>
        </Card>
      </div>

      {/* Collection Info */}
      <Card>
        <CardHeader>
          <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
            <div className="min-w-0 flex-1">
              <CardTitle className="flex items-center gap-2">
                <Database className="h-5 w-5 flex-shrink-0" />
                <span className="truncate">Collection Details</span>
              </CardTitle>
              <CardDescription className="line-clamp-2">{collectionInfo.description}</CardDescription>
            </div>
            <div className="flex flex-col sm:flex-row gap-2">
              <Link to={`/collections/${encodeURIComponent(collection!)}/edit`}>
                <Button variant="outline" size="sm" className="w-full sm:w-auto">
                  <Edit className="h-4 w-4 mr-2" />
                  Edit Schema
                </Button>
              </Link>
              <Button variant="outline" size="sm" className="text-red-600 w-full sm:w-auto">
                <Trash2 className="h-4 w-4 mr-2" />
                Delete
              </Button>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 text-sm">
            <div>
              <span className="text-muted-foreground">Created:</span>
              <div className="font-medium">{collectionInfo.created}</div>
            </div>
            <div>
              <span className="text-muted-foreground">Last Modified:</span>
              <div className="font-medium">{collectionInfo.lastModified}</div>
            </div>
            <div>
              <span className="text-muted-foreground">Status:</span>
              <div>
                <Badge className="bg-green-100 text-green-800">{collectionInfo.status}</Badge>
              </div>
            </div>
            <div>
              <span className="text-muted-foreground">Type:</span>
              <div className="font-medium">{schema?.collection_type || 'User Collection'}</div>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Access Rules */}
      {editingPermissions ? (
        <Card>
          <CardHeader>
            <CardTitle>Edit Permissions: {collection}</CardTitle>
            <CardDescription>
              Configure access control rules for each CRUD operation
            </CardDescription>
          </CardHeader>
          <CardContent>
            <PermissionRuleEditor
              permissions={editingPermissions}
              onChange={setEditingPermissions}
            />
            <div className="flex space-x-2 mt-6">
              <Button 
                onClick={() => updateCollectionPermissions(editingPermissions)}
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
        <Card>
          <CardHeader>
            <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
              <div>
                <CardTitle className="flex items-center gap-2">
                  <Shield className="h-5 w-5" />
                  Access Rules
                </CardTitle>
                <CardDescription>Control who can access this collection</CardDescription>
              </div>
              <div className="flex flex-col sm:flex-row gap-2">
                <Select onValueChange={(preset: string) => applyPreset(preset as PermissionPresetType)}>
                  <SelectTrigger className="w-full sm:w-40">
                    <SelectValue placeholder="Apply Preset" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="public">Public</SelectItem>
                    <SelectItem value="authenticated_only">Authenticated Only</SelectItem>
                    <SelectItem value="superuser_only">Superuser Only</SelectItem>
                    <SelectItem value="read_only">Read Only</SelectItem>
                  </SelectContent>
                </Select>
                <Button 
                  variant="outline" 
                  size="sm"
                  onClick={() => resetPermissions()}
                  className="w-full sm:w-auto"
                >
                  <RotateCcw className="h-4 w-4 mr-2" />
                  Reset
                </Button>
                <Button 
                  size="sm"
                  onClick={() => permissions && setEditingPermissions(permissions)}
                  disabled={!permissions}
                  className="w-full sm:w-auto"
                >
                  <Edit className="h-4 w-4 mr-2" />
                  Edit Rules
                </Button>
              </div>
            </div>
          </CardHeader>
          <CardContent>
            {permissions ? (
              <>
                {/* Mobile view - Cards */}
                <div className="block md:hidden space-y-3">
                  {Object.entries(permissions.rules).map(([operation, rule]) => {
                    const display = getPermissionLevelDisplay(rule.permission);
                    return (
                      <Card key={operation} className="p-4">
                        <div className="space-y-3">
                          <div className="flex items-center justify-between">
                            <div className="flex items-center space-x-2">
                              {getOperationIcon(operation as CrudOperation)}
                              <span className="font-medium capitalize">{operation}</span>
                            </div>
                            <Badge className={display.color}>
                              <span className="flex items-center space-x-1">
                                {display.icon}
                                <span className="text-xs">{display.text}</span>
                              </span>
                            </Badge>
                          </div>
                          {typeof rule.permission === 'object' && 'rule' in rule.permission && (
                            <div>
                              <div className="font-mono text-sm bg-gray-50 p-2 rounded border break-all">
                                {rule.permission.rule}
                              </div>
                            </div>
                          )}
                          {rule.filter && (
                            <div>
                              <span className="text-sm text-muted-foreground">Filter: </span>
                              <code className="text-sm bg-gray-100 px-2 py-1 rounded">{rule.filter}</code>
                            </div>
                          )}
                        </div>
                      </Card>
                    );
                  })}
                </div>

                {/* Desktop view - Table */}
                <div className="hidden md:block">
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead>Operation</TableHead>
                        <TableHead>Permission Level</TableHead>
                        <TableHead>Custom Rule</TableHead>
                        <TableHead>Filter</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {Object.entries(permissions.rules).map(([operation, rule]) => {
                        const display = getPermissionLevelDisplay(rule.permission);
                        return (
                          <TableRow key={operation}>
                            <TableCell>
                              <div className="flex items-center space-x-2">
                                {getOperationIcon(operation as CrudOperation)}
                                <span className="font-medium capitalize">{operation}</span>
                              </div>
                            </TableCell>
                            <TableCell>
                              <Badge className={display.color}>
                                <span className="flex items-center space-x-1">
                                  {display.icon}
                                  <span className="text-xs">{display.text}</span>
                                </span>
                              </Badge>
                            </TableCell>
                            <TableCell>
                              {typeof rule.permission === 'object' && 'rule' in rule.permission ? (
                                <code className="text-xs bg-gray-100 px-2 py-1 rounded max-w-xs block truncate">
                                  {rule.permission.rule}
                                </code>
                              ) : (
                                <span className="text-muted-foreground">-</span>
                              )}
                            </TableCell>
                            <TableCell>
                              {rule.filter ? (
                                <code className="text-xs bg-gray-100 px-2 py-1 rounded max-w-xs block truncate">
                                  {rule.filter}
                                </code>
                              ) : (
                                <span className="text-muted-foreground">-</span>
                              )}
                            </TableCell>
                          </TableRow>
                        );
                      })}
                    </TableBody>
                  </Table>
                </div>
              </>
            ) : (
              <div className="text-center py-8">
                <Shield className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
                <h3 className="text-lg font-medium mb-2">No permissions configured</h3>
                <p className="text-muted-foreground mb-4">Set up access rules to control who can access this collection</p>
                <div className="flex flex-col sm:flex-row gap-2 justify-center">
                  <Select onValueChange={(preset: string) => applyPreset(preset as PermissionPresetType)}>
                    <SelectTrigger className="w-full sm:w-48">
                      <SelectValue placeholder="Apply Permission Preset" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="public">Public Access</SelectItem>
                      <SelectItem value="authenticated_only">Authenticated Only</SelectItem>
                      <SelectItem value="superuser_only">Superuser Only</SelectItem>
                      <SelectItem value="read_only">Read Only</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
              </div>
            )}
          </CardContent>
        </Card>
      )}

      {/* Schema - Mobile optimized */}
      <Card>
        <CardHeader>
          <CardTitle>Schema</CardTitle>
          <CardDescription>Field definitions and constraints</CardDescription>
        </CardHeader>
        <CardContent>
          {schemaFields.length > 0 ? (
            <>
              {/* Mobile view - Cards */}
              <div className="block md:hidden space-y-3">
                {schemaFields.map((field) => (
                  <Card key={field.name} className="p-4">
                    <div className="space-y-2">
                      <div className="flex items-center justify-between">
                        <h4 className="font-medium">{field.name}</h4>
                        <Badge variant="outline">{field.type}</Badge>
                      </div>
                      <div className="flex flex-wrap gap-2">
                        {field.required && <Badge className="bg-red-100 text-red-800 text-xs">Required</Badge>}
                        {field.unique && <Badge className="bg-blue-100 text-blue-800 text-xs">Unique</Badge>}
                        {field.indexed && <Badge className="bg-green-100 text-green-800 text-xs">Indexed</Badge>}
                      </div>
                    </div>
                  </Card>
                ))}
              </div>

              {/* Desktop view - Table */}
              <div className="hidden md:block">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Field Name</TableHead>
                      <TableHead>Type</TableHead>
                      <TableHead>Required</TableHead>
                      <TableHead>Unique</TableHead>
                      <TableHead>Indexed</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {schemaFields.map((field) => (
                      <TableRow key={field.name}>
                        <TableCell className="font-medium">{field.name}</TableCell>
                        <TableCell>
                          <Badge variant="outline">{field.type}</Badge>
                        </TableCell>
                        <TableCell>
                          {field.required ? (
                            <Badge className="bg-red-100 text-red-800">Required</Badge>
                          ) : (
                            <Badge variant="secondary">Optional</Badge>
                          )}
                        </TableCell>
                        <TableCell>
                          {field.unique ? (
                            <Badge className="bg-blue-100 text-blue-800">Unique</Badge>
                          ) : (
                            <span className="text-muted-foreground">-</span>
                          )}
                        </TableCell>
                        <TableCell>
                          {field.indexed ? (
                            <Badge className="bg-green-100 text-green-800">Indexed</Badge>
                          ) : (
                            <span className="text-muted-foreground">-</span>
                          )}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
            </>
          ) : (
            <div className="text-center py-8">
              <Database className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
              <h3 className="text-lg font-medium mb-2">No schema defined</h3>
              <p className="text-muted-foreground mb-4">Define a schema to structure your collection data</p>
              <Link to={`/collections/${encodeURIComponent(collection!)}/edit`}>
                <Button>
                  <Edit className="h-4 w-4 mr-2" />
                  Define Schema
                </Button>
              </Link>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Recent Records */}
      <Card>
        <CardHeader>
          <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
            <div>
              <CardTitle>Recent Records</CardTitle>
              <CardDescription>Latest records in this collection</CardDescription>
            </div>
            <div className="flex flex-col sm:flex-row gap-2">
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
            />
          )}
        </CardContent>
      </Card>
    </PageLayout>
  );
};

export default Records; 