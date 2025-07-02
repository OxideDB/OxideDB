import React from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Database, Shield, Users } from 'lucide-react';
import type { CollectionPermissionsInfo } from '@/types/api';
import { 
  getPermissionLevelDisplay, 
  sortCrudOperations, 
  sortAuthOperations,
  getCrudOperationIcon,
  getAuthOperationIcon
} from '@/utils/permissions/permissionUtils';

interface CollectionRulesTabProps {
  permissionsData: CollectionPermissionsInfo[];
}

export const CollectionRulesTab: React.FC<CollectionRulesTabProps> = ({
  permissionsData
}) => {
  return (
    <div className="space-y-6">
      {/* Simplified intro section */}
      <div className="flex items-center justify-between">
        <div className="space-y-1">
          <p className="text-muted-foreground leading-relaxed">
            Quick overview of permission rules applied to each collection. Get a high-level view of your access controls.
          </p>
        </div>
        <div className="text-sm text-muted-foreground">
          {permissionsData.length} collection{permissionsData.length !== 1 ? 's' : ''}
        </div>
      </div>

      {/* Main Content */}
      <Card className="border-2 shadow-sm">
        <CardHeader className="pb-6 border-b border-border/50">
          <CardTitle className="flex items-center gap-2 text-lg">
            <Shield className="h-4 w-4 text-primary" />
            Permission Rules Summary
          </CardTitle>
          <CardDescription>
            View the permission levels applied to each collection and their operations at a glance.
          </CardDescription>
        </CardHeader>
        <CardContent className="p-6">
          {permissionsData.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 text-center">
              <div className="p-4 bg-muted/50 rounded-full mb-6">
                <Database className="h-12 w-12 text-muted-foreground" />
              </div>
              <h3 className="text-xl font-semibold text-muted-foreground mb-3">No Collections Found</h3>
              <p className="text-sm text-muted-foreground max-w-md leading-relaxed">
                Create some collections first to see their permission rules here. Once you have collections, you'll be able to view and manage their access controls.
              </p>
            </div>
          ) : (
            <div className="space-y-6">
              {permissionsData.map((info) => (
                <Card key={info.collection_name} className="border-2 transition-all duration-200 hover:shadow-md hover:border-primary/20">
                  <CardContent className="p-6">
                    <div className="space-y-5">
                      {/* Collection Header */}
                      <div className="flex flex-col lg:flex-row lg:items-start lg:justify-between gap-4">
                        <div className="space-y-2 flex-1">
                          <div className="flex items-center gap-3 flex-wrap">
                            <h4 className="font-bold text-xl">{info.collection_name}</h4>
                            <div className="flex gap-2">
                              <Badge 
                                variant={info.collection_type === 'auth' ? 'secondary' : 'default'} 
                                className="font-medium px-2.5 py-1"
                              >
                                {info.collection_type === 'auth' ? (
                                  <><Users className="w-3 h-3 mr-1" /> Auth Collection</>
                                ) : (
                                  <><Database className="w-3 h-3 mr-1" /> Data Collection</>
                                )}
                              </Badge>
                              {info.has_custom_rules && (
                                <Badge variant="outline" className="border-orange-300 text-orange-700 bg-orange-50 font-medium px-2.5 py-1">
                                  <Shield className="w-3 h-3 mr-1" />
                                  Custom Rules
                                </Badge>
                              )}
                            </div>
                          </div>
                          <p className="text-sm text-muted-foreground leading-relaxed">
                            {info.has_custom_rules 
                              ? 'This collection uses custom permission rules for enhanced security control' 
                              : 'This collection uses the default permission settings for all operations'
                            }
                          </p>
                        </div>
                      </div>

                      {/* Operations Overview */}
                      <div className="space-y-5">
                        {/* CRUD Operations */}
                        <div className="space-y-3">
                          <h5 className="text-sm font-semibold text-foreground flex items-center gap-2">
                            <div className="w-2 h-2 bg-blue-500 rounded-full"></div>
                            CRUD Operations
                            <span className="text-xs text-muted-foreground font-normal">
                              ({Object.keys(info.permissions.crud_rules).length} operations)
                            </span>
                          </h5>
                          <div className="flex flex-wrap gap-3">
                            {sortCrudOperations(info.permissions.crud_rules).map(([operation, rule]) => {
                              const display = getPermissionLevelDisplay(rule.permission);
                              return (
                                <div key={operation} className="flex items-center gap-2 px-3 py-2 rounded-lg border-2 bg-gradient-to-r from-blue-50/50 to-blue-50/70 hover:border-blue-300 transition-colors">
                                  <div className="flex-shrink-0">
                                    {getCrudOperationIcon(operation as any)}
                                  </div>
                                  <div className="flex items-center gap-2">
                                    <span className="text-sm font-medium capitalize">{operation}</span>
                                    <Badge 
                                      variant="outline" 
                                      className={`text-xs ${display.color} font-medium`}
                                    >
                                      {display.text}
                                    </Badge>
                                  </div>
                                </div>
                              );
                            })}
                          </div>
                        </div>
                        
                        {/* Auth Operations - Only show for auth collections */}
                        {info.collection_type === 'auth' && info.permissions.auth_rules && (
                          <div className="space-y-3">
                            <h5 className="text-sm font-semibold text-foreground flex items-center gap-2">
                              <div className="w-2 h-2 bg-purple-500 rounded-full"></div>
                              Authentication Operations
                              <span className="text-xs text-muted-foreground font-normal">
                                ({Object.keys(info.permissions.auth_rules).length} operations)
                              </span>
                            </h5>
                            <div className="flex flex-wrap gap-3">
                              {sortAuthOperations(info.permissions.auth_rules).map(([operation, rule]) => {
                                const display = getPermissionLevelDisplay(rule.permission);
                                return (
                                  <div key={operation} className="flex items-center gap-2 px-3 py-2 rounded-lg border-2 bg-gradient-to-r from-purple-50/50 to-purple-50/70 hover:border-purple-300 transition-colors">
                                    <div className="flex-shrink-0">
                                      {getAuthOperationIcon(operation as any)}
                                    </div>
                                    <div className="flex items-center gap-2">
                                      <span className="text-sm font-medium capitalize">{operation.replace('_', ' ')}</span>
                                      <Badge 
                                        variant="outline" 
                                        className={`text-xs ${display.color} font-medium`}
                                      >
                                        {display.text}
                                      </Badge>
                                    </div>
                                  </div>
                                );
                              })}
                            </div>
                          </div>
                        )}
                      </div>
                    </div>
                  </CardContent>
                </Card>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
};
