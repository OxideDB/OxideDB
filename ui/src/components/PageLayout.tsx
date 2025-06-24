import React from "react";
import { SidebarInset } from "@/components/ui/sidebar";
import PageHeader from "./PageHeader";

interface PageLayoutProps {
  title: string;
  description?: string;
  headerActions?: React.ReactNode;
  leftActions?: React.ReactNode;
  children: React.ReactNode;
}

const PageLayout: React.FC<PageLayoutProps> = ({ 
  title, 
  description, 
  headerActions, 
  leftActions,
  children 
}) => {
  return (
    <SidebarInset>
      <PageHeader title={title} description={description} leftActions={leftActions}>
        {headerActions}
      </PageHeader>
      
      <div className="flex-1 space-y-3 p-3 sm:space-y-4 sm:p-4 lg:space-y-6 lg:p-6 max-w-full overflow-x-hidden">
        {children}
      </div>
    </SidebarInset>
  );
};

export default PageLayout; 