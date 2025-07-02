import React, { useState } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Shield } from 'lucide-react';
import type { 
  CollectionPermissionsInfo, 
  CollectionPermissions 
} from '@/types/api';
import { PermissionRuleEditor } from './PermissionRuleEditor';
import { AddRuleDialog } from './AddRuleDialog';
import { 
  getPermissionLevelDisplay, 
  sortCrudOperations, 
  sortAuthOperations,
  getCrudOperationIcon,
  getAuthOperationIcon
} from '@/utils/permissions/permissionUtils';

interface AccessRulesTabProps {
  permissionsData: CollectionPermissionsInfo[];
  onUpdatePermissions: (collection: string, permissions: CollectionPermissions) => Promise<void>;
}

export const AccessRulesTab: React.FC<AccessRulesTabProps> = ({
  permissionsData,
  onUpdatePermissions
}) => {
  const [editingPermissions, setEditingPermissions] = useState<CollectionPermissions | null>(null);
  const [isRuleDialogOpen, setIsRuleDialogOpen] = useState(false);

  const handleUpdatePermissions = async (collection: string, permissions: CollectionPermissions) => {
    await onUpdatePermissions(collection, permissions);
    setEditingPermissions(null);
  };

  const handleCreateRule = (rule: any) => {
    // TODO: Implement rule creation logic
    console.log('Creating rule:', rule);
  };

  if (editingPermissions) {
    return (
      <div className="space-y-8">
        {/* Enhanced Back navigation */}
        <div className="flex items-center justify-between">
          <Button 
            variant="ghost" 
            size="sm"
            onClick={() => setEditingPermissions(null)}
            className="flex items-center space-x-2 text-muted-foreground hover:text-foreground transition-colors"
          >
            <span className="text-lg">←</span>
            <span>Back to Collections</span>
          </Button>
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <div className="w-2 h-2 bg-blue-500 rounded-full"></div>
            <span>Editing Mode</span>
          </div>
        </div>
        
        {/* Simplified editing card */}
        <Card className="border-2 shadow-lg">
          <CardHeader className="pb-6 border-b border-border/50">
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <Shield className="h-4 w-4 text-primary" />
                <div>
                  <CardTitle className="text-lg">Edit Permissions</CardTitle>
                  <div className="text-sm text-muted-foreground mt-1">
                    {editingPermissions.collection}
                  </div>
                </div>
              </div>
              <CardDescription>
                Configure access control rules for each CRUD operation. Changes will be applied immediately when you save.
                Use custom filters to create fine-grained access controls.
              </CardDescription>
            </div>
          </CardHeader>
          <CardContent className="p-6 lg:p-8">
            <div className="space-y-8">
              <PermissionRuleEditor
                permissions={editingPermissions}
                permissionsData={permissionsData}
                onChange={setEditingPermissions}
              />
              
              {/* Enhanced action buttons */}
              <div className="flex flex-col sm:flex-row gap-4 pt-6 border-t border-border/50">
                <Button 
                  onClick={() => handleUpdatePermissions(editingPermissions.collection, editingPermissions)}
                  className="flex-1 sm:flex-none min-w-[140px] h-11 font-medium"
                  size="lg"
                >
                  <Shield className="w-4 h-4 mr-2" />
                  Save Changes
                </Button>
                <Button 
                  variant="outline" 
                  onClick={() => setEditingPermissions(null)}
                  className="flex-1 sm:flex-none min-w-[120px] h-11"
                  size="lg"
                >
                  Cancel
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <>
      {/* Simplified intro section that doesn't compete with PageHeader */}
      <div className="flex flex-col space-y-4 sm:flex-row sm:items-center sm:justify-between sm:space-y-0">
        <div className="space-y-2">
          <p className="text-muted-foreground leading-relaxed">
            Configure access control rules for your collections. Define who can perform CRUD operations and authentication actions.
          </p>
          <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
            <span className="flex items-center gap-1">
              <div className="w-1.5 h-1.5 bg-green-500 rounded-full"></div>
              Active Rules
            </span>
            <span className="flex items-center gap-1">
              <div className="w-1.5 h-1.5 bg-orange-500 rounded-full"></div>
              Custom Rules
            </span>
            <span className="flex items-center gap-1">
              <div className="w-1.5 h-1.5 bg-gray-400 rounded-full"></div>
              Default Rules
            </span>
          </div>
        </div>
        <div className="flex-shrink-0">
          <AddRuleDialog
            permissionsData={permissionsData}
            isOpen={isRuleDialogOpen}
            onOpenChange={setIsRuleDialogOpen}
            onCreateRule={handleCreateRule}
          />
        </div>
      </div>

      {/* Main Content with simplified header */}
      <Card className="border-2 shadow-sm">
        <CardHeader className="pb-6 border-b border-border/50">
          <div className="flex items-center justify-between">
            <div className="space-y-1">
              <CardTitle className="flex items-center gap-2 text-lg">
                <Shield className="h-4 w-4 text-primary" />
                Collection Permissions
              </CardTitle>
              <CardDescription>
                Manage access rules for your collections. Click "Edit Rules" to modify permissions for specific operations.
              </CardDescription>
            </div>
            {permissionsData.length > 0 && (
              <div className="text-sm text-muted-foreground">
                {permissionsData.length} collection{permissionsData.length !== 1 ? 's' : ''}
              </div>
            )}
          </div>
        </CardHeader>
        <CardContent className="p-6">
          {permissionsData.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 text-center">
              <div className="p-4 bg-muted/50 rounded-full mb-6">
                <Shield className="h-12 w-12 text-muted-foreground" />
              </div>
              <h3 className="text-xl font-semibold text-muted-foreground mb-3">No Collections Found</h3>
              <p className="text-sm text-muted-foreground max-w-md leading-relaxed mb-6">
                Create some collections first, then you can configure their access rules here. Collections define the data structure and permissions for your application.
              </p>
              <Button variant="outline" className="mt-2">
                <Shield className="w-4 h-4 mr-2" />
                Learn about Collections
              </Button>
            </div>
          ) : (
            <div className="space-y-6">
              {permissionsData.map((info) => (
                <Card key={info.collection_name} className="transition-all duration-200 hover:shadow-lg hover:border-primary/20 border-2">
                  <CardHeader className="pb-4">
                    <div className="flex flex-col space-y-4 lg:flex-row lg:items-start lg:justify-between lg:space-y-0">
                      <div className="space-y-3 flex-1">
                        <div className="flex items-center gap-3 flex-wrap">
                          <h3 className="text-xl font-bold">{info.collection_name}</h3>
                          <div className="flex gap-2">
                            <Badge 
                              variant={info.collection_type === 'auth' ? 'secondary' : 'default'} 
                              className="text-xs font-medium px-2.5 py-1"
                            >
                              {info.collection_type === 'auth' ? '🔐 Auth' : '📊 Data'}
                            </Badge>
                            {info.has_custom_rules && (
                              <Badge 
                                variant="outline" 
                                className="text-xs border-orange-300 text-orange-700 bg-orange-50 font-medium px-2.5 py-1"
                              >
                                ⚙️ Custom Rules
                              </Badge>
                            )}
                          </div>
                        </div>
                        <p className="text-sm text-muted-foreground leading-relaxed">
                          {info.has_custom_rules 
                            ? 'This collection has custom permission rules configured for enhanced security' 
                            : 'Using default permission settings for all operations - consider customizing for better security'
                          }
                        </p>
                      </div>
                      <div className="flex-shrink-0">
                        <Button 
                          size="sm" 
                          onClick={() => setEditingPermissions(info.permissions)}
                          className="w-full lg:w-auto min-w-[120px] font-medium"
                        >
                          <Shield className="w-4 h-4 mr-2" />
                          Edit Rules
                        </Button>
                      </div>
                    </div>
                  </CardHeader>
                  <CardContent className="pt-0">
                    {/* Enhanced permissions display with better visual hierarchy */}
                    <div className="space-y-6">
                      {/* CRUD Operations */}
                      <div>
                        <div className="flex items-center justify-between mb-4">
                          <h4 className="text-sm font-semibold text-foreground flex items-center gap-2">
                            <div className="w-2 h-2 bg-blue-500 rounded-full"></div>
                            CRUD Operations
                          </h4>
                          <span className="text-xs text-muted-foreground">
                            {Object.keys(info.permissions.crud_rules).length} operations
                          </span>
                        </div>
                        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5 gap-3">
                          {sortCrudOperations(info.permissions.crud_rules).map(([operation, rule]) => {
                            const display = getPermissionLevelDisplay(rule.permission);
                            return (
                              <div key={operation} className="group relative">
                                <div className="flex items-center space-x-3 p-3 rounded-lg bg-gradient-to-r from-muted/30 to-muted/50 border border-border/50 hover:border-primary/30 transition-all duration-200">
                                  <div className="flex-shrink-0 p-1.5 bg-background rounded-md shadow-sm">
                                    {getCrudOperationIcon(operation as any)}
                                  </div>
                                  <div className="min-w-0 flex-1">
                                    <div className="text-sm font-medium capitalize truncate mb-1">{operation}</div>
                                    <Badge 
                                      variant="outline" 
                                      className={`text-xs font-medium ${display.color}`}
                                    >
                                      {display.text}
                                    </Badge>
                                  </div>
                                </div>
                              </div>
                            );
                          })}
                        </div>
                      </div>

                      {/* Auth Operations - Only show for auth collections */}
                      {info.collection_type === 'auth' && info.permissions.auth_rules && (
                        <div>
                          <div className="flex items-center justify-between mb-4">
                            <h4 className="text-sm font-semibold text-foreground flex items-center gap-2">
                              <div className="w-2 h-2 bg-purple-500 rounded-full"></div>
                              Authentication Operations
                            </h4>
                            <span className="text-xs text-muted-foreground">
                              {Object.keys(info.permissions.auth_rules).length} operations
                            </span>
                          </div>
                          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
                            {sortAuthOperations(info.permissions.auth_rules).map(([operation, rule]) => {
                              const display = getPermissionLevelDisplay(rule.permission);
                              return (
                                <div key={operation} className="group relative">
                                  <div className="flex items-center space-x-3 p-3 rounded-lg bg-gradient-to-r from-purple-50 to-blue-50 border border-purple-200/50 hover:border-purple-300 transition-all duration-200">
                                    <div className="flex-shrink-0 p-1.5 bg-white rounded-md shadow-sm">
                                      {getAuthOperationIcon(operation as any)}
                                    </div>
                                    <div className="min-w-0 flex-1">
                                      <div className="text-sm font-medium capitalize truncate mb-1">{operation}</div>
                                      <Badge 
                                        variant="outline" 
                                        className={`text-xs font-medium ${display.color}`}
                                      >
                                        {display.text}
                                      </Badge>
                                    </div>
                                  </div>
                                </div>
                              );
                            })}
                          </div>
                        </div>
                      )}
                    </div>
                  </CardContent>
                </Card>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </>
  );
};
