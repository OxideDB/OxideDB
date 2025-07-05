import React from 'react';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Input } from '@/components/ui/input';
import { Plus, Eye, Edit3, Trash2, Lock, Users, Shield, RotateCcw, Unlock } from 'lucide-react';
import type { 
  CollectionPermissions, 
  CrudOperation, 
  AuthOperation,
  PermissionLevel,
  CollectionSchema
} from '../types/api';

interface PermissionRuleEditorProps {
  permissions: CollectionPermissions;
  onChange: (permissions: CollectionPermissions) => void;
  schema: CollectionSchema | null;
}

/**
 * Component for editing permission rules for CRUD and Auth operations
 * Provides a form interface for configuring access control rules
 */
export const PermissionRuleEditor: React.FC<PermissionRuleEditorProps> = ({
  permissions,
  onChange,
  schema,
}) => {
  // Determine collection type and appropriate operations
  const isAuthCollection = schema?.collection_type === 'auth';
  const crudOperations: CrudOperation[] = ['create', 'read', 'update', 'delete', 'list'];
  const authOperations: AuthOperation[] = [
    'login', 'register', 'token_validation', 'token_refresh', 
    'logout', 'get_current_user', 'list_auth_collections'
  ];

  const updateCrudOperation = (operation: CrudOperation, level: PermissionLevel, filter?: string) => {
    const newCrudRules = { ...permissions.crud_rules };
    newCrudRules[operation] = {
      operation,
      permission: level,
      filter: filter || undefined,
    };
    
    onChange({
      ...permissions,
      crud_rules: newCrudRules,
    });
  };

  const updateAuthOperation = (operation: AuthOperation, level: PermissionLevel, filter?: string) => {
    const newAuthRules = { ...permissions.auth_rules };
    newAuthRules[operation] = {
      operation,
      permission: level,
      filter: filter || undefined,
    };
    
    onChange({
      ...permissions,
      auth_rules: newAuthRules,
    });
  };

  const getCrudOperationIcon = (operation: CrudOperation) => {
    switch (operation) {
      case 'create': return <Plus className="w-4 h-4" />;
      case 'read': return <Eye className="w-4 h-4" />;
      case 'update': return <Edit3 className="w-4 h-4" />;
      case 'delete': return <Trash2 className="w-4 h-4" />;
      case 'list': return <Eye className="w-4 h-4" />;
      default: return <Lock className="w-4 h-4" />;
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
      default: return <Lock className="w-4 h-4" />;
    }
  };

  return (
    <div className="space-y-6">
      {/* CRUD Operations Section */}
      <div className="space-y-4">
        <h3 className="text-lg font-medium">CRUD Operations</h3>
        <div className="grid gap-4">
          {crudOperations.map((operation) => {
            const rule = permissions.crud_rules[operation];
            return (
              <div key={operation} className="flex items-center space-x-4 p-4 border rounded-lg">
                <div className="flex items-center space-x-2 min-w-0 flex-1">
                  {getCrudOperationIcon(operation)}
                  <span className="font-medium capitalize">
                    {operation.replace(/([A-Z])/g, ' $1').toLowerCase()}
                  </span>
                </div>
                
                <div className="flex-1">
                  <Select
                    value={typeof rule?.permission === 'object' ? 'rule' : rule?.permission || 'none'}
                    onValueChange={(value: string) => {
                      if (value === 'rule') {
                        updateCrudOperation(operation, { rule: '@request.auth.id != null' });
                      } else {
                        updateCrudOperation(operation, value as PermissionLevel);
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
                      onChange={(e) => updateCrudOperation(operation, { rule: e.target.value })}
                    />
                  </div>
                )}

                {rule?.filter !== undefined && (
                  <div className="flex-1">
                    <Input
                      placeholder="Filter expression (optional)"
                      value={rule.filter || ''}
                      onChange={(e) => updateCrudOperation(operation, rule.permission, e.target.value)}
                    />
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* Auth Operations Section - Only show for auth collections */}
      {isAuthCollection && (
        <div className="space-y-4">
          <h3 className="text-lg font-medium">Authentication Operations</h3>
          <div className="grid gap-4">
            {authOperations.map((operation) => {
              const rule = permissions.auth_rules[operation];
              return (
                <div key={operation} className="flex items-center space-x-4 p-4 border rounded-lg bg-blue-50">
                  <div className="flex items-center space-x-2 min-w-0 flex-1">
                    {getAuthOperationIcon(operation)}
                    <span className="font-medium capitalize">
                      {operation.replace(/([A-Z])/g, ' $1').toLowerCase()}
                    </span>
                  </div>
                  
                  <div className="flex-1">
                    <Select
                      value={typeof rule?.permission === 'object' ? 'rule' : rule?.permission || 'none'}
                      onValueChange={(value: string) => {
                        if (value === 'rule') {
                          updateAuthOperation(operation, { rule: '@request.auth.id != null' });
                        } else {
                          updateAuthOperation(operation, value as PermissionLevel);
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
                        onChange={(e) => updateAuthOperation(operation, { rule: e.target.value })}
                      />
                    </div>
                  )}

                  {rule?.filter !== undefined && (
                    <div className="flex-1">
                      <Input
                        placeholder="Filter expression (optional)"
                        value={rule.filter || ''}
                        onChange={(e) => updateAuthOperation(operation, rule.permission, e.target.value)}
                      />
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}; 