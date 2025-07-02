import React from 'react';
import { Button } from '@/components/ui/button';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { RotateCcw, Shield } from 'lucide-react';
import PageLayout from '@/components/PageLayout';
import { usePermissions } from '@/hooks/permissions/usePermissions';
import { 
  AccessRulesTab, 
  CollectionRulesTab, 
  RuleExamplesTab 
} from '@/components/permissions';

const Permissions: React.FC = () => {
  const {
    permissionsData,
    loading,
    error,
    loadPermissions,
    updateCollectionPermissions,
    resetPermissions,
    applyPreset,
  } = usePermissions();

  if (loading) {
    return (
      <PageLayout title="Collection Permissions">
        <div className="space-y-8">
          {/* Enhanced loading state */}
          <div className="flex items-center justify-center py-16">
            <div className="text-center space-y-4">
              <div className="w-16 h-16 bg-primary/10 rounded-full flex items-center justify-center mx-auto">
                <Shield className="h-8 w-8 text-primary animate-pulse" />
              </div>
              <div className="space-y-2">
                <h3 className="text-lg font-semibold">Loading Permissions</h3>
                <p className="text-muted-foreground">Fetching your collection access rules...</p>
              </div>
            </div>
          </div>
          
          {/* Enhanced skeleton cards */}
          <div className="grid gap-6">
            {[1, 2, 3].map((i) => (
              <Card key={i} className="border-2">
                <CardHeader className="animate-pulse">
                  <div className="flex items-center justify-between">
                    <div className="space-y-2">
                      <div className="h-6 bg-muted rounded-lg w-48"></div>
                      <div className="h-4 bg-muted/60 rounded w-64"></div>
                    </div>
                    <div className="h-10 bg-muted rounded-lg w-24"></div>
                  </div>
                </CardHeader>
                <CardContent className="animate-pulse">
                  <div className="space-y-4">
                    <div className="grid grid-cols-2 lg:grid-cols-5 gap-3">
                      {[1, 2, 3, 4, 5].map((j) => (
                        <div key={j} className="h-16 bg-muted/60 rounded-lg"></div>
                      ))}
                    </div>
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        </div>
      </PageLayout>
    );
  }

  if (error) {
    return (
      <PageLayout title="Collection Permissions">
        <div className="space-y-8">
          {/* Enhanced error state */}
          <div className="flex items-center justify-center py-16">
            <div className="text-center space-y-6 max-w-md">
              <div className="w-16 h-16 bg-red-100 rounded-full flex items-center justify-center mx-auto">
                <Shield className="h-8 w-8 text-red-600" />
              </div>
              <div className="space-y-2">
                <h3 className="text-xl font-semibold text-red-900">Failed to Load Permissions</h3>
                <p className="text-red-700 leading-relaxed">
                  We couldn't fetch your collection permissions. This might be due to a network issue or server problem.
                </p>
              </div>
              <Alert variant="destructive" className="text-left">
                <AlertDescription className="font-mono text-sm">{error}</AlertDescription>
              </Alert>
              <div className="flex flex-col sm:flex-row gap-3 justify-center">
                <Button onClick={loadPermissions} className="min-w-[120px]">
                  <RotateCcw className="w-4 h-4 mr-2" />
                  Try Again
                </Button>
                <Button variant="outline" className="min-w-[120px]">
                  <Shield className="w-4 h-4 mr-2" />
                  Help & Support
                </Button>
              </div>
            </div>
          </div>
        </div>
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
        {/* Tabs navigation - integrated with PageLayout spacing */}
        <div className="-mt-3 sm:-mt-4 lg:-mt-6 -mx-3 sm:-mx-4 lg:-mx-6 px-3 sm:px-4 lg:px-6 border-b bg-background/50 backdrop-blur-sm">
          <TabsList className="h-12 p-0 bg-transparent w-full justify-start rounded-none border-0">
            <TabsTrigger 
              value="rules" 
              className="h-12 px-4 sm:px-6 rounded-none border-b-2 border-transparent data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-primary data-[state=active]:shadow-none bg-transparent transition-all duration-200 text-sm font-medium hover:text-primary/80"
            >
              <Shield className="w-4 h-4 mr-2 hidden xs:inline" />
              <span className="hidden sm:inline">Access Rules</span>
              <span className="sm:hidden">Rules</span>
            </TabsTrigger>
            <TabsTrigger 
              value="collections" 
              className="h-12 px-4 sm:px-6 rounded-none border-b-2 border-transparent data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-primary data-[state=active]:shadow-none bg-transparent transition-all duration-200 text-sm font-medium hover:text-primary/80"
            >
              <Shield className="w-4 h-4 mr-2 hidden xs:inline" />
              Collections
            </TabsTrigger>
            <TabsTrigger 
              value="examples" 
              className="h-12 px-4 sm:px-6 rounded-none border-b-2 border-transparent data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-primary data-[state=active]:shadow-none bg-transparent transition-all duration-200 text-sm font-medium hover:text-primary/80"
            >
              <Shield className="w-4 h-4 mr-2 hidden xs:inline" />
              Examples
            </TabsTrigger>
          </TabsList>
        </div>

        {/* Tab content with proper spacing that matches PageLayout */}
        <div className="space-y-3 sm:space-y-4 lg:space-y-6 pt-3 sm:pt-4 lg:pt-6">
          <TabsContent value="rules" className="space-y-3 sm:space-y-4 lg:space-y-6 mt-0 focus-visible:outline-none">
            <AccessRulesTab 
              permissionsData={permissionsData}
              onUpdatePermissions={updateCollectionPermissions}
            />
          </TabsContent>

          <TabsContent value="collections" className="space-y-3 sm:space-y-4 lg:space-y-6 mt-0 focus-visible:outline-none">
            <CollectionRulesTab permissionsData={permissionsData} />
          </TabsContent>

          <TabsContent value="examples" className="space-y-3 sm:space-y-4 lg:space-y-6 mt-0 focus-visible:outline-none">
            <RuleExamplesTab />
          </TabsContent>
        </div>
      </Tabs>
    </PageLayout>
  );
};

export default Permissions;
