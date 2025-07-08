import React from "react";
import { Tabs, TabsList, TabsTrigger as PrimitiveTabsTrigger, TabsContent as PrimitiveTabsContent } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";

interface PageTabsProps {
  /**
   * Current active tab value (controlled mode).
   */
  value?: string;
  /**
   * Default active tab (uncontrolled mode).
   */
  defaultValue?: string;
  /**
   * Callback fired when the active tab changes.
   */
  onValueChange?: (value: string) => void;
  /**
   * The collection of `TabsTrigger` nodes used to render the navigation bar.
   */
  tabTriggers: React.ReactNode;
  /**
   * Tab content elements (`TabsContent`) that should be displayed.
   */
  children: React.ReactNode;
  /**
   * Extra classes for the underlying `Tabs` root.
   */
  className?: string;
  /**
   * Extra classes for the content wrapper (applied to the `<div>` that wraps all `TabsContent`).
   */
  contentClassName?: string;
}

/**
 * PageTabs is a convenience wrapper around Radix‐UI Tabs that matches the
 * layout and styling used on the Permissions page. It renders a full-width
 * navigation bar that blends into the surrounding `PageLayout` spacing and a
 * content container with matching padding.
 */
const PageTabs: React.FC<PageTabsProps> = ({
  value,
  defaultValue,
  onValueChange,
  tabTriggers,
  children,
  className,
  contentClassName,
}) => {
  return (
    <Tabs
      value={value}
      defaultValue={defaultValue}
      onValueChange={onValueChange}
      className={cn("w-full", className)}
    >
      {/* Navigation bar */}
      <div className="-mt-3 sm:-mt-4 lg:-mt-6 -mx-3 sm:-mx-4 lg:-mx-6 px-3 sm:px-4 lg:px-6 border-b bg-background/50 backdrop-blur-sm">
        <TabsList className="h-12 p-0 bg-transparent w-full justify-start rounded-none border-0">
          {tabTriggers}
        </TabsList>
      </div>

      {/* Content wrapper */}
      <div className={cn("space-y-3 sm:space-y-4 lg:space-y-6 pt-3 sm:pt-4 lg:pt-6", contentClassName)}>
        {children}
      </div>
    </Tabs>
  );
};

// Re-export Radix primitives so that consumers can import them from one place.
export const TabsTrigger = PrimitiveTabsTrigger;
export const TabsContent = PrimitiveTabsContent;
export default PageTabs; 