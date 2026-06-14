import React from 'react';
import { Link } from 'react-router-dom';
import { Database, Edit } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { StatusIndicator } from '@/components/admin/StatusIndicator';
import { getStatusTone } from '@/components/admin/statusUtils';
import type { CollectionSchema } from '../types/api';

interface CollectionInfo {
  name: string;
  description: string;
  recordCount: number;
  size: string;
  created: string;
  lastModified: string;
  status: string;
}

interface CollectionInfoProps {
  collectionInfo: CollectionInfo;
  collection: string;
  schema: CollectionSchema | null;
}

/**
 * Component for displaying detailed collection information
 * Shows collection metadata and provides action buttons
 */
export const CollectionInfo: React.FC<CollectionInfoProps> = ({
  collectionInfo,
  collection,
  schema,
}) => {
  return (
    <Card>
      <CardHeader>
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
          <div className="min-w-0 flex-1">
            <CardTitle className="flex items-center gap-2">
              <Database className="h-5 w-5 flex-shrink-0" />
              <span className="truncate">Collection Details</span>
            </CardTitle>
            <CardDescription className="line-clamp-2">
              {collectionInfo.description}
            </CardDescription>
          </div>
          <div className="flex flex-col sm:flex-row gap-2">
            <Link to={`/collections/${encodeURIComponent(collection)}/edit`}>
              <Button variant="outline" size="sm" className="w-full sm:w-auto">
                <Edit className="h-4 w-4 mr-2" />
                Edit Schema
              </Button>
            </Link>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 text-sm">
          <div>
            <span className="text-muted-foreground">Created:</span>
            <div className="font-medium">{collectionInfo.created}</div>
          </div>
          <div>
            <span className="text-muted-foreground">Last Modified:</span>
            <div className="font-medium">{collectionInfo.lastModified}</div>
          </div>
          <div>
            <span className="text-muted-foreground">Status:</span>
            <div>
              <StatusIndicator
                label={collectionInfo.status}
                tone={getStatusTone(collectionInfo.status)}
              />
            </div>
          </div>
          <div>
            <span className="text-muted-foreground">Type:</span>
            <div className="font-medium">
              {schema?.collection_type || 'User Collection'}
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}; 
