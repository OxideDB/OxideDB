import React from 'react';
import { Link } from 'react-router-dom';
import { Edit, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Badge } from '@/components/ui/badge';
import type { DbRecord, CollectionSchema } from '../types/api';

interface RecordTableProps {
  records: DbRecord[];
  schema: CollectionSchema | null;
  collection: string;
  onDelete: (recordId: string) => void;
}

export const RecordTable: React.FC<RecordTableProps> = ({
  records,
  schema,
  collection,
  onDelete,
}) => {
  // Extract all unique data keys from records to create columns
  const getDataColumns = (): string[] => {
    if (schema && Object.keys(schema.fields).length > 0) {
      // Use schema fields as primary columns
      return Object.keys(schema.fields);
    }
    
    // Fallback: extract all unique keys from record data
    const allKeys = new Set<string>();
    records.forEach(record => {
      if (record.data && typeof record.data === 'object') {
        Object.keys(record.data).forEach(key => allKeys.add(key));
      }
    });
    return Array.from(allKeys).sort();
  };

  const formatCellValue = (value: any, fieldName?: string): React.ReactNode => {
    if (value === null || value === undefined) {
      return <span className="text-muted-foreground italic">null</span>;
    }
    
    if (typeof value === 'boolean') {
      return (
        <Badge variant={value ? "default" : "secondary"} className="text-xs">
          {value ? 'true' : 'false'}
        </Badge>
      );
    }
    
    if (typeof value === 'object') {
      const jsonStr = JSON.stringify(value);
      if (jsonStr.length > 50) {
        return (
          <div className="max-w-xs">
            <code className="text-xs bg-muted p-1 rounded block overflow-hidden text-ellipsis">
              {jsonStr.substring(0, 47)}...
            </code>
          </div>
        );
      }
      return (
        <code className="text-xs bg-muted p-1 rounded">
          {jsonStr}
        </code>
      );
    }
    
    if (typeof value === 'string' && value.length > 100) {
      return (
        <div className="max-w-xs">
          <span className="block overflow-hidden text-ellipsis" title={value}>
            {value.substring(0, 97)}...
          </span>
        </div>
      );
    }
    
    return String(value);
  };

  const dataColumns = getDataColumns();
  const maxColumns = 6; // Limit columns to prevent overcrowding
  const displayColumns = dataColumns.slice(0, maxColumns);
  const hasMoreColumns = dataColumns.length > maxColumns;

  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead className="w-24">ID</TableHead>
          {displayColumns.map(column => (
            <TableHead key={column} className="min-w-32">
              <div className="flex items-center space-x-2">
                <span>{column}</span>
                {schema?.fields[column] && (
                  <Badge variant="outline" className="text-xs">
                    {schema.fields[column].field_type}
                  </Badge>
                )}
              </div>
            </TableHead>
          ))}
          {hasMoreColumns && (
            <TableHead className="text-muted-foreground">
              +{dataColumns.length - maxColumns} more...
            </TableHead>
          )}
          <TableHead>Created</TableHead>
          <TableHead>Updated</TableHead>
          <TableHead className="text-right w-24">Actions</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {records.map((record) => (
          <TableRow key={record.id}>
            <TableCell className="font-mono text-sm">
              {record.id.length > 8 ? `${record.id.substring(0, 8)}...` : record.id}
            </TableCell>
            {displayColumns.map(column => (
              <TableCell key={column}>
                {formatCellValue(record.data?.[column], column)}
              </TableCell>
            ))}
            {hasMoreColumns && (
              <TableCell className="text-muted-foreground text-sm">
                <Button
                  asChild
                  variant="ghost"
                  size="sm"
                  className="h-auto p-0 text-xs"
                >
                  <Link to={`/collections/${encodeURIComponent(collection)}/edit/${record.id}`}>
                    View all
                  </Link>
                </Button>
              </TableCell>
            )}
            <TableCell className="text-muted-foreground text-xs">
              {new Date(record.created_at).toLocaleDateString()}
            </TableCell>
            <TableCell className="text-muted-foreground text-xs">
              {new Date(record.updated_at).toLocaleDateString()}
            </TableCell>
            <TableCell className="text-right">
              <div className="flex justify-end space-x-1">
                <Button
                  asChild
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8"
                >
                  <Link to={`/collections/${encodeURIComponent(collection)}/edit/${record.id}`}>
                    <Edit className="h-3 w-3" />
                  </Link>
                </Button>
                <Button
                  onClick={() => onDelete(record.id)}
                  variant="ghost"
                  size="icon"
                  className="h-8 w-8 text-destructive hover:text-destructive/80"
                >
                  <Trash2 className="h-3 w-3" />
                </Button>
              </div>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}; 