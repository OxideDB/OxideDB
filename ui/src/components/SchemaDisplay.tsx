import React from 'react';
import { Link } from 'react-router-dom';
import { Database, Edit } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';

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
                        <Badge className="bg-red-100 text-red-800 text-xs">Required</Badge>
                      )}
                      {field.unique && (
                        <Badge className="bg-blue-100 text-blue-800 text-xs">Unique</Badge>
                      )}
                      {field.indexed && (
                        <Badge className="bg-green-100 text-green-800 text-xs">Indexed</Badge>
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
                          <Badge className="bg-red-100 text-red-800">Required</Badge>
                        ) : (
                          <Badge variant="secondary">Optional</Badge>
                        )}
                      </TableCell>
                      <TableCell>
                        {field.unique ? (
                          <Badge className="bg-blue-100 text-blue-800">Unique</Badge>
                        ) : (
                          <span className="text-muted-foreground">-</span>
                        )}
                      </TableCell>
                      <TableCell>
                        {field.indexed ? (
                          <Badge className="bg-green-100 text-green-800">Indexed</Badge>
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
          <div className="text-center py-8">
            <Database className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
            <h3 className="text-lg font-medium mb-2">No schema defined</h3>
            <p className="text-muted-foreground mb-4">
              Define a schema to structure your collection data
            </p>
            <Link to={`/collections/${encodeURIComponent(collection)}/edit`}>
              <Button>
                <Edit className="h-4 w-4 mr-2" />
                Define Schema
              </Button>
            </Link>
          </div>
        )}
      </CardContent>
    </Card>
  );
}; 