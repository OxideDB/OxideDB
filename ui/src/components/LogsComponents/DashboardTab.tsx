import React from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../ui/card';
import { Badge } from '../ui/badge';
import { Button } from '../ui/button';
import { ScrollArea } from '../ui/scroll-area';
import { Tooltip, TooltipContent, TooltipTrigger } from '../ui/tooltip';
import { Search } from 'lucide-react';
import { DashboardCards } from './DashboardCards';
import type { 
  DashboardMetrics, 
  LoggingHealthResponse, 
  LogEntry 
} from '../../types/api';

interface DashboardTabProps {
  dashboardMetrics: DashboardMetrics | null;
  loggingHealth: LoggingHealthResponse | null;
  recentLogs: LogEntry[];
  onCorrelationSearch: (correlationId: string) => void;
}

export const DashboardTab: React.FC<DashboardTabProps> = ({
  dashboardMetrics,
  loggingHealth,
  recentLogs,
  onCorrelationSearch
}) => {
  const formatDate = (dateString: string): string => {
    return new Date(dateString).toLocaleString();
  };

  return (
    <>
      <DashboardCards 
        dashboardMetrics={dashboardMetrics}
        loggingHealth={loggingHealth}
      />

      {/* Recent Activity */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mt-6">
        <Card>
          <CardHeader>
            <CardTitle>Recent Logs</CardTitle>
            <CardDescription>Latest 10 log entries</CardDescription>
          </CardHeader>
          <CardContent>
            <ScrollArea className="h-[300px]">
              <div className="space-y-2">
                {recentLogs.map((log) => (
                  <div 
                    key={log.id} 
                    className="flex items-start space-x-2 p-2 border rounded hover:bg-muted/50 transition-colors"
                  >
                    <div className="flex-1 min-w-0">
                      <Tooltip>
                        <TooltipTrigger asChild>
                          <p className="text-sm font-medium truncate cursor-help">
                            {log.message}
                          </p>
                        </TooltipTrigger>
                        <TooltipContent className="max-w-sm">
                          <div className="whitespace-pre-wrap break-words">
                            {log.message}
                          </div>
                        </TooltipContent>
                      </Tooltip>
                      <p className="text-xs text-muted-foreground">
                        {log.module} • {formatDate(log.timestamp)}
                      </p>
                      <div className="flex items-center space-x-2 mt-1">
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <code className="text-xs bg-muted px-1 py-0.5 rounded cursor-help">
                              {log.correlation_id.slice(0, 8)}...
                            </code>
                          </TooltipTrigger>
                          <TooltipContent>
                            <div className="text-xs font-mono">{log.correlation_id}</div>
                          </TooltipContent>
                        </Tooltip>
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => onCorrelationSearch(log.correlation_id)}
                              className="h-5 w-5 p-0"
                              aria-label="Search by correlation ID"
                            >
                              <Search className="h-3 w-3" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>Search by correlation ID</TooltipContent>
                        </Tooltip>
                      </div>
                    </div>
                    <Badge 
                      variant={
                        log.level === 'ERROR' ? 'destructive' : 
                        log.level === 'WARN' ? 'secondary' : 
                        'default'
                      }
                    >
                      {log.level}
                    </Badge>
                  </div>
                ))}
              </div>
            </ScrollArea>
          </CardContent>
        </Card>

        {/* Top Users */}
        {dashboardMetrics && (
          <Card>
            <CardHeader>
              <CardTitle>Top Active Users</CardTitle>
              <CardDescription>Most active users by log volume</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="space-y-2">
                {dashboardMetrics.log_metrics.top_users.slice(0, 5).map(([userId, count]) => (
                  <div 
                    key={userId} 
                    className="flex justify-between items-center p-2 rounded hover:bg-muted/50 transition-colors"
                  >
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <span className="text-sm font-medium cursor-help">{userId}</span>
                      </TooltipTrigger>
                      <TooltipContent>User ID: {userId}</TooltipContent>
                    </Tooltip>
                    <Badge variant="outline">{count} actions</Badge>
                  </div>
                ))}
              </div>
            </CardContent>
          </Card>
        )}
      </div>
    </>
  );
}; 