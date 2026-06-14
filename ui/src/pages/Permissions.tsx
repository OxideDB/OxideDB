import React from 'react';
import { Button } from '@/components/ui/button';
import { RotateCcw, Shield } from 'lucide-react';
import PageLayout from '@/components/PageLayout';
import PageTabs, { TabsContent, TabsTrigger } from '@/components/PageTabs';
import { ErrorState, LoadingState } from '@/components/admin/AdminState';
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
  } = usePermissions();

  if (loading) {
    return (
      <PageLayout title="Collection Permissions">
        <LoadingState label="Loading permissions" />
      </PageLayout>
    );
  }

  if (error) {
    return (
      <PageLayout title="Collection Permissions">
        <ErrorState
          title="Failed to load permissions"
          description={error}
          onRetry={loadPermissions}
        />
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
      <PageTabs
        defaultValue="rules"
        tabTriggers={
          <>
            <TabsTrigger value="rules">
              <Shield className="w-4 h-4 mr-2 hidden xs:inline" />
              <span className="hidden sm:inline">Access Rules</span>
              <span className="sm:hidden">Rules</span>
            </TabsTrigger>
            <TabsTrigger value="collections">
              <Shield className="w-4 h-4 mr-2 hidden xs:inline" />
              Collections
            </TabsTrigger>
            <TabsTrigger value="examples">
              <Shield className="w-4 h-4 mr-2 hidden xs:inline" />
              Examples
            </TabsTrigger>
          </>
        }
      >
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
      </PageTabs>
    </PageLayout>
  );
};

export default Permissions;
