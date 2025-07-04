import React from 'react';
import { Link } from 'react-router-dom';
import { Edit, Trash2, Download, FileIcon } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Badge } from '@/components/ui/badge';
import type { DbRecord, CollectionSchema, FieldType, FileReference, FieldDefinition } from '../types/api';
import type { FieldCustomization } from '../types/fieldCustomization';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';

interface RecordTableProps {
  records: DbRecord[];
  schema: CollectionSchema | null;
  collection: string;
  onDelete: (recordId: string) => void;
  orderedFields?: Array<{
    fieldName: string;
    fieldDef: FieldDefinition;
    customization: FieldCustomization;
  }>;
}

export const RecordTable: React.FC<RecordTableProps> = ({
  records,
  schema,
  collection,
  onDelete,
  orderedFields,
}) => {
  // Extract all unique data keys from records to create columns
  const getDataColumns = (): string[] => {
    // Use ordered fields if available (from field customization)
    if (orderedFields && orderedFields.length > 0) {
      return orderedFields.map(field => field.fieldName);
    }
    
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

  const getFieldTypeDisplayName = (fieldType: FieldType): string => {
    if (typeof fieldType === 'string') {
      return fieldType;
    }
    if (typeof fieldType === 'object' && 'relationship' in fieldType) {
      return 'relationship';
    }
    if (typeof fieldType === 'object' && 'file' in fieldType) {
      return 'file';
    }
    return 'unknown';
  };

  const formatFileSize = (bytes: number): string => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const downloadFile = async (fileRef: FileReference) => {
    try {
      const { apiService } = await import('../services/api');
      const blob = await apiService.downloadFile(collection, fileRef.file_id);
      
      // Create download link
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = fileRef.name;
      document.body.appendChild(a);
      a.click();
      window.URL.revokeObjectURL(url);
      document.body.removeChild(a);
    } catch (error) {
      console.error('Download failed:', error);
    }
  };

  const renderFileField = (value: any): React.ReactNode => {
    // Handle single file reference
    if (value && typeof value === 'object' && 'file_id' in value) {
      const fileRef = value as FileReference;
      return (
        <div className="flex items-center space-x-2 max-w-xs">
          <FileIcon className="h-4 w-4 text-muted-foreground" />
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm font-medium">{fileRef.name}</div>
            <div className="text-xs text-muted-foreground">
              {formatFileSize(fileRef.size)}
            </div>
          </div>
          <Button
            variant="ghost"
            size="sm"
            className="h-6 w-6 p-0"
            onClick={() => downloadFile(fileRef)}
          >
            <Download className="h-3 w-3" />
          </Button>
        </div>
      );
    }

    // Handle multiple file references
    if (Array.isArray(value) && value.length > 0 && typeof value[0] === 'object' && 'file_id' in value[0]) {
      const fileRefs = value as FileReference[];
      return (
        <div className="space-y-1 max-w-xs">
          {fileRefs.slice(0, 2).map((fileRef, index) => (
            <div key={fileRef.file_id} className="flex items-center space-x-2">
              <FileIcon className="h-3 w-3 text-muted-foreground" />
              <div className="min-w-0 flex-1">
                <div className="truncate text-xs">{fileRef.name}</div>
              </div>
              <Button
                variant="ghost"
                size="sm"
                className="h-4 w-4 p-0"
                onClick={() => downloadFile(fileRef)}
              >
                <Download className="h-2 w-2" />
              </Button>
            </div>
          ))}
          {fileRefs.length > 2 && (
            <div className="text-xs text-muted-foreground">
              +{fileRefs.length - 2} more files
            </div>
          )}
        </div>
      );
    }

    return <span className="text-muted-foreground italic">No files</span>;
  };

  const formatCellValue = (value: any, fieldName?: string): React.ReactNode => {
    if (value === null || value === undefined) {
      return <span className="text-muted-foreground italic">null</span>;
    }

    // Check if this is a file field based on the schema
    const fieldType = schema?.fields[fieldName || '']?.field_type;
    if (fieldType && typeof fieldType === 'object' && 'file' in fieldType) {
      return renderFileField(value);
    }
    
    if (typeof value === 'boolean') {
      return (
        <Badge variant={value ? "default" : "secondary"} className="text-xs">
          {value ? 'true' : 'false'}
        </Badge>
      );
    }
    
    if (typeof value === 'object') {
      // Check if this looks like a file reference
      if ('file_id' in value && 'name' in value && 'mime_type' in value) {
        return renderFileField(value);
      }
      
      // Check if this is an array of file references
      if (Array.isArray(value) && value.length > 0 && typeof value[0] === 'object' && 'file_id' in value[0]) {
        return renderFileField(value);
      }

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
                    {getFieldTypeDisplayName(schema.fields[column].field_type)}
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
              <Tooltip>
                <TooltipTrigger>
                  <span className="text-xs text-muted-foreground">
                    {new Date(Number(record.created_at) * 1000).toLocaleDateString()}
                  </span>
                </TooltipTrigger>
                <TooltipContent>
                  <p>Created: {new Date(Number(record.created_at) * 1000).toLocaleString()}</p>
                  <p>Updated: {new Date(Number(record.updated_at) * 1000).toLocaleString()}</p>
                </TooltipContent>
              </Tooltip>
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