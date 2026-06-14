import React from 'react';
import { Link } from 'react-router-dom';
import { Database, Edit } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { AdminState } from '@/components/admin/AdminState';
import { StatusIndicator } from '@/components/admin/StatusIndicator';

interface SchemaField {
  name: string;
  type: string;
  required: boolean;
  unique: boolean;
  indexed: boolean;
}

interface SchemaDisplayProps {
  schemaFields: SchemaField[];
  collection: string;
}

/**
 * Component for displaying collection schema information
 * Shows field definitions and constraints with mobile-responsive design
 */
export const SchemaDisplay: React.FC<SchemaDisplayProps> = ({
  schemaFields,
  collection,
}) => {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Schema</CardTitle>
        <CardDescription>Field definitions and constraints</CardDescription>
      </CardHeader>
      <CardContent>
        {schemaFields.length > 0 ? (
          <>
            {/* Mobile view - Cards */}
            <div className="block md:hidden space-y-3">
              {schemaFields.map((field) => (
                <Card key={field.name} className="p-4">
                  <div className="space-y-2">
                    <div className="flex items-center justify-between">
                      <h4 className="font-medium">{field.name}</h4>
                      <Badge variant="outline">{field.type}</Badge>
                    </div>
                    <div className="flex flex-wrap gap-2">
                      {field.required && (
                        <StatusIndicator label="Required" tone="warning" showDot={false} />
                      )}
                      {field.unique && (
                        <StatusIndicator label="Unique" tone="info" showDot={false} />
                      )}
                      {field.indexed && (
                        <StatusIndicator label="Indexed" tone="success" showDot={false} />
                      )}
                    </div>
                  </div>
                </Card>
              ))}
            </div>

            {/* Desktop view - Table */}
            <div className="hidden md:block">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Field Name</TableHead>
                    <TableHead>Type</TableHead>
                    <TableHead>Required</TableHead>
                    <TableHead>Unique</TableHead>
                    <TableHead>Indexed</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {schemaFields.map((field) => (
                    <TableRow key={field.name}>
                      <TableCell className="font-medium">{field.name}</TableCell>
                      <TableCell>
                        <Badge variant="outline">{field.type}</Badge>
                      </TableCell>
                      <TableCell>
                        {field.required ? (
                          <StatusIndicator label="Required" tone="warning" showDot={false} />
                        ) : (
                          <Badge variant="secondary">Optional</Badge>
                        )}
                      </TableCell>
                      <TableCell>
                        {field.unique ? (
                          <StatusIndicator label="Unique" tone="info" showDot={false} />
                        ) : (
                          <span className="text-muted-foreground">-</span>
                        )}
                      </TableCell>
                      <TableCell>
                        {field.indexed ? (
                          <StatusIndicator label="Indexed" tone="success" showDot={false} />
                        ) : (
                          <span className="text-muted-foreground">-</span>
                        )}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          </>
        ) : (
          <AdminState
            title="No schema defined"
            description="Define a schema to structure your collection data."
            icon={Database}
            action={
              <Button asChild>
                <Link to={`/collections/${encodeURIComponent(collection)}/edit`}>
                  <Edit className="h-4 w-4" />
                  Define Schema
                </Link>
              </Button>
            }
          />
        )}
      </CardContent>
    </Card>
  );
}; 
