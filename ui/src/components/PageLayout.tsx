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
      
      <div className="flex-1 space-y-4 md:space-y-6 p-4 md:p-6">
        {children}
      </div>
    </SidebarInset>
  );
};

export default PageLayout; 