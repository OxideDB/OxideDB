import React from 'react';
import { TableCell, TableRow } from '../ui/table';
import { Badge, type BadgeProps } from '../ui/badge';
import { Button } from '../ui/button';
import { Tooltip, TooltipContent, TooltipTrigger } from '../ui/tooltip';
import { 
  AlertTriangle, Info, Bug, Eye, Copy, Search 
} from 'lucide-react';
import type { LogEntry, LogLevel } from '../../types/api';

interface LogEntryRowProps {
  log: LogEntry;
  onCorrelationSearch: (correlationId: string) => void;
}

const getLogLevelColor = (level: LogLevel): string => {
  switch (level) {
    case 'ERROR': return 'destructive';
    case 'WARN': return 'secondary';
    case 'INFO': return 'default';
    case 'DEBUG': return 'outline';
    case 'TRACE': return 'outline';
    default: return 'default';
  }
};

const getLogLevelIcon = (level: LogLevel) => {
  switch (level) {
    case 'ERROR': return <AlertTriangle className="h-4 w-4" />;
    case 'WARN': return <AlertTriangle className="h-4 w-4" />;
    case 'INFO': return <Info className="h-4 w-4" />;
    case 'DEBUG': return <Bug className="h-4 w-4" />;
    case 'TRACE': return <Eye className="h-4 w-4" />;
    default: return <Info className="h-4 w-4" />;
  }
};

const formatDate = (dateString: string): string => {
  return new Date(dateString).toLocaleString();
};

export const LogEntryRow: React.FC<LogEntryRowProps> = ({ log, onCorrelationSearch }) => {
  const handleCopyCorrelationId = async () => {
    try {
      await navigator.clipboard.writeText(log.correlation_id);
    } catch (error) {
      console.error('Failed to copy correlation ID:', error);
    }
  };

  return (
    <TableRow>
      <TableCell>
        <div className="flex items-center space-x-2">
          {getLogLevelIcon(log.level)}
          <Badge 
            variant={getLogLevelColor(log.level as LogLevel) as BadgeProps['variant']}
            className="whitespace-nowrap"
          >
            {log.level}
          </Badge>
        </div>
      </TableCell>
      
      <TableCell className="text-sm font-mono">
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="cursor-help">
              {formatDate(log.timestamp)}
            </span>
          </TooltipTrigger>
          <TooltipContent>
            <div className="text-xs">
              <div>Local time: {new Date(log.timestamp).toLocaleString()}</div>
              <div>UTC: {new Date(log.timestamp).toISOString()}</div>
            </div>
          </TooltipContent>
        </Tooltip>
      </TableCell>
      
      <TableCell className="font-medium">
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="cursor-help truncate block max-w-[120px]">
              {log.module}
            </span>
          </TooltipTrigger>
          <TooltipContent>{log.module}</TooltipContent>
        </Tooltip>
      </TableCell>
      
      <TableCell className="max-w-md">
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="cursor-help truncate block">
              {log.message}
            </span>
          </TooltipTrigger>
          <TooltipContent className="max-w-sm">
            <div className="whitespace-pre-wrap break-words">
              {log.message}
            </div>
          </TooltipContent>
        </Tooltip>
      </TableCell>
      
      <TableCell>
        {log.context.user_id ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <Badge variant="outline" className="cursor-help">
                {log.context.user_id}
              </Badge>
            </TooltipTrigger>
            <TooltipContent>User ID: {log.context.user_id}</TooltipContent>
          </Tooltip>
        ) : (
          <span className="text-muted-foreground">-</span>
        )}
      </TableCell>
      
      <TableCell>
        {log.context.collection ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <Badge variant="outline" className="cursor-help">
                {log.context.collection}
              </Badge>
            </TooltipTrigger>
            <TooltipContent>Collection: {log.context.collection}</TooltipContent>
          </Tooltip>
        ) : (
          <span className="text-muted-foreground">-</span>
        )}
      </TableCell>
      
      <TableCell>
        <div className="flex items-center space-x-1">
          <Tooltip>
            <TooltipTrigger asChild>
              <code className="text-xs bg-muted px-2 py-1 rounded max-w-[100px] truncate cursor-help">
                {log.correlation_id.slice(0, 8)}...
              </code>
            </TooltipTrigger>
            <TooltipContent>
              <div className="text-xs font-mono">
                {log.correlation_id}
              </div>
            </TooltipContent>
          </Tooltip>
          
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                onClick={handleCopyCorrelationId}
                className="h-6 w-6 p-0"
                aria-label="Copy correlation ID to clipboard"
              >
                <Copy className="h-3 w-3" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Copy correlation ID</TooltipContent>
          </Tooltip>
          
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => onCorrelationSearch(log.correlation_id)}
                className="h-6 w-6 p-0"
                aria-label="Search by correlation ID"
              >
                <Search className="h-3 w-3" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Search by correlation ID</TooltipContent>
          </Tooltip>
        </div>
      </TableCell>
    </TableRow>
  );
}; 