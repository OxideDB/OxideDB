import React from "react";
import { SidebarTrigger } from "@/components/ui/sidebar";
import { Separator } from "@/components/ui/separator";

interface PageHeaderProps {
  title: string;
  description?: string;
  children?: React.ReactNode;
  leftActions?: React.ReactNode;
}

const PageHeader: React.FC<PageHeaderProps> = ({ title, description, children, leftActions }) => {
  return (
    <header className="flex flex-col gap-3 border-b px-3 py-3 sm:px-4 sm:py-4 lg:flex-row lg:h-16 lg:py-0 lg:gap-2">
      {/* Top row with sidebar trigger and left actions */}
      <div className="flex items-center gap-2 lg:gap-3">
        <SidebarTrigger className="-ml-1 h-8 w-8 sm:h-6 sm:w-6" />
        <Separator orientation="vertical" className="mr-1 h-4 sm:mr-2" />
        {leftActions && (
          <>
            <div className="flex-shrink-0">
              {leftActions}
            </div>
            <Separator orientation="vertical" className="mr-1 h-4 sm:mr-2 lg:block" />
          </>
        )}
      </div>
      
      {/* Content area */}
      <div className="flex-1 min-w-0 flex flex-col gap-3 sm:gap-2 lg:flex-row lg:items-center lg:justify-between">
        <div className="min-w-0 flex-1">
          <h1 className="text-lg font-semibold leading-tight sm:text-xl lg:text-lg truncate" title={title}>
            {title}
          </h1>
          {description && (
            <p className="text-sm text-muted-foreground mt-1 sm:mt-0.5 truncate" title={description}>
              {description}
            </p>
          )}
        </div>
        
        {/* Actions area */}
        {children && (
          <div className="flex items-center gap-2 flex-shrink-0 lg:ml-4">
            {children}
          </div>
        )}
      </div>
    </header>
  );
};

export default PageHeader; 