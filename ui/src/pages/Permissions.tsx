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
  Settings
} from 'lucide-react';
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
      <div className="space-y-6">
        <div className="flex items-center space-x-2">
          <Shield className="w-6 h-6" />
          <h1 className="text-2xl font-bold">Collection Permissions</h1>
        </div>
        <div className="animate-pulse">
          <div className="grid gap-4">
            {[1, 2, 3].map((i) => (
              <div key={i} className="h-24 bg-gray-200 rounded-lg"></div>
            ))}
          </div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="space-y-6">
        <div className="flex items-center space-x-2">
          <Shield className="w-6 h-6" />
          <h1 className="text-2xl font-bold">Collection Permissions</h1>
        </div>
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
        <Button onClick={loadPermissions}>Retry</Button>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center space-x-2">
          <Shield className="w-6 h-6" />
          <h1 className="text-2xl font-bold">Collection Permissions</h1>
        </div>
        <Button onClick={loadPermissions} variant="outline">
          <RotateCcw className="w-4 h-4 mr-2" />
          Refresh
        </Button>
      </div>

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
        <div className="grid gap-4">
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
      )}
    </div>
  );
};

export default Permissions; 