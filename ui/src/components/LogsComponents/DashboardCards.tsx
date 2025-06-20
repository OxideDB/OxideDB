import React from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '../ui/card';
import { Badge } from '../ui/badge';
import { Progress } from '../ui/progress';
import { Tooltip, TooltipContent, TooltipTrigger } from '../ui/tooltip';
import { 
  Activity, AlertTriangle, Database, User, TrendingUp 
} from 'lucide-react';
import type { DashboardMetrics, LoggingHealthResponse } from '../../types/api';

interface DashboardCardsProps {
  dashboardMetrics: DashboardMetrics | null;
  loggingHealth: LoggingHealthResponse | null;
}

const formatBytes = (bytes: number): string => {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
};

export const DashboardCards: React.FC<DashboardCardsProps> = ({ 
  dashboardMetrics, 
  loggingHealth 
}) => {
  if (!dashboardMetrics) {
    return (
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        {[...Array(4)].map((_, i) => (
          <Card key={i} className="animate-pulse">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <div className="h-4 w-24 bg-muted rounded"></div>
              <div className="h-4 w-4 bg-muted rounded"></div>
            </CardHeader>
            <CardContent>
              <div className="h-8 w-16 bg-muted rounded mb-2"></div>
              <div className="h-3 w-20 bg-muted rounded"></div>
            </CardContent>
          </Card>
        ))}
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Main Metrics */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        <Tooltip>
          <TooltipTrigger asChild>
            <Card className="cursor-help hover:shadow-md transition-shadow">
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Total Logs</CardTitle>
                <Activity className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {dashboardMetrics.log_metrics.total_entries.toLocaleString()}
                </div>
                <p className="text-xs text-muted-foreground">
                  Avg {dashboardMetrics.log_metrics.avg_entries_per_day.toFixed(1)}/day
                </p>
              </CardContent>
            </Card>
          </TooltipTrigger>
          <TooltipContent>
            <div className="text-xs">
              <div>Total log entries in the system</div>
              <div>Average daily log volume</div>
            </div>
          </TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Card className="cursor-help hover:shadow-md transition-shadow">
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Error Rate</CardTitle>
                <AlertTriangle className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {dashboardMetrics.log_metrics.error_rate_24h.toFixed(2)}%
                </div>
                <p className="text-xs text-muted-foreground">Last 24 hours</p>
                <Progress 
                  value={Math.min(dashboardMetrics.log_metrics.error_rate_24h, 100)} 
                  className="mt-2 h-1"
                />
              </CardContent>
            </Card>
          </TooltipTrigger>
          <TooltipContent>
            <div className="text-xs">
              <div>Percentage of error-level logs in the last 24 hours</div>
              <div>Lower is better for system health</div>
            </div>
          </TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Card className="cursor-help hover:shadow-md transition-shadow">
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Storage</CardTitle>
                <Database className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {formatBytes(dashboardMetrics.log_metrics.storage_size_bytes)}
                </div>
                <div className="flex items-center justify-between text-xs text-muted-foreground mt-1">
                  <span>{dashboardMetrics.health_indicators.storage_utilization.toFixed(1)}% utilized</span>
                </div>
                <Progress 
                  value={dashboardMetrics.health_indicators.storage_utilization} 
                  className="mt-2 h-1"
                />
              </CardContent>
            </Card>
          </TooltipTrigger>
          <TooltipContent>
            <div className="text-xs">
              <div>Total storage used by log data</div>
              <div>Storage utilization percentage</div>
            </div>
          </TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Card className="cursor-help hover:shadow-md transition-shadow">
              <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">Active Users</CardTitle>
                <User className="h-4 w-4 text-muted-foreground" />
              </CardHeader>
              <CardContent>
                <div className="text-2xl font-bold">
                  {dashboardMetrics.health_indicators.active_users}
                </div>
                <p className="text-xs text-muted-foreground">Currently active</p>
              </CardContent>
            </Card>
          </TooltipTrigger>
          <TooltipContent>
            <div className="text-xs">
              <div>Number of users with recent activity</div>
              <div>Based on recent log entries</div>
            </div>
          </TooltipContent>
        </Tooltip>
      </div>

      {/* System Health Card */}
      {loggingHealth && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center space-x-2">
              <TrendingUp className="h-5 w-5" />
              <span>System Health</span>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
              <div>
                <div className="flex items-center justify-between mb-2">
                  <span className="text-sm font-medium">Status</span>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Badge 
                        variant={loggingHealth.status === 'healthy' ? 'default' : 'destructive'}
                        className="cursor-help"
                      >
                        {loggingHealth.status}
                      </Badge>
                    </TooltipTrigger>
                    <TooltipContent>
                      Overall system health status
                    </TooltipContent>
                  </Tooltip>
                </div>
              </div>

              <div>
                <div className="flex items-center justify-between mb-2">
                  <span className="text-sm font-medium">Storage Size</span>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <span className="text-lg font-semibold cursor-help">
                        {loggingHealth.storage_size_mb} MB
                      </span>
                    </TooltipTrigger>
                    <TooltipContent>
                      Current size of log database
                    </TooltipContent>
                  </Tooltip>
                </div>
              </div>

              <div>
                <div className="flex items-center justify-between mb-2">
                  <span className="text-sm font-medium">Error Rate (24h)</span>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <span className="text-lg font-semibold cursor-help">
                        {loggingHealth.error_rate_24h.toFixed(2)}%
                      </span>
                    </TooltipTrigger>
                    <TooltipContent>
                      Error rate over the last 24 hours
                    </TooltipContent>
                  </Tooltip>
                </div>
                <Progress 
                  value={Math.min(loggingHealth.error_rate_24h, 100)} 
                  className="h-2"
                />
              </div>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}; 