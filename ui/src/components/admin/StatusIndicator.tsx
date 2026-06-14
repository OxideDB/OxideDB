import React from "react";

import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

export type StatusTone = "success" | "warning" | "danger" | "info" | "neutral";

interface StatusIndicatorProps {
  label: React.ReactNode;
  tone?: StatusTone;
  showDot?: boolean;
  className?: string;
}

const dotClasses: Record<StatusTone, string> = {
  success: "bg-success",
  warning: "bg-warning",
  danger: "bg-destructive",
  info: "bg-info",
  neutral: "bg-muted-foreground",
};

const pillClasses: Record<StatusTone, string> = {
  success: "border-success/20 bg-success/10 text-success",
  warning: "border-warning/30 bg-warning/15 text-warning-foreground dark:text-warning",
  danger: "border-destructive/20 bg-destructive/10 text-destructive",
  info: "border-info/20 bg-info/10 text-info",
  neutral: "border-border bg-muted text-muted-foreground",
};

export function StatusDot({ tone = "neutral", className }: Pick<StatusIndicatorProps, "tone" | "className">) {
  return (
    <span
      aria-hidden="true"
      className={cn("h-2 w-2 rounded-full", dotClasses[tone], className)}
    />
  );
}

export function StatusIndicator({
  label,
  tone = "neutral",
  showDot = true,
  className,
}: StatusIndicatorProps) {
  return (
    <Badge variant="outline" className={cn("gap-1.5 font-medium", pillClasses[tone], className)}>
      {showDot && <StatusDot tone={tone} />}
      {label}
    </Badge>
  );
}
