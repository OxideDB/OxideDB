import React from 'react';
import { Card, CardContent } from '@/components/ui/card';
import { 
  Select, 
  SelectContent, 
  SelectItem, 
  SelectTrigger, 
  SelectValue 
} from '@/components/ui/select';
import { Input } from '@/components/ui/input';
import { Shield } from 'lucide-react';
import type {
  CrudOperation,
  AuthOperation,
  PermissionLevel
} from '@/types/generated';
import type { 
  CollectionPermissionsInfo, 
  CollectionPermissions
} from '@/types/api';
import { 
  getCrudOperationIcon, 
  getAuthOperationIcon, 
  sortCrudOperations, 
  sortAuthOperations 
} from '@/utils/permissions/permissionUtils';

interface PermissionRuleEditorProps {
  permissions: CollectionPermissions;
  permissionsData: CollectionPermissionsInfo[];
  onChange: (permissions: CollectionPermissions) => void;
}

export const PermissionRuleEditor: React.FC<PermissionRuleEditorProps> = ({ 
  permissions, 
  permissionsData, 
  onChange 
}) => {
  const collectionInfo = permissionsData.find(info => info.collection_name === permissions.collection);
  const isAuthCollection = collectionInfo?.collection_type === 'auth';

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

  return (
    <div className="space-y-10">
      {/* Enhanced CRUD Operations Section */}
      <div className="space-y-6">
        <div className="space-y-3">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-blue-100 rounded-lg">
              <Shield className="h-5 w-5 text-blue-600" />
            </div>
            <div>
              <h3 className="text-xl font-bold">CRUD Operations</h3>
              <p className="text-sm text-muted-foreground">
                Configure access permissions for create, read, update, delete, and list operations.
              </p>
            </div>
          </div>
        </div>
        
        <div className="space-y-4">
          {sortCrudOperations(permissions.crud_rules).map(([operation, rule]) => {
            return (
              <Card key={operation} className="border-2 hover:border-primary/30 transition-all duration-200">
                <CardContent className="p-6">
                  <div className="space-y-6">
                    {/* Operation Header */}
                    <div className="flex items-center justify-between">
                      <div className="flex items-center space-x-3">
                        <div className="flex-shrink-0 p-2.5 rounded-lg bg-gradient-to-br from-blue-50 to-blue-100 border border-blue-200">
                          {getCrudOperationIcon(operation as CrudOperation)}
                        </div>
                        <div>
                          <div className="font-bold capitalize text-lg">{operation}</div>
                          <div className="text-sm text-muted-foreground">
                            {operation === 'create' && 'Add new records to the collection'}
                            {operation === 'read' && 'View and access individual records'}
                            {operation === 'update' && 'Modify existing record data'}
                            {operation === 'delete' && 'Remove records from the collection'}
                            {operation === 'list' && 'Browse and search all records'}
                          </div>
                        </div>
                      </div>
                      <div className="text-xs text-muted-foreground bg-muted px-2 py-1 rounded-md">
                        CRUD
                      </div>
                    </div>

                    {/* Permission Configuration */}
                    <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                      {/* Permission Level */}
                      <div className="space-y-3">
                        <label className="text-sm font-semibold text-foreground flex items-center gap-2">
                          <div className="w-2 h-2 bg-primary rounded-full"></div>
                          Permission Level
                        </label>
                        <Select
                          value={typeof rule?.permission === 'object' ? 'rule' : rule?.permission || 'none'}
                          onValueChange={(value: string) => {
                            if (value === 'rule') {
                              updateCrudOperation(operation as CrudOperation, { rule: '@request.auth.id != null' });
                            } else {
                              updateCrudOperation(operation as CrudOperation, value as PermissionLevel);
                            }
                          }}
                        >
                          <SelectTrigger className="w-full h-12 border-2">
                            <SelectValue placeholder="Select permission level" />
                          </SelectTrigger>
                          <SelectContent>
                            <SelectItem value="none" className="py-3">
                              <div className="flex items-center gap-3">
                                <span>🔒</span>
                                <div>
                                  <div className="font-medium">None</div>
                                  <div className="text-xs text-muted-foreground">No access allowed</div>
                                </div>
                              </div>
                            </SelectItem>
                            <SelectItem value="public" className="py-3">
                              <div className="flex items-center gap-3">
                                <span>🌍</span>
                                <div>
                                  <div className="font-medium">Public</div>
                                  <div className="text-xs text-muted-foreground">Anyone can access</div>
                                </div>
                              </div>
                            </SelectItem>
                            <SelectItem value="authenticatedonly" className="py-3">
                              <div className="flex items-center gap-3">
                                <span>👤</span>
                                <div>
                                  <div className="font-medium">Authenticated Only</div>
                                  <div className="text-xs text-muted-foreground">Logged in users only</div>
                                </div>
                              </div>
                            </SelectItem>
                            <SelectItem value="superuseronly" className="py-3">
                              <div className="flex items-center gap-3">
                                <span>👑</span>
                                <div>
                                  <div className="font-medium">Superuser Only</div>
                                  <div className="text-xs text-muted-foreground">Admin access required</div>
                                </div>
                              </div>
                            </SelectItem>
                            <SelectItem value="rule" className="py-3">
                              <div className="flex items-center gap-3">
                                <span>⚙️</span>
                                <div>
                                  <div className="font-medium">Custom Rule</div>
                                  <div className="text-xs text-muted-foreground">Define custom logic</div>
                                </div>
                              </div>
                            </SelectItem>
                          </SelectContent>
                        </Select>
                      </div>

                      {/* Custom Rule Expression (conditionally shown) */}
                      {typeof rule?.permission === 'object' && 'rule' in rule.permission && (
                        <div className="space-y-3">
                          <label className="text-sm font-semibold text-foreground flex items-center gap-2">
                            <div className="w-2 h-2 bg-orange-500 rounded-full"></div>
                            Custom Rule Expression
                          </label>
                          <Input
                            placeholder="e.g., @request.auth.id = @record.user_id"
                            value={rule.permission.rule}
                            onChange={(e) => updateCrudOperation(operation as CrudOperation, { rule: e.target.value })}
                            className="font-mono text-sm h-12 border-2"
                          />
                          <p className="text-xs text-muted-foreground">
                            Use variables like @request.auth.id, @record.field_name to define access logic
                          </p>
                        </div>
                      )}
                    </div>

                    {/* Filter Expression (conditionally shown) */}
                    {rule?.filter !== undefined && (
                      <div className="space-y-3 pt-4 border-t border-border/50">
                        <label className="text-sm font-semibold text-foreground flex items-center gap-2">
                          <div className="w-2 h-2 bg-green-500 rounded-full"></div>
                          Additional Filter (Optional)
                        </label>
                        <Input
                          placeholder="e.g., status = 'active'"
                          value={rule.filter || ''}
                          onChange={(e) => updateCrudOperation(operation as CrudOperation, rule.permission, e.target.value)}
                          className="font-mono text-sm h-12 border-2"
                        />
                        <p className="text-xs text-muted-foreground">
                          Additional filtering criteria applied to the operation
                        </p>
                      </div>
                    )}
                  </div>
                </CardContent>
              </Card>
            );
          })}
        </div>
      </div>

      {/* Enhanced Auth Operations Section - Only show for auth collections */}
      {isAuthCollection && (
        <div className="space-y-6">
          <div className="space-y-3">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-purple-100 rounded-lg">
                <Shield className="h-5 w-5 text-purple-600" />
              </div>
              <div>
                <h3 className="text-xl font-bold">Authentication Operations</h3>
                <p className="text-sm text-muted-foreground">
                  Configure access permissions for authentication-related operations like login, register, and token management.
                </p>
              </div>
            </div>
          </div>
          
          <div className="space-y-4">
            {permissions.auth_rules && sortAuthOperations(permissions.auth_rules).map(([operation, rule]) => {
              return (
                <Card key={operation} className="border-2 border-purple-200 bg-gradient-to-r from-purple-50/50 to-blue-50/50 hover:border-purple-300 transition-all duration-200">
                  <CardContent className="p-6">
                    <div className="space-y-6">
                      {/* Operation Header */}
                      <div className="flex items-center justify-between">
                        <div className="flex items-center space-x-3">
                          <div className="flex-shrink-0 p-2.5 rounded-lg bg-gradient-to-br from-purple-50 to-purple-100 border border-purple-200">
                            {getAuthOperationIcon(operation as AuthOperation)}
                          </div>
                          <div>
                            <div className="font-bold capitalize text-lg">{operation.replace('_', ' ')}</div>
                            <div className="text-sm text-muted-foreground">
                              {operation === 'login' && 'Allow users to sign into the system'}
                              {operation === 'register' && 'Allow new user account creation'}
                              {operation === 'token_validation' && 'Validate authentication tokens'}
                              {operation === 'token_refresh' && 'Refresh expired authentication tokens'}
                              {operation === 'logout' && 'Allow users to sign out of the system'}
                              {operation === 'get_current_user' && 'Retrieve current user information'}
                              {operation === 'list_auth_collections' && 'List available authentication collections'}
                            </div>
                          </div>
                        </div>
                        <div className="text-xs text-purple-700 bg-purple-100 px-2 py-1 rounded-md font-medium">
                          AUTH
                        </div>
                      </div>

                      {/* Permission Configuration */}
                      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                        {/* Permission Level */}
                        <div className="space-y-3">
                          <label className="text-sm font-semibold text-foreground flex items-center gap-2">
                            <div className="w-2 h-2 bg-purple-500 rounded-full"></div>
                            Permission Level
                          </label>
                          <Select
                            value={typeof rule?.permission === 'object' ? 'rule' : rule?.permission || 'none'}
                            onValueChange={(value: string) => {
                              if (value === 'rule') {
                                updateAuthOperation(operation as AuthOperation, { rule: '@request.headers.x-api-key != ""' });
                              } else {
                                updateAuthOperation(operation as AuthOperation, value as PermissionLevel);
                              }
                            }}
                          >
                            <SelectTrigger className="w-full h-12 border-2">
                              <SelectValue placeholder="Select permission level" />
                            </SelectTrigger>
                            <SelectContent>
                              <SelectItem value="none" className="py-3">
                                <div className="flex items-center gap-3">
                                  <span>🔒</span>
                                  <div>
                                    <div className="font-medium">None</div>
                                    <div className="text-xs text-muted-foreground">Operation disabled</div>
                                  </div>
                                </div>
                              </SelectItem>
                              <SelectItem value="public" className="py-3">
                                <div className="flex items-center gap-3">
                                  <span>🌍</span>
                                  <div>
                                    <div className="font-medium">Public</div>
                                    <div className="text-xs text-muted-foreground">No restrictions</div>
                                  </div>
                                </div>
                              </SelectItem>
                              <SelectItem value="authenticatedonly" className="py-3">
                                <div className="flex items-center gap-3">
                                  <span>👤</span>
                                  <div>
                                    <div className="font-medium">Authenticated Only</div>
                                    <div className="text-xs text-muted-foreground">Valid session required</div>
                                  </div>
                                </div>
                              </SelectItem>
                              <SelectItem value="superuseronly" className="py-3">
                                <div className="flex items-center gap-3">
                                  <span>👑</span>
                                  <div>
                                    <div className="font-medium">Superuser Only</div>
                                    <div className="text-xs text-muted-foreground">Admin privileges required</div>
                                  </div>
                                </div>
                              </SelectItem>
                              <SelectItem value="rule" className="py-3">
                                <div className="flex items-center gap-3">
                                  <span>⚙️</span>
                                  <div>
                                    <div className="font-medium">Custom Rule</div>
                                    <div className="text-xs text-muted-foreground">Custom authorization logic</div>
                                  </div>
                                </div>
                              </SelectItem>
                            </SelectContent>
                          </Select>
                        </div>

                        {/* Custom Rule Expression */}
                        {typeof rule?.permission === 'object' && 'rule' in rule.permission && (
                          <div className="space-y-3">
                            <label className="text-sm font-semibold text-foreground flex items-center gap-2">
                              <div className="w-2 h-2 bg-orange-500 rounded-full"></div>
                              Custom Rule Expression
                            </label>
                            <Input
                              placeholder="e.g., @request.headers.x-api-key != ''"
                              value={rule.permission.rule}
                              onChange={(e) => updateAuthOperation(operation as AuthOperation, { rule: e.target.value })}
                              className="font-mono text-sm h-12 border-2"
                            />
                            <p className="text-xs text-muted-foreground">
                              Define custom authentication requirements using request context
                            </p>
                          </div>
                        )}
                      </div>

                      {/* Filter Expression */}
                      {rule?.filter !== undefined && (
                        <div className="space-y-3 pt-4 border-t border-purple-200/50">
                          <label className="text-sm font-semibold text-foreground flex items-center gap-2">
                            <div className="w-2 h-2 bg-green-500 rounded-full"></div>
                            Additional Filter (Optional)
                          </label>
                          <Input
                            placeholder="e.g., role = 'admin'"
                            value={rule.filter || ''}
                            onChange={(e) => updateAuthOperation(operation as AuthOperation, rule.permission, e.target.value)}
                            className="font-mono text-sm h-12 border-2"
                          />
                          <p className="text-xs text-muted-foreground">
                            Additional filtering criteria for this authentication operation
                          </p>
                        </div>
                      )}
                    </div>
                  </CardContent>
                </Card>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
};
