import React from 'react';
import { Shield, Lock, Unlock, Users, Settings, Eye, Edit3, RotateCcw, Plus, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import type { 
  CollectionPermissions, 
  CrudOperation, 
  AuthOperation,
  PermissionLevel, 
  PermissionPresetType,
  CollectionSchema
} from '../types/api';

interface AccessRulesProps {
  permissions: CollectionPermissions | null;
  collection: string;
  schema: CollectionSchema | null;
  onEdit: () => void;
  onReset: () => Promise<void>;
  onApplyPreset: (preset: PermissionPresetType) => Promise<void>;
}

/**
 * Component for displaying and managing collection access rules
 * Shows CRUD and Auth operation permissions with mobile-responsive design
 */
export const AccessRules: React.FC<AccessRulesProps> = ({
  permissions,
  schema,
  onEdit,
  onReset,
  onApplyPreset,
}) => {
  // Determine collection type and appropriate operations
  const isAuthCollection = schema?.collection_type === 'auth';
  const crudOperations: CrudOperation[] = ['create', 'read', 'update', 'delete', 'list'];
  const authOperations: AuthOperation[] = [
    'login', 'register', 'token_validation', 'token_refresh', 
    'logout', 'get_current_user', 'list_auth_collections'
  ];

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

  const getCrudOperationIcon = (operation: CrudOperation) => {
    switch (operation) {
      case 'create': return <Plus className="w-4 h-4" />;
      case 'read': return <Eye className="w-4 h-4" />;
      case 'update': return <Edit3 className="w-4 h-4" />;
      case 'delete': return <Trash2 className="w-4 h-4" />;
      case 'list': return <Eye className="w-4 h-4" />;
      default: return <Settings className="w-4 h-4" />;
    }
  };

  const getAuthOperationIcon = (operation: AuthOperation) => {
    switch (operation) {
      case 'login': return <Lock className="w-4 h-4" />;
      case 'register': return <Users className="w-4 h-4" />;
      case 'token_validation': return <Shield className="w-4 h-4" />;
      case 'token_refresh': return <RotateCcw className="w-4 h-4" />;
      case 'logout': return <Unlock className="w-4 h-4" />;
      case 'get_current_user': return <Users className="w-4 h-4" />;
      case 'list_auth_collections': return <Shield className="w-4 h-4" />;
      default: return <Settings className="w-4 h-4" />;
    }
  };

  return (
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
            <Select onValueChange={(preset: string) => onApplyPreset(preset as PermissionPresetType)}>
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
              onClick={onReset}
              className="w-full sm:w-auto"
            >
              <RotateCcw className="h-4 w-4 mr-2" />
              Reset
            </Button>
            <Button 
              size="sm"
              onClick={onEdit}
              disabled={!permissions}
              className="w-full sm:w-auto"
            >
              <Edit3 className="h-4 w-4 mr-2" />
              Edit Rules
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        {permissions ? (
          <>
            {/* CRUD Operations Section */}
            <div className="space-y-4 mb-6">
              <h3 className="text-lg font-medium">CRUD Operations</h3>
              
              {/* Mobile view - Cards */}
              <div className="block md:hidden space-y-3">
                {crudOperations.map((operation) => {
                  const rule = permissions.crud_rules[operation];
                  if (!rule) return null; // Skip operations without rules
                  const display = getPermissionLevelDisplay(rule.permission);
                  return (
                    <Card key={operation} className="p-4">
                      <div className="space-y-3">
                        <div className="flex items-center justify-between">
                          <div className="flex items-center space-x-2">
                            {getCrudOperationIcon(operation)}
                            <span className="font-medium capitalize">
                              {operation.replace(/([A-Z])/g, ' $1').toLowerCase()}
                            </span>
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
                            <code className="text-sm bg-gray-100 px-2 py-1 rounded">
                              {rule.filter}
                            </code>
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
                    {crudOperations.map((operation) => {
                      const rule = permissions.crud_rules[operation];
                      if (!rule) return null; // Skip operations without rules
                      const display = getPermissionLevelDisplay(rule.permission);
                      return (
                        <TableRow key={operation}>
                          <TableCell>
                            <div className="flex items-center space-x-2">
                              {getCrudOperationIcon(operation)}
                              <span className="font-medium capitalize">
                                {operation.replace(/([A-Z])/g, ' $1').toLowerCase()}
                              </span>
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
            </div>

            {/* Auth Operations Section - Only show for auth collections */}
            {isAuthCollection && (
              <div className="space-y-4">
                <h3 className="text-lg font-medium">Authentication Operations</h3>
                
                {/* Mobile view - Cards */}
                <div className="block md:hidden space-y-3">
                  {authOperations.map((operation) => {
                    const rule = permissions.auth_rules[operation];
                    if (!rule) return null; // Skip operations without rules
                    const display = getPermissionLevelDisplay(rule.permission);
                    return (
                      <Card key={operation} className="p-4 bg-blue-50">
                        <div className="space-y-3">
                          <div className="flex items-center justify-between">
                            <div className="flex items-center space-x-2">
                              {getAuthOperationIcon(operation)}
                              <span className="font-medium capitalize">
                                {operation.replace(/([A-Z])/g, ' $1').toLowerCase()}
                              </span>
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
                              <code className="text-sm bg-gray-100 px-2 py-1 rounded">
                                {rule.filter}
                              </code>
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
                      {authOperations.map((operation) => {
                        const rule = permissions.auth_rules[operation];
                        if (!rule) return null; // Skip operations without rules
                        const display = getPermissionLevelDisplay(rule.permission);
                        return (
                          <TableRow key={operation} className="bg-blue-50">
                            <TableCell>
                              <div className="flex items-center space-x-2">
                                {getAuthOperationIcon(operation)}
                                <span className="font-medium capitalize">
                                  {operation.replace(/([A-Z])/g, ' $1').toLowerCase()}
                                </span>
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
              </div>
            )}
          </>
        ) : (
          <div className="text-center py-8">
            <Shield className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
            <h3 className="text-lg font-medium mb-2">No permissions configured</h3>
            <p className="text-muted-foreground mb-4">
              Set up access rules to control who can access this collection
            </p>
            <div className="flex flex-col sm:flex-row gap-2 justify-center">
              <Select onValueChange={(preset: string) => onApplyPreset(preset as PermissionPresetType)}>
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
  );
}; 
