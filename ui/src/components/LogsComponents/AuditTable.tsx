import React from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../ui/card';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../ui/table';
import { Separator } from '../ui/separator';
import { AuditFilters, AuditEventRow, PaginationControls } from './';
import type { SecurityAuditEvent, AuditQueryParams } from '../../types/api';

interface AuditTableProps {
  auditEvents: SecurityAuditEvent[];
  filters: AuditQueryParams;
  loading: boolean;
  pagination: {
    offset: number;
    limit: number;
    total: number;
    hasMore: boolean;
  };
  onFiltersChange: (filters: AuditQueryParams) => void;
  onSearch: () => void;
  onCorrelationSearch: (correlationId: string) => void;
  onPageChange: (newOffset: number) => void;
  onLimitChange: (newLimit: number) => void;
}

export const AuditTable: React.FC<AuditTableProps> = ({
  auditEvents,
  filters,
  loading,
  pagination,
  onFiltersChange,
  onSearch,
  onCorrelationSearch,
  onPageChange,
  onLimitChange
}) => {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Security Audit Events</CardTitle>
        <CardDescription>
          Monitor security-related events and activities
        </CardDescription>
      </CardHeader>
      <CardContent>
        <AuditFilters
          filters={filters}
          loading={loading}
          onFiltersChange={onFiltersChange}
          onSearch={onSearch}
        />

        <Separator className="my-4" />

        {loading && (
          <p className="text-center text-muted-foreground">Loading audit events...</p>
        )}

        {/* Audit Events Table */}
        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Type</TableHead>
                <TableHead>Timestamp</TableHead>
                <TableHead>Actor</TableHead>
                <TableHead>Action</TableHead>
                <TableHead>Target</TableHead>
                <TableHead>Result</TableHead>
                <TableHead>Risk</TableHead>
                <TableHead>Correlation ID</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {auditEvents.length === 0 && !loading ? (
                <TableRow>
                  <TableCell colSpan={8} className="text-center text-muted-foreground">
                    No audit events found. Click "Search" to load events.
                  </TableCell>
                </TableRow>
              ) : (
                auditEvents.map((event) => (
                  <AuditEventRow
                    key={event.id}
                    event={event}
                    onCorrelationSearch={onCorrelationSearch}
                  />
                ))
              )}
            </TableBody>
          </Table>
        </div>

        {/* Pagination */}
        {auditEvents.length > 0 && (
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