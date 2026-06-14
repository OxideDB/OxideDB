import React from "react";
import type { LucideIcon } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/utils";

type MetricTone = "default" | "success" | "warning" | "info";

interface MetricCardProps {
  title: string;
  value: React.ReactNode;
  description?: string;
  trend?: React.ReactNode;
  icon: LucideIcon;
  tone?: MetricTone;
}

const toneClasses: Record<MetricTone, string> = {
  default: "bg-muted text-muted-foreground",
  success: "bg-success/10 text-success",
  warning: "bg-warning/15 text-warning",
  info: "bg-info/10 text-info",
};

export function MetricCard({
  title,
  value,
  description,
  trend,
  icon: Icon,
  tone = "default",
}: MetricCardProps) {
  return (
    <Card className="overflow-hidden">
      <CardHeader className="flex flex-row items-start justify-between space-y-0 pb-3">
        <CardTitle className="text-sm font-medium text-muted-foreground">
          {title}
        </CardTitle>
        <div className={cn("rounded-md p-2", toneClasses[tone])}>
          <Icon className="h-4 w-4" />
        </div>
      </CardHeader>
      <CardContent>
        <div className="text-2xl font-semibold tabular-nums">{value}</div>
        {description && (
          <p className="mt-1 text-sm text-muted-foreground">{description}</p>
        )}
        {trend && (
          <p className="mt-3 text-xs font-medium text-success">{trend}</p>
        )}
      </CardContent>
    </Card>
  );
}
