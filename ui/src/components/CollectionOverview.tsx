import React from 'react';
import { Card, CardContent } from '@/components/ui/card';

interface CollectionInfo {
  name: string;
  description: string;
  recordCount: number;
  size: string;
  created: string;
  lastModified: string;
  status: string;
}

interface SchemaField {
  name: string;
  type: string;
  required: boolean;
  unique: boolean;
  indexed: boolean;
}

interface CollectionOverviewProps {
  collectionInfo: CollectionInfo;
  schemaFields: SchemaField[];
}

/**
 * Component for displaying collection overview statistics
 * Shows record count, storage size, field count, and index count
 */
export const CollectionOverview: React.FC<CollectionOverviewProps> = ({
  collectionInfo,
  schemaFields,
}) => {
  return (
    <div className="grid gap-3 md:gap-4 grid-cols-2 md:grid-cols-4">
      <Card>
        <CardContent className="p-4">
          <div className="text-xl md:text-2xl font-bold">
            {collectionInfo.recordCount.toLocaleString()}
          </div>
          <p className="text-xs text-muted-foreground">Total Records</p>
        </CardContent>
      </Card>
      <Card>
        <CardContent className="p-4">
          <div className="text-xl md:text-2xl font-bold">{collectionInfo.size}</div>
          <p className="text-xs text-muted-foreground">Storage Size</p>
        </CardContent>
      </Card>
      <Card>
        <CardContent className="p-4">
          <div className="text-xl md:text-2xl font-bold">{schemaFields.length}</div>
          <p className="text-xs text-muted-foreground">Fields</p>
        </CardContent>
      </Card>
      <Card>
        <CardContent className="p-4">
          <div className="text-xl md:text-2xl font-bold">
            {schemaFields.filter((f) => f.indexed).length}
          </div>
          <p className="text-xs text-muted-foreground">Indexes</p>
        </CardContent>
      </Card>
    </div>
  );
}; 