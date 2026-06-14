import React from "react";
import { AlertTriangle, Loader2, type LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

type AdminStateTone = "default" | "danger" | "success" | "info";

interface AdminStateProps {
  title: string;
  description?: string;
  icon?: LucideIcon;
  action?: React.ReactNode;
  tone?: AdminStateTone;
  className?: string;
}

const toneClasses: Record<AdminStateTone, string> = {
  default: "bg-muted text-muted-foreground",
  danger: "bg-destructive/10 text-destructive",
  success: "bg-success/10 text-success",
  info: "bg-info/10 text-info",
};

export function AdminState({
  title,
  description,
  icon: Icon,
  action,
  tone = "default",
  className,
}: AdminStateProps) {
  return (
    <div
      className={cn(
        "flex min-h-[280px] items-center justify-center rounded-lg border border-dashed bg-card/60 px-4 py-10",
        className
      )}
    >
      <div className="mx-auto flex max-w-md flex-col items-center text-center">
        {Icon && (
          <div className={cn("mb-4 rounded-lg p-3", toneClasses[tone])}>
            <Icon className="h-6 w-6" />
          </div>
        )}
        <h3 className="text-base font-semibold text-foreground">{title}</h3>
        {description && (
          <p className="mt-2 text-sm leading-6 text-muted-foreground">{description}</p>
        )}
        {action && <div className="mt-5">{action}</div>}
      </div>
    </div>
  );
}

interface LoadingStateProps {
  label?: string;
  className?: string;
}

export function LoadingState({ label = "Loading...", className }: LoadingStateProps) {
  return (
    <AdminState
      title={label}
      description="Fetching the latest admin data."
      icon={Loader2}
      className={cn("[&_svg]:animate-spin", className)}
    />
  );
}

interface ErrorStateProps {
  title?: string;
  description: string;
  onRetry?: () => void;
  className?: string;
}

export function ErrorState({
  title = "Something went wrong",
  description,
  onRetry,
  className,
}: ErrorStateProps) {
  return (
    <AdminState
      title={title}
      description={description}
      icon={AlertTriangle}
      tone="danger"
      className={className}
      action={
        onRetry ? (
          <Button type="button" variant="outline" onClick={onRetry}>
            Try again
          </Button>
        ) : undefined
      }
    />
  );
}
