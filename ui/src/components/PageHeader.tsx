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
    <header className="flex h-14 md:h-16 shrink-0 items-center gap-2 border-b px-4">
      <SidebarTrigger className="-ml-1" />
      <Separator orientation="vertical" className="mr-2 h-4" />
      {leftActions && (
        <>
          {leftActions}
          <Separator orientation="vertical" className="mr-2 h-4" />
        </>
      )}
      <div className="flex-1 flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold">{title}</h1>
          {description && (
            <p className="text-sm text-muted-foreground hidden sm:block">{description}</p>
          )}
        </div>
        {children && (
          <div className="flex items-center gap-2">
            {children}
          </div>
        )}
      </div>
    </header>
  );
};

export default PageHeader; 