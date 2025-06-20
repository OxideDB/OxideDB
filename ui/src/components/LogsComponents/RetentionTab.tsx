import React from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../ui/card';
import { Badge } from '../ui/badge';
import { Label } from '../ui/label';
import { Separator } from '../ui/separator';
import { Tooltip, TooltipContent, TooltipTrigger } from '../ui/tooltip';
import { Database, Settings, Clock } from 'lucide-react';
import type { RetentionStats } from '../../types/api';

interface RetentionTabProps {
  retentionStats: RetentionStats | null;
  loading: boolean;
}

export const RetentionTab: React.FC<RetentionTabProps> = ({
  retentionStats,
  loading
}) => {
  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const formatDate = (dateString: string): string => {
    return new Date(dateString).toLocaleString();
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center space-x-2">
          <Database className="h-5 w-5" />
          <span>Log Retention</span>
        </CardTitle>
        <CardDescription>
          Manage log retention policies and storage
        </CardDescription>
      </CardHeader>
      <CardContent>
        {loading && (
          <p className="text-center text-muted-foreground">Loading retention data...</p>
        )}
        
        {retentionStats ? (
          <div className="space-y-6">
            {/* Overview Cards */}
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              <Card className="p-4">
                <Tooltip>
                  <TooltipTrigger asChild>
                    <div className="cursor-help">
                      <Label className="text-sm font-medium">Total Log Entries</Label>
                      <p className="text-2xl font-bold mt-1">
                        {retentionStats.total_log_entries.toLocaleString()}
                      </p>
                    </div>
                  </TooltipTrigger>
                  <TooltipContent>
                    Total number of log entries in the system
                  </TooltipContent>
                </Tooltip>
              </Card>
              
              <Card className="p-4">
                <Tooltip>
                  <TooltipTrigger asChild>
                    <div className="cursor-help">
                      <Label className="text-sm font-medium">Storage Size</Label>
                      <p className="text-2xl font-bold mt-1">
                        {formatBytes(retentionStats.storage_size_bytes)}
                      </p>
                    </div>
                  </TooltipTrigger>
                  <TooltipContent>
                    Total disk space used by log storage
                  </TooltipContent>
                </Tooltip>
              </Card>
              
              <Card className="p-4">
                <Tooltip>
                  <TooltipTrigger asChild>
                    <div className="cursor-help">
                      <Label className="text-sm font-medium">Cleanup Candidates</Label>
                      <p className="text-2xl font-bold mt-1">
                        {retentionStats.estimated_cleanup_candidates.toLocaleString()}
                      </p>
                    </div>
                  </TooltipTrigger>
                  <TooltipContent>
                    Number of logs eligible for cleanup based on retention policy
                  </TooltipContent>
                </Tooltip>
              </Card>
            </div>

            <Separator />

            {/* Retention Policy */}
            <div>
              <h3 className="text-lg font-semibold mb-4 flex items-center space-x-2">
                <Settings className="h-5 w-5" />
                <span>Retention Policy</span>
              </h3>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="p-4 border rounded-lg">
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <div className="cursor-help">
                        <Label className="text-sm font-medium">Standard Logs</Label>
                        <p className="text-lg font-semibold mt-1">
                          {retentionStats.policy.standard_retention_days} days
                        </p>
                      </div>
                    </TooltipTrigger>
                    <TooltipContent>
                      How long standard log entries are kept before cleanup
                    </TooltipContent>
                  </Tooltip>
                </div>
                
                <div className="p-4 border rounded-lg">
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <div className="cursor-help">
                        <Label className="text-sm font-medium">Audit Logs</Label>
                        <p className="text-lg font-semibold mt-1">
                          {retentionStats.policy.audit_retention_days} days
                        </p>
                      </div>
                    </TooltipTrigger>
                    <TooltipContent>
                      How long security audit logs are kept (typically longer for compliance)
                    </TooltipContent>
                  </Tooltip>
                </div>
                
                <div className="p-4 border rounded-lg">
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <div className="cursor-help">
                        <Label className="text-sm font-medium">Error Logs</Label>
                        <p className="text-lg font-semibold mt-1">
                          {retentionStats.policy.error_retention_days} days
                        </p>
                      </div>
                    </TooltipTrigger>
                    <TooltipContent>
                      How long error-level logs are kept for debugging purposes
                    </TooltipContent>
                  </Tooltip>
                </div>
                
                <div className="p-4 border rounded-lg">
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <div className="cursor-help">
                        <Label className="text-sm font-medium">Compression</Label>
                        <Badge 
                          variant={retentionStats.policy.enable_compression ? 'default' : 'secondary'}
                          className="mt-1"
                        >
                          {retentionStats.policy.enable_compression ? 'Enabled' : 'Disabled'}
                        </Badge>
                      </div>
                    </TooltipTrigger>
                    <TooltipContent>
                      Whether old logs are compressed to save storage space
                    </TooltipContent>
                  </Tooltip>
                </div>
              </div>
            </div>

            {/* Last Cleanup Info */}
            {retentionStats.last_cleanup_time && (
              <div className="p-4 border rounded-lg">
                <Tooltip>
                  <TooltipTrigger asChild>
                    <div className="cursor-help">
                      <Label className="text-sm font-medium flex items-center space-x-2">
                        <Clock className="h-4 w-4" />
                        <span>Last Cleanup</span>
                      </Label>
                      <p className="text-lg font-semibold mt-1">
                        {formatDate(retentionStats.last_cleanup_time)}
                      </p>
                    </div>
                  </TooltipTrigger>
                  <TooltipContent>
                    When the retention cleanup process last ran
                  </TooltipContent>
                </Tooltip>
              </div>
            )}
          </div>
        ) : !loading ? (
          <p className="text-center text-muted-foreground">
            No retention information available
          </p>
        ) : null}
      </CardContent>
    </Card>
  );
}; 