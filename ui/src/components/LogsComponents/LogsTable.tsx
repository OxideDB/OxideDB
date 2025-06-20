import React from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../ui/card';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../ui/table';
import { Separator } from '../ui/separator';
import { LogFilters, LogEntryRow, PaginationControls } from './';
import type { LogEntry, LogQueryParams } from '../../types/api';

interface LogsTableProps {
  logs: LogEntry[];
  searchTerm: string;
  filters: LogQueryParams;
  selectedCorrelationId: string;
  loading: boolean;
  pagination: {
    offset: number;
    limit: number;
    total: number;
    hasMore: boolean;
  };
  onSearchTermChange: (term: string) => void;
  onFiltersChange: (filters: LogQueryParams) => void;
  onSearch: () => void;
  onClearSearch: () => void;
  onCorrelationSearch: (correlationId: string) => void;
  onPageChange: (newOffset: number) => void;
  onLimitChange: (newLimit: number) => void;
}

export const LogsTable: React.FC<LogsTableProps> = ({
  logs,
  searchTerm,
  filters,
  selectedCorrelationId,
  loading,
  pagination,
  onSearchTermChange,
  onFiltersChange,
  onSearch,
  onClearSearch,
  onCorrelationSearch,
  onPageChange,
  onLimitChange
}) => {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Log Entries</CardTitle>
        <CardDescription>
          {selectedCorrelationId ? 
            `Showing logs for correlation ID: ${selectedCorrelationId}` : 
            'Browse and filter system logs'
          }
        </CardDescription>
      </CardHeader>
      <CardContent>
        <LogFilters
          searchTerm={searchTerm}
          filters={filters}
          selectedCorrelationId={selectedCorrelationId}
          loading={loading}
          onSearchTermChange={onSearchTermChange}
          onFiltersChange={onFiltersChange}
          onSearch={onSearch}
          onClearSearch={onClearSearch}
        />

        <Separator className="my-4" />

        {loading && (
          <p className="text-center text-muted-foreground">Loading logs...</p>
        )}

        {/* Logs Table */}
        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Level</TableHead>
                <TableHead>Timestamp</TableHead>
                <TableHead>Module</TableHead>
                <TableHead>Message</TableHead>
                <TableHead>User</TableHead>
                <TableHead>Collection</TableHead>
                <TableHead>Correlation ID</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {logs.length === 0 && !loading ? (
                <TableRow>
                  <TableCell colSpan={7} className="text-center text-muted-foreground">
                    No logs found. Click "Search" to load logs.
                  </TableCell>
                </TableRow>
              ) : (
                logs.map((log) => (
                  <LogEntryRow
                    key={log.id}
                    log={log}
                    onCorrelationSearch={onCorrelationSearch}
                  />
                ))
              )}
            </TableBody>
          </Table>
        </div>

        {/* Pagination */}
        {logs.length > 0 && (
          <div className="mt-4">
            <PaginationControls
              offset={pagination.offset}
              limit={pagination.limit}
              total={pagination.total}
              hasMore={pagination.hasMore}
              loading={loading}
              onPageChange={onPageChange}
              onLimitChange={onLimitChange}
            />
          </div>
        )}
      </CardContent>
    </Card>
  );
}; 