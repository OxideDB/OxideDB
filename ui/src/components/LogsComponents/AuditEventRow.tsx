import React from 'react';
import { TableCell, TableRow } from '../ui/table';
import { Badge } from '../ui/badge';
import { Button } from '../ui/button';
import { Tooltip, TooltipContent, TooltipTrigger } from '../ui/tooltip';
import { 
  User, Shield, Database, Settings, AlertTriangle, Activity, Info, Copy, Search 
} from 'lucide-react';
import type { SecurityAuditEvent, AuditEventType } from '../../types/api';

interface AuditEventRowProps {
  event: SecurityAuditEvent;
  onCorrelationSearch: (correlationId: string) => void;
}

const getAuditEventIcon = (eventType: AuditEventType) => {
  switch (eventType) {
    case 'Authentication': return <User className="h-4 w-4" />;
    case 'Authorization': return <Shield className="h-4 w-4" />;
    case 'DataAccess': return <Database className="h-4 w-4" />;
    case 'DataModification': return <Database className="h-4 w-4" />;
    case 'ConfigurationChange': return <Settings className="h-4 w-4" />;
    case 'SecurityViolation': return <AlertTriangle className="h-4 w-4" />;
    case 'PluginEvent': return <Activity className="h-4 w-4" />;
    case 'SystemEvent': return <Settings className="h-4 w-4" />;
    default: return <Info className="h-4 w-4" />;
  }
};

const getRiskBadgeVariant = (riskScore: number) => {
  if (riskScore > 70) return 'destructive';
  if (riskScore > 40) return 'secondary';
  return 'default';
};

const formatDate = (dateString: string): string => {
  return new Date(dateString).toLocaleString();
};

export const AuditEventRow: React.FC<AuditEventRowProps> = ({ event, onCorrelationSearch }) => {
  const handleCopyCorrelationId = async () => {
    try {
      await navigator.clipboard.writeText(event.correlation_id);
    } catch (error) {
      console.error('Failed to copy correlation ID:', error);
    }
  };

  return (
    <TableRow>
      <TableCell>
        <div className="flex items-center space-x-2">
          {getAuditEventIcon(event.event_type)}
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="text-sm cursor-help">
                {event.event_type}
              </span>
            </TooltipTrigger>
            <TooltipContent>
              Event Type: {event.event_type.replace(/([A-Z])/g, ' $1').trim()}
            </TooltipContent>
          </Tooltip>
        </div>
      </TableCell>
      
      <TableCell className="text-sm font-mono">
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="cursor-help">
              {formatDate(event.timestamp)}
            </span>
          </TooltipTrigger>
          <TooltipContent>
            <div className="text-xs">
              <div>Local time: {new Date(event.timestamp).toLocaleString()}</div>
              <div>UTC: {new Date(event.timestamp).toISOString()}</div>
            </div>
          </TooltipContent>
        </Tooltip>
      </TableCell>
      
      <TableCell className="font-medium">
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="cursor-help truncate block max-w-[120px]">
              {event.actor}
            </span>
          </TooltipTrigger>
          <TooltipContent>Actor: {event.actor}</TooltipContent>
        </Tooltip>
      </TableCell>
      
      <TableCell>
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="cursor-help truncate block max-w-[150px]">
              {event.action}
            </span>
          </TooltipTrigger>
          <TooltipContent className="max-w-sm">
            <div className="whitespace-pre-wrap break-words">
              Action: {event.action}
            </div>
          </TooltipContent>
        </Tooltip>
      </TableCell>
      
      <TableCell>
        {event.target ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <Badge variant="outline" className="cursor-help max-w-[120px] truncate">
                {event.target}
              </Badge>
            </TooltipTrigger>
            <TooltipContent>Target: {event.target}</TooltipContent>
          </Tooltip>
        ) : (
          <span className="text-muted-foreground">-</span>
        )}
      </TableCell>
      
      <TableCell>
        <Tooltip>
          <TooltipTrigger asChild>
            <Badge 
              variant={event.result === 'success' ? 'default' : 'destructive'}
              className="cursor-help"
            >
              {event.result}
            </Badge>
          </TooltipTrigger>
          <TooltipContent>
            Operation result: {event.result}
          </TooltipContent>
        </Tooltip>
      </TableCell>
      
      <TableCell>
        {event.risk_score ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <Badge 
                variant={getRiskBadgeVariant(event.risk_score)}
                className="cursor-help"
              >
                {event.risk_score}
              </Badge>
            </TooltipTrigger>
            <TooltipContent>
              <div className="text-xs">
                <div>Risk Score: {event.risk_score}/100</div>
                <div className="text-muted-foreground">
                  {event.risk_score > 70 ? 'High Risk' : 
                   event.risk_score > 40 ? 'Medium Risk' : 'Low Risk'}
                </div>
              </div>
            </TooltipContent>
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
                {event.correlation_id.slice(0, 8)}...
              </code>
            </TooltipTrigger>
            <TooltipContent>
              <div className="text-xs font-mono">
                {event.correlation_id}
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
                onClick={() => onCorrelationSearch(event.correlation_id)}
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