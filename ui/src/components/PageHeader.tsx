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
    <header className="sticky top-0 z-20 flex min-h-16 flex-col gap-3 border-b bg-background/95 px-3 py-3 backdrop-blur supports-[backdrop-filter]:bg-background/80 sm:px-4 lg:flex-row lg:items-center lg:gap-3 lg:px-6">
      <div className="flex items-center gap-2 lg:gap-3">
        <SidebarTrigger className="-ml-1 h-9 w-9" />
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
      
      <div className="flex-1 min-w-0 flex flex-col gap-3 sm:gap-2 lg:flex-row lg:items-center lg:justify-between">
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-lg font-semibold leading-tight text-foreground" title={title}>
            {title}
          </h1>
          {description && (
            <p className="mt-1 truncate text-sm text-muted-foreground" title={description}>
              {description}
            </p>
          )}
        </div>
        
        {children && (
          <div className="flex w-full flex-wrap items-center gap-2 lg:ml-4 lg:w-auto lg:flex-nowrap lg:justify-end">
            {children}
          </div>
        )}
      </div>
    </header>
  );
};

export default PageHeader; 
