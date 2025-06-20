import React from 'react';
import { Button } from '../ui/button';
import { Label } from '../ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../ui/select';
import { Tooltip, TooltipContent, TooltipTrigger } from '../ui/tooltip';
import { Search } from 'lucide-react';
import type { AuditQueryParams } from '../../types/api';

interface AuditFiltersProps {
  filters: AuditQueryParams;
  loading: boolean;
  onFiltersChange: (filters: AuditQueryParams) => void;
  onSearch: () => void;
}

export const AuditFilters: React.FC<AuditFiltersProps> = ({
  filters,
  loading,
  onFiltersChange,
  onSearch
}) => {
  return (
    <div className="space-y-4">
      {/* Filter Controls */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        <div>
          <Label htmlFor="audit-severity">Severity</Label>
          <Select 
            value={filters.severity || "all"} 
            onValueChange={(value) => 
              onFiltersChange({ ...filters, severity: value === "all" ? "" : value })
            }
            disabled={loading}
          >
            <SelectTrigger id="audit-severity">
              <SelectValue placeholder="All severities" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All Severities</SelectItem>
              <SelectItem value="ERROR">
                <div className="flex items-center space-x-2">
                  <div className="w-2 h-2 rounded-full bg-red-500"></div>
                  <span>Error</span>
                </div>
              </SelectItem>
              <SelectItem value="WARN">
                <div className="flex items-center space-x-2">
                  <div className="w-2 h-2 rounded-full bg-yellow-500"></div>
                  <span>Warning</span>
                </div>
              </SelectItem>
              <SelectItem value="INFO">
                <div className="flex items-center space-x-2">
                  <div className="w-2 h-2 rounded-full bg-blue-500"></div>
                  <span>Info</span>
                </div>
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div>
          <Label htmlFor="audit-event-type">Event Type</Label>
          <Select 
            value={filters.event_type || "all"} 
            onValueChange={(value) => 
              onFiltersChange({ ...filters, event_type: value === "all" ? "" : value })
            }
            disabled={loading}
          >
            <SelectTrigger id="audit-event-type">
              <SelectValue placeholder="All event types" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All Event Types</SelectItem>
              <SelectItem value="Authentication">Authentication</SelectItem>
              <SelectItem value="Authorization">Authorization</SelectItem>
              <SelectItem value="DataAccess">Data Access</SelectItem>
              <SelectItem value="DataModification">Data Modification</SelectItem>
              <SelectItem value="ConfigurationChange">Configuration Change</SelectItem>
              <SelectItem value="SecurityViolation">Security Violation</SelectItem>
              <SelectItem value="PluginEvent">Plugin Event</SelectItem>
              <SelectItem value="SystemEvent">System Event</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div className="flex items-end">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button 
                onClick={onSearch} 
                disabled={loading}
                className="w-full md:w-auto"
              >
                <Search className="h-4 w-4 mr-2" />
                {loading ? 'Searching...' : 'Search'}
              </Button>
            </TooltipTrigger>
            <TooltipContent>Search audit events with current filters</TooltipContent>
          </Tooltip>
        </div>
      </div>

      {/* Filter Summary */}
      <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
        {filters.severity && (
          <span className="px-2 py-1 bg-muted rounded-md">
            Severity: {filters.severity}
          </span>
        )}
        {filters.event_type && (
          <span className="px-2 py-1 bg-muted rounded-md">
            Type: {filters.event_type.replace(/([A-Z])/g, ' $1').trim()}
          </span>
        )}
        {!filters.severity && !filters.event_type && (
          <span className="text-muted-foreground">No filters applied</span>
        )}
      </div>
    </div>
  );
}; 