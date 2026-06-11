import React from "react";
import { Button} from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { MoreVertical, Check } from "lucide-react";

export interface ActionItem {
  id: string;
  label: string;
  icon?: React.ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  variant?: "default" | "destructive";
  type?: "button" | "separator";
  active?: boolean;
}

interface ActionDropdownProps {
  actions: ActionItem[];
  showDropdownOn?: "mobile" | "always" | "never";
  maxVisibleActions?: number;
  className?: string;
}

const ActionDropdown: React.FC<ActionDropdownProps> = ({
  actions,
  showDropdownOn = "mobile",
  maxVisibleActions = 2,
  className = "",
}) => {
  const buttonActions = actions.filter(action => action.type !== "separator");
  
  if (actions.length === 0) {
    return null;
  }

  // On mobile (< 640px), always show dropdown for 2+ actions
  // On tablet (640px - 1024px), show some buttons + dropdown if needed
  // On desktop (>= 1024px), show all as buttons unless "always" dropdown
  
  if (showDropdownOn === "never") {
    return (
      <div className={`flex flex-wrap items-center gap-1.5 sm:gap-2 ${className}`}>
        {buttonActions.map((action) => (
          <Button
            key={action.id}
            variant={action.active ? "default" : "outline"}
            size="sm"
            onClick={action.onClick}
            disabled={action.disabled}
            className="h-9 px-2.5 sm:h-8 sm:px-3 flex items-center gap-1.5 sm:gap-2 text-sm"
          >
            {action.icon && <span className="flex-shrink-0">{action.icon}</span>}
            <span className="hidden xs:inline sm:inline">{action.label}</span>
          </Button>
        ))}
      </div>
    );
  }

  if (showDropdownOn === "always") {
    return (
      <div className={`flex items-center ${className}`}>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="outline"
              size="sm"
              className="h-9 px-2.5 sm:h-8 sm:px-3 flex items-center gap-1.5"
            >
              <MoreVertical className="h-4 w-4" />
              <span className="sr-only">Actions</span>
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-48">
            {actions.map((action) => {
              if (action.type === "separator") {
                return <DropdownMenuSeparator key={action.id} />;
              }

              return (
                <DropdownMenuItem
                  key={action.id}
                  onClick={action.onClick}
                  disabled={action.disabled}
                  className={`flex items-center gap-2 min-h-[44px] sm:min-h-[36px] ${
                    action.variant === "destructive" ? "text-destructive focus:text-destructive" : ""
                  } ${action.active ? "bg-accent" : ""}`}
                >
                  {action.icon && <span className="flex-shrink-0">{action.icon}</span>}
                  <span className="flex-1">{action.label}</span>
                  {action.active && <Check className="h-4 w-4 flex-shrink-0" />}
                </DropdownMenuItem>
              );
            })}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    );
  }

  // "mobile" mode - responsive behavior
  const shouldShowDropdown = buttonActions.length > 1; // Always dropdown on mobile for 2+ actions
  const visibleButtonActions = shouldShowDropdown ? buttonActions.slice(0, Math.min(maxVisibleActions, 1)) : buttonActions;

  return (
    <div className={`flex items-center gap-1.5 sm:gap-2 ${className}`}>
      {/* Desktop: Show all buttons */}
      <div className="hidden lg:flex items-center gap-2">
        {buttonActions.map((action) => (
          <Button
            key={action.id}
            variant={action.active ? "default" : "outline"}
            size="sm"
            onClick={action.onClick}
            disabled={action.disabled}
            className="h-8 px-3 flex items-center gap-2 text-sm"
          >
            {action.icon && <span className="flex-shrink-0">{action.icon}</span>}
            {action.label}
          </Button>
        ))}
      </div>

      {/* Tablet: Show some buttons + dropdown if needed */}
      <div className="hidden sm:flex lg:hidden items-center gap-2">
        {visibleButtonActions.map((action) => (
          <Button
            key={action.id}
            variant={action.active ? "default" : "outline"}
            size="sm"
            onClick={action.onClick}
            disabled={action.disabled}
            className="h-8 px-3 flex items-center gap-2 text-sm"
          >
            {action.icon && <span className="flex-shrink-0">{action.icon}</span>}
            {action.label}
          </Button>
        ))}
        
        {shouldShowDropdown && buttonActions.length > maxVisibleActions && (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                className="h-8 px-3 flex items-center gap-1.5"
              >
                <MoreVertical className="h-4 w-4" />
                <span className="sr-only">More actions</span>
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-48">
              {buttonActions.slice(maxVisibleActions).map((action) => (
                <DropdownMenuItem
                  key={action.id}
                  onClick={action.onClick}
                  disabled={action.disabled}
                  className={`flex items-center gap-2 min-h-[36px] ${
                    action.variant === "destructive" ? "text-destructive focus:text-destructive" : ""
                  } ${action.active ? "bg-accent" : ""}`}
                >
                  {action.icon && <span className="flex-shrink-0">{action.icon}</span>}
                  <span className="flex-1">{action.label}</span>
                  {action.active && <Check className="h-4 w-4 flex-shrink-0" />}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        )}
      </div>

      {/* Mobile: Always use dropdown for 2+ actions, single button for 1 action */}
      <div className="flex sm:hidden items-center">
        {buttonActions.length === 1 ? (
          <Button
            variant={buttonActions[0].active ? "default" : "outline"}
            size="sm"
            onClick={buttonActions[0].onClick}
            disabled={buttonActions[0].disabled}
            className="h-9 px-2.5 flex items-center gap-1.5 text-sm"
          >
            {buttonActions[0].icon && <span className="flex-shrink-0">{buttonActions[0].icon}</span>}
            <span>{buttonActions[0].label}</span>
          </Button>
        ) : (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                className="h-9 px-2.5 flex items-center gap-1.5"
              >
                <MoreVertical className="h-4 w-4" />
                <span className="sr-only">Actions</span>
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-48">
              {actions.map((action) => {
                if (action.type === "separator") {
                  return <DropdownMenuSeparator key={action.id} />;
                }

                return (
                  <DropdownMenuItem
                    key={action.id}
                    onClick={action.onClick}
                    disabled={action.disabled}
                    className={`flex items-center gap-2 min-h-[44px] ${
                      action.variant === "destructive" ? "text-destructive focus:text-destructive" : ""
                    } ${action.active ? "bg-accent" : ""}`}
                  >
                    {action.icon && <span className="flex-shrink-0">{action.icon}</span>}
                    <span className="flex-1">{action.label}</span>
                    {action.active && <Check className="h-4 w-4 flex-shrink-0" />}
                  </DropdownMenuItem>
                );
              })}
            </DropdownMenuContent>
          </DropdownMenu>
        )}
      </div>
    </div>
  );
};

export default ActionDropdown;
