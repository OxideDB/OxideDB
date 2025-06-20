import React, { useState, useEffect } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { 
  Select, 
  SelectContent, 
  SelectItem, 
  SelectTrigger, 
  SelectValue 
} from '@/components/ui/select';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { 
  Shield, 
  Users, 
  Lock, 
  Unlock, 
  Eye, 
  Edit3, 
  Trash2, 
  Plus,
  RotateCcw,
  Settings,
  Database,
  Code
} from 'lucide-react';
import PageLayout from '@/components/PageLayout';
import { apiService } from '@/services/api';
import type { 
  CollectionPermissionsInfo, 
  CollectionPermissions,
  CrudOperation,
  PermissionLevel,
  PermissionPresetType
} from '@/types/api';

const Permissions: React.FC = () => {
  const [permissionsData, setPermissionsData] = useState<CollectionPermissionsInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editingPermissions, setEditingPermissions] = useState<CollectionPermissions | null>(null);
  const [isRuleDialogOpen, setIsRuleDialogOpen] = useState(false);
  const [selectedCollection, setSelectedCollection] = useState("");

  useEffect(() => {
    loadPermissions();
  }, []);

  const loadPermissions = async () => {
    try {
      setLoading(true);
      setError(null);
      const response = await apiService.getPermissions();
      setPermissionsData(response.data);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load permissions');
    } finally {
      setLoading(false);
    }
  };

  const updateCollectionPermissions = async (collection: string, permissions: CollectionPermissions) => {
    try {
      await apiService.updateCollectionPermissions(collection, permissions);
      await loadPermissions(); // Reload to get updated data
      setEditingPermissions(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update permissions');
    }
  };

  const resetPermissions = async (collection: string) => {
    try {
      await apiService.resetCollectionPermissions(collection);
      await loadPermissions();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to reset permissions');
    }
  };

  const applyPreset = async (collection: string, preset: PermissionPresetType) => {
    try {
      await apiService.applyPermissionPreset(collection, preset);
      await loadPermissions();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to apply preset');
    }
  };

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

  const getOperationColor = (operation: string) => {
    switch (operation) {
      case "read":
      case "list":
        return "bg-blue-100 text-blue-800"
      case "create":
      case "update":
        return "bg-green-100 text-green-800"
      case "delete":
        return "bg-red-100 text-red-800"
      default:
        return "bg-gray-100 text-gray-800"
    }
  };

  const ruleExamples = [
    {
      title: "Public Access",
      rule: "true",
      description: "Allow unrestricted access",
    },
    {
      title: "API Key Required",
      rule: "@req.headers.x-api-key != ''",
      description: "Require any API key",
    },
    {
      title: "Specific API Key",
      rule: "@req.headers.x-api-key = 'your-secret-key'",
      description: "Require specific API key",
    },
    {
      title: "Authenticated Users",
      rule: "@req.user.id != ''",
      description: "Require authenticated user",
    },
    {
      title: "Admin Only",
      rule: "@req.user.role = 'admin'",
      description: "Admin users only",
    },
    {
      title: "Owner Access",
      rule: "@req.user.id = @record.user_id",
      description: "Users can only access their own records",
    },
    {
      title: "IP Whitelist",
      rule: "@req.headers.x-forwarded-for ~ '192.168.1.*'",
      description: "Allow specific IP range",
    },
    {
      title: "Time-based Access",
      rule: "@now.hour >= 9 && @now.hour <= 17",
      description: "Business hours only (9 AM - 5 PM)",
    },
  ];

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

  if (loading) {
    return (
      <PageLayout title="Collection Permissions">
        <div className="animate-pulse">
          <div className="grid gap-4">
            {[1, 2, 3].map((i) => (
              <div key={i} className="h-24 bg-gray-200 rounded-lg"></div>
            ))}
          </div>
        </div>
      </PageLayout>
    );
  }

  if (error) {
    return (
      <PageLayout title="Collection Permissions">
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
        <Button onClick={loadPermissions}>Retry</Button>
      </PageLayout>
    );
  }

  const headerActions = (
    <Button onClick={loadPermissions} variant="outline">
      <RotateCcw className="w-4 h-4 mr-2" />
      Refresh
    </Button>
  );

  return (
    <PageLayout 
      title="Access Rules" 
      description="Manage access control rules for your collections"
      headerActions={headerActions}
    >

      <Tabs defaultValue="rules" className="w-full">
        <TabsList className="grid w-full grid-cols-3">
          <TabsTrigger value="rules">Access Rules</TabsTrigger>
          <TabsTrigger value="collections">Collection Rules</TabsTrigger>
          <TabsTrigger value="examples">Rule Examples</TabsTrigger>
        </TabsList>

        <TabsContent value="rules" className="space-y-4 md:space-y-6">
          {editingPermissions ? (
            <Card>
              <CardHeader>
                <CardTitle>Edit Permissions: {editingPermissions.collection}</CardTitle>
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
                    onClick={() => updateCollectionPermissions(editingPermissions.collection, editingPermissions)}
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
            <>
              <div className="flex flex-col sm:flex-row gap-3 sm:justify-between sm:items-center">
                <div>
                  <h2 className="text-xl md:text-2xl font-bold">Access Rules</h2>
                  <p className="text-muted-foreground text-sm md:text-base">
                    Define simple rules to control access to your collections
                  </p>
                </div>
                <Dialog open={isRuleDialogOpen} onOpenChange={setIsRuleDialogOpen}>
                  <DialogTrigger asChild>
                    <Button className="w-full sm:w-auto">
                      <Plus className="h-4 w-4 mr-2" />
                      Add Rule
                    </Button>
                  </DialogTrigger>
                  <DialogContent className="max-w-2xl mx-4">
                    <DialogHeader>
                      <DialogTitle>Create Access Rule</DialogTitle>
                      <DialogDescription>Define a new access rule for a collection</DialogDescription>
                    </DialogHeader>
                    <div className="space-y-4">
                      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                        <div>
                          <Label htmlFor="rule-collection">Collection</Label>
                          <Select value={selectedCollection} onValueChange={setSelectedCollection}>
                            <SelectTrigger>
                              <SelectValue placeholder="Select collection" />
                            </SelectTrigger>
                            <SelectContent>
                              {permissionsData.map((info) => (
                                <SelectItem key={info.collection_name} value={info.collection_name}>
                                  {info.collection_name}
                                </SelectItem>
                              ))}
                            </SelectContent>
                          </Select>
                        </div>
                        <div>
                          <Label htmlFor="rule-operation">Operation</Label>
                          <Select>
                            <SelectTrigger>
                              <SelectValue placeholder="Select operation" />
                            </SelectTrigger>
                            <SelectContent>
                              <SelectItem value="create">Create</SelectItem>
                              <SelectItem value="read">Read</SelectItem>
                              <SelectItem value="update">Update</SelectItem>
                              <SelectItem value="delete">Delete</SelectItem>
                              <SelectItem value="list">List</SelectItem>
                            </SelectContent>
                          </Select>
                        </div>
                      </div>
                      <div>
                        <Label htmlFor="rule-expression">Rule Expression</Label>
                        <Textarea
                          id="rule-expression"
                          placeholder="@req.headers.x-api-key = 'your-secret-key'"
                          className="font-mono text-sm min-h-[100px]"
                        />
                        <p className="text-xs text-muted-foreground mt-1">
                          Use expressions like @req.headers.x-api-key, @req.user.id, @record.field_name
                        </p>
                      </div>
                      <div>
                        <Label htmlFor="rule-description">Description</Label>
                        <Input id="rule-description" placeholder="Describe what this rule does..." />
                      </div>
                    </div>
                    <DialogFooter>
                      <Button variant="outline" onClick={() => setIsRuleDialogOpen(false)}>
                        Cancel
                      </Button>
                      <Button onClick={() => setIsRuleDialogOpen(false)}>Create Rule</Button>
                    </DialogFooter>
                  </DialogContent>
                </Dialog>
              </div>

              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    <Shield className="h-5 w-5" />
                    Active Rules
                  </CardTitle>
                  <CardDescription>Manage access rules for your collections</CardDescription>
                </CardHeader>
                <CardContent>
                  <div className="space-y-4">
                    {/* Mobile view - Cards */}
                    <div className="block md:hidden space-y-3">
                      {permissionsData.map((info) => (
                        <Card key={info.collection_name} className="p-4">
                          <div className="space-y-3">
                            <div className="flex items-center justify-between">
                              <div className="flex items-center gap-2">
                                <Badge variant="outline">{info.collection_name}</Badge>
                                <Badge variant={info.collection_type === 'auth' ? 'secondary' : 'default'}>
                                  {info.collection_type}
                                </Badge>
                                {info.has_custom_rules && (
                                  <Badge variant="outline">Custom Rules</Badge>
                                )}
                              </div>
                            </div>
                            <div className="grid grid-cols-5 gap-2">
                              {Object.entries(info.permissions.rules).map(([operation, rule]) => {
                                const display = getPermissionLevelDisplay(rule.permission);
                                return (
                                  <div key={operation} className="flex flex-col items-center space-y-1">
                                    <div className="flex items-center space-x-1">
                                      {getOperationIcon(operation as CrudOperation)}
                                      <span className="text-xs font-medium capitalize">{operation}</span>
                                    </div>
                                    <Badge className={`${display.color} text-xs`}>
                                      <span className="flex items-center space-x-1">
                                        {display.icon}
                                        <span>{display.text}</span>
                                      </span>
                                    </Badge>
                                  </div>
                                );
                              })}
                            </div>
                            <div className="flex gap-2">
                              <Button 
                                size="sm"
                                onClick={() => setEditingPermissions(info.permissions)}
                                className="flex-1"
                              >
                                <Edit3 className="h-4 w-4 mr-2" />
                                Edit
                              </Button>
                              <Button 
                                variant="outline" 
                                size="sm"
                                onClick={() => resetPermissions(info.collection_name)}
                              >
                                Reset
                              </Button>
                            </div>
                          </div>
                        </Card>
                      ))}
                    </div>

                    {/* Desktop view - Table format */}
                    <div className="hidden md:block space-y-4">
                      {permissionsData.map((info) => (
                        <Card key={info.collection_name}>
                          <CardHeader>
                            <div className="flex items-center justify-between">
                              <div className="flex items-center space-x-2">
                                <CardTitle>{info.collection_name}</CardTitle>
                                <Badge variant={info.collection_type === 'auth' ? 'secondary' : 'default'}>
                                  {info.collection_type}
                                </Badge>
                                {info.has_custom_rules && (
                                  <Badge variant="outline">Custom Rules</Badge>
                                )}
                              </div>
                              <div className="flex space-x-2">
                                <Select onValueChange={(preset: string) => applyPreset(info.collection_name, preset as PermissionPresetType)}>
                                  <SelectTrigger className="w-40">
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
                                  onClick={() => resetPermissions(info.collection_name)}
                                >
                                  Reset
                                </Button>
                                <Button 
                                  size="sm"
                                  onClick={() => setEditingPermissions(info.permissions)}
                                >
                                  Edit
                                </Button>
                              </div>
                            </div>
                          </CardHeader>
                          <CardContent>
                            <div className="grid grid-cols-5 gap-4">
                              {Object.entries(info.permissions.rules).map(([operation, rule]) => {
                                const display = getPermissionLevelDisplay(rule.permission);
                                return (
                                  <div key={operation} className="flex flex-col items-center space-y-2">
                                    <div className="flex items-center space-x-1">
                                      {getOperationIcon(operation as CrudOperation)}
                                      <span className="text-sm font-medium capitalize">{operation}</span>
                                    </div>
                                    <Badge className={display.color}>
                                      <span className="flex items-center space-x-1">
                                        {display.icon}
                                        <span className="text-xs">{display.text}</span>
                                      </span>
                                    </Badge>
                                  </div>
                                );
                              })}
                            </div>
                          </CardContent>
                        </Card>
                      ))}
                    </div>
                  </div>
                </CardContent>
              </Card>
            </>
          )}
        </TabsContent>

        <TabsContent value="collections" className="space-y-4 md:space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Database className="h-5 w-5" />
                Collection Rules Overview
              </CardTitle>
              <CardDescription>View rules applied to each collection</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="space-y-4">
                {permissionsData.map((info) => (
                  <Card key={info.collection_name} className="p-4">
                    <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
                      <div className="flex-1">
                        <div className="flex items-center gap-2 mb-1">
                          <h4 className="font-medium">{info.collection_name}</h4>
                          <Badge variant={info.collection_type === 'auth' ? 'secondary' : 'default'}>
                            {info.collection_type}
                          </Badge>
                        </div>
                        <p className="text-sm text-muted-foreground">
                          {info.has_custom_rules ? 'Has custom permission rules configured' : 'Using default permission settings'}
                        </p>
                      </div>
                      <div className="flex flex-wrap gap-2">
                        {Object.entries(info.permissions.rules).map(([operation, rule]) => {
                          const display = getPermissionLevelDisplay(rule.permission);
                          return (
                            <Badge key={operation} className={getOperationColor(operation)}>
                              {operation}
                            </Badge>
                          );
                        })}
                      </div>
                    </div>
                  </Card>
                ))}
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="examples" className="space-y-4 md:space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Code className="h-5 w-5" />
                Rule Examples
              </CardTitle>
              <CardDescription>Common access rule patterns you can use</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="grid gap-4 grid-cols-1 lg:grid-cols-2">
                {ruleExamples.map((example, index) => (
                  <Card key={index} className="p-4">
                    <div className="space-y-3">
                      <div>
                        <h4 className="font-medium">{example.title}</h4>
                        <p className="text-sm text-muted-foreground">{example.description}</p>
                      </div>
                      <div className="font-mono text-sm bg-gray-50 p-3 rounded border break-all">{example.rule}</div>
                      <Button variant="outline" size="sm" className="w-full">
                        Use This Rule
                      </Button>
                    </div>
                  </Card>
                ))}
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Available Variables</CardTitle>
              <CardDescription>Variables you can use in your access rules</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="space-y-4">
                <div>
                  <h4 className="font-medium mb-2">Request Variables</h4>
                  <div className="space-y-2 text-sm">
                    <div className="flex flex-col sm:flex-row sm:items-center gap-2">
                      <code className="bg-gray-100 px-2 py-1 rounded">@req.headers.x-api-key</code>
                      <span className="text-muted-foreground">API key from headers</span>
                    </div>
                    <div className="flex flex-col sm:flex-row sm:items-center gap-2">
                      <code className="bg-gray-100 px-2 py-1 rounded">@req.headers.authorization</code>
                      <span className="text-muted-foreground">Authorization header</span>
                    </div>
                    <div className="flex flex-col sm:flex-row sm:items-center gap-2">
                      <code className="bg-gray-100 px-2 py-1 rounded">@req.user.id</code>
                      <span className="text-muted-foreground">Authenticated user ID</span>
                    </div>
                    <div className="flex flex-col sm:flex-row sm:items-center gap-2">
                      <code className="bg-gray-100 px-2 py-1 rounded">@req.user.role</code>
                      <span className="text-muted-foreground">User role</span>
                    </div>
                  </div>
                </div>
                <div>
                  <h4 className="font-medium mb-2">Record Variables</h4>
                  <div className="space-y-2 text-sm">
                    <div className="flex flex-col sm:flex-row sm:items-center gap-2">
                      <code className="bg-gray-100 px-2 py-1 rounded">@record.field_name</code>
                      <span className="text-muted-foreground">Any field in the record</span>
                    </div>
                    <div className="flex flex-col sm:flex-row sm:items-center gap-2">
                      <code className="bg-gray-100 px-2 py-1 rounded">@record.user_id</code>
                      <span className="text-muted-foreground">Record owner ID</span>
                    </div>
                  </div>
                </div>
                <div>
                  <h4 className="font-medium mb-2">System Variables</h4>
                  <div className="space-y-2 text-sm">
                    <div className="flex flex-col sm:flex-row sm:items-center gap-2">
                      <code className="bg-gray-100 px-2 py-1 rounded">@now</code>
                      <span className="text-muted-foreground">Current timestamp</span>
                    </div>
                    <div className="flex flex-col sm:flex-row sm:items-center gap-2">
                      <code className="bg-gray-100 px-2 py-1 rounded">@now.hour</code>
                      <span className="text-muted-foreground">Current hour (0-23)</span>
                    </div>
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </PageLayout>
  );
};

export default Permissions; 