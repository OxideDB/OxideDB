import React from 'react';
import { Button } from '../ui/button';
import { Input } from '../ui/input';
import { Label } from '../ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../ui/select';
import { Tooltip, TooltipContent, TooltipTrigger } from '../ui/tooltip';
import { Search, RefreshCw, X } from 'lucide-react';
import type { LogQueryParams } from '../../types/api';

interface LogFiltersProps {
  searchTerm: string;
  filters: LogQueryParams;
  selectedCorrelationId: string;
  loading: boolean;
  onSearchTermChange: (term: string) => void;
  onFiltersChange: (filters: LogQueryParams) => void;
  onSearch: () => void;
  onClearSearch: () => void;
}

export const LogFilters: React.FC<LogFiltersProps> = ({
  searchTerm,
  filters,
  selectedCorrelationId,
  loading,
  onSearchTermChange,
  onFiltersChange,
  onSearch,
  onClearSearch
}) => {
  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !selectedCorrelationId) {
      onSearch();
    }
  };

  return (
    <div className="space-y-4">
      {/* Search and Level Filter Row */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 items-end">
        <div className="lg:col-span-2">
          <Label htmlFor="log-search">Search Messages</Label>
          <div className="relative">
            <Input
              id="log-search"
              placeholder="Search log messages..."
              value={searchTerm}
              onChange={(e) => onSearchTermChange(e.target.value)}
              onKeyPress={handleKeyPress}
              disabled={!!selectedCorrelationId || loading}
              className="pr-10"
            />
            {searchTerm && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => onSearchTermChange('')}
                    className="absolute right-2 top-1/2 -translate-y-1/2 h-6 w-6 p-0"
                    aria-label="Clear search"
                  >
                    <X className="h-4 w-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Clear search</TooltipContent>
              </Tooltip>
            )}
          </div>
        </div>

        <div>
          <Label htmlFor="log-level">Log Level</Label>
          <Select 
            value={filters.level || "all"} 
            onValueChange={(value) => 
              onFiltersChange({ ...filters, level: value === "all" ? "" : value })
            }
            disabled={!!selectedCorrelationId || loading}
          >
            <SelectTrigger id="log-level">
              <SelectValue placeholder="All levels" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All Levels</SelectItem>
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
              <SelectItem value="DEBUG">
                <div className="flex items-center space-x-2">
                  <div className="w-2 h-2 rounded-full bg-gray-500"></div>
                  <span>Debug</span>
                </div>
              </SelectItem>
              <SelectItem value="TRACE">
                <div className="flex items-center space-x-2">
                  <div className="w-2 h-2 rounded-full bg-gray-400"></div>
                  <span>Trace</span>
                </div>
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        {/* Search Button - Now aligned with inputs */}
        <div className="flex items-end">
          {selectedCorrelationId ? (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button onClick={onClearSearch} variant="outline" disabled={loading} className="w-full">
                  <RefreshCw className="h-4 w-4 mr-2" />
                  Clear Search
                </Button>
              </TooltipTrigger>
              <TooltipContent>Clear correlation ID search and show all logs</TooltipContent>
            </Tooltip>
          ) : (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button onClick={onSearch} disabled={loading} className="w-full">
                  <Search className="h-4 w-4 mr-2" />
                  {loading ? 'Searching...' : 'Search'}
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                Search logs with current filters
                {searchTerm && <div className="text-xs">Enter to search quickly</div>}
              </TooltipContent>
            </Tooltip>
          )}
        </div>
      </div>

      {/* Filter Summary */}
      {(filters.level || searchTerm || selectedCorrelationId) && (
        <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
          {filters.level && (
            <span className="px-2 py-1 bg-muted rounded-md">
              Level: {filters.level}
            </span>
          )}
          {searchTerm && (
            <span className="px-2 py-1 bg-muted rounded-md">
              Search: "{searchTerm.length > 20 ? searchTerm.slice(0, 20) + '...' : searchTerm}"
            </span>
          )}
          {selectedCorrelationId && (
            <span className="px-2 py-1 bg-primary/10 rounded-md">
              Correlation: {selectedCorrelationId.slice(0, 8)}...
            </span>
          )}
        </div>
      )}
    </div>
  );
}; 