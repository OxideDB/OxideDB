import React, { useState, useEffect } from 'react';
import { Link } from 'react-router-dom';
import { Plus, Trash2, BarChart3, Database, Settings, Shield, AlertTriangle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { apiService } from '../services/api';
import type { CollectionStats, CollectionSchema } from '../types/api';

const Collections: React.FC = () => {
  const [collections, setCollections] = useState<CollectionSchema[]>([]);
  const [collectionStats, setCollectionStats] = useState<Record<string, CollectionStats>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchCollections();
  }, []);

  const fetchCollections = async () => {
    try {
      setLoading(true);
      const collectionsData = await apiService.getCollections();
      setCollections(collectionsData);
      
      // Fetch stats for each collection
      const stats: Record<string, CollectionStats> = {};
      await Promise.all(
        collectionsData.map(async (collection) => {
          try {
            const collectionStat = await apiService.getCollectionStats(collection.name);
            stats[collection.name] = collectionStat;
          } catch (err) {
            console.warn(`Failed to fetch stats for collection ${collection.name}:`, err);
          }
        })
      );
      setCollectionStats(stats);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch collections');
    } finally {
      setLoading(false);
    }
  };

  const handleDeleteCollection = async (collection: CollectionSchema) => {
    // Prevent deletion of system collections
    if (apiService.isSystemCollection(collection)) {
      setError('System collections cannot be deleted');
      return;
    }

    if (!confirm(`Are you sure you want to delete the collection "${collection.name}"? This action cannot be undone.`)) {
      return;
    }

    try {
      await apiService.deleteCollection(collection.name);
      await fetchCollections(); // Refresh the list
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete collection');
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-muted-foreground">Loading collections...</div>
      </div>
    );
  }

  return (
    <div>
      <div className="flex justify-between items-center mb-8">
        <div>
          <h1 className="text-2xl font-bold text-foreground">Collections</h1>
          <p className="text-muted-foreground mt-1">Manage your database collections</p>
        </div>
        <Button asChild>
          <Link to="/collections/new">
            <Plus className="h-4 w-4 mr-2" />
            New Collection
          </Link>
        </Button>
      </div>

      {error && (
        <Card className="mb-6 border-destructive">
          <CardContent className="p-4">
            <div className="text-destructive">{error}</div>
            <Button
              onClick={() => setError(null)}
              variant="ghost"
              size="sm"
              className="text-destructive text-sm mt-2 hover:text-destructive/80 p-0 h-auto"
            >
              Dismiss
            </Button>
          </CardContent>
        </Card>
      )}

      {collections.length === 0 ? (
        <Card>
          <CardContent className="text-center py-12">
            <Database className="mx-auto h-12 w-12 text-muted-foreground" />
            <CardTitle className="mt-4 text-lg">No collections</CardTitle>
            <CardDescription className="mt-2">Get started by creating a new collection with a defined schema.</CardDescription>
            <div className="mt-6">
              <Button asChild>
                <Link to="/collections/new">
                  Create Collection
                </Link>
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : (
        <div className="space-y-8">
          {/* Base Collections */}
          {collections.filter(c => c.collection_type === 'base').length > 0 && (
            <div>
              <div className="flex items-center gap-2 mb-4">
                <Database className="h-5 w-5 text-primary" />
                <h2 className="text-lg font-semibold">Base Collections</h2>
                <Badge variant="secondary">{collections.filter(c => c.collection_type === 'base').length}</Badge>
              </div>
              <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
                {collections.filter(c => c.collection_type === 'base').map((collection) => {
                  const stats = collectionStats[collection.name];
                  return (
                    <Card key={collection.id}>
                      <CardHeader className="pb-4">
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2">
                            <CardTitle className="text-lg">{collection.name}</CardTitle>
                          </div>
                          <div className="flex space-x-2">
                            <Button
                              asChild
                              variant="ghost"
                              size="icon"
                              className="h-8 w-8 text-primary hover:text-primary/80"
                              title="View records"
                            >
                              <Link to={`/collections/${encodeURIComponent(collection.name)}`}>
                                <BarChart3 className="h-4 w-4" />
                              </Link>
                            </Button>
                            <Button
                              asChild
                              variant="ghost"
                              size="icon"
                              className="h-8 w-8 text-muted-foreground hover:text-foreground"
                              title="Edit schema"
                            >
                              <Link to={`/collections/${encodeURIComponent(collection.name)}/edit`}>
                                <Settings className="h-4 w-4" />
                              </Link>
                            </Button>
                            <Button
                              onClick={() => handleDeleteCollection(collection)}
                              variant="ghost"
                              size="icon"
                              className="h-8 w-8 text-destructive hover:text-destructive/80"
                              title="Delete collection"
                            >
                              <Trash2 className="h-4 w-4" />
                            </Button>
                          </div>
                        </div>
                        {stats && (
                          <CardDescription>
                            <div className="space-y-1">
                              <p>Records: {stats.record_count}</p>
                              <p>Status: Active</p>
                            </div>
                          </CardDescription>
                        )}
                      </CardHeader>
                      <CardContent className="pt-0">
                        <Button asChild variant="outline" className="w-full">
                          <Link to={`/collections/${encodeURIComponent(collection.name)}`}>
                            Manage Records
                          </Link>
                        </Button>
                      </CardContent>
                    </Card>
                  );
                })}
              </div>
            </div>
          )}

          {/* System Collections */}
          {collections.filter(c => c.collection_type === 'auth').length > 0 && (
            <div>
              <div className="flex items-center gap-2 mb-4">
                <Shield className="h-5 w-5 text-orange-500" />
                <h2 className="text-lg font-semibold">System Collections</h2>
                <Badge variant="secondary">{collections.filter(c => c.collection_type === 'auth').length}</Badge>
                <Badge variant="outline" className="text-orange-600 border-orange-200">
                  <AlertTriangle className="h-3 w-3 mr-1" />
                  Protected
                </Badge>
              </div>
              <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
                {collections.filter(c => c.collection_type === 'auth').map((collection) => {
                  const stats = collectionStats[collection.name];
                  return (
                    <Card key={collection.id} className="border-orange-200">
                      <CardHeader className="pb-4">
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2">
                            <CardTitle className="text-lg">{collection.name}</CardTitle>
                            <Badge variant="outline" className="text-orange-600 border-orange-200">
                              System
                            </Badge>
                          </div>
                          <div className="flex space-x-2">
                            <Button
                              asChild
                              variant="ghost"
                              size="icon"
                              className="h-8 w-8 text-primary hover:text-primary/80"
                              title="View records"
                            >
                              <Link to={`/collections/${encodeURIComponent(collection.name)}`}>
                                <BarChart3 className="h-4 w-4" />
                              </Link>
                            </Button>
                            <Button
                              asChild
                              variant="ghost"
                              size="icon"
                              className="h-8 w-8 text-muted-foreground hover:text-foreground"
                              title="View schema (read-only)"
                            >
                              <Link to={`/collections/${encodeURIComponent(collection.name)}/schema`}>
                                <Settings className="h-4 w-4" />
                              </Link>
                            </Button>
                            <Button
                              onClick={() => handleDeleteCollection(collection)}
                              variant="ghost"
                              size="icon"
                              className="h-8 w-8 text-muted-foreground cursor-not-allowed opacity-50"
                              title="System collections cannot be deleted"
                              disabled
                            >
                              <Trash2 className="h-4 w-4" />
                            </Button>
                          </div>
                        </div>
                        {stats && (
                          <CardDescription>
                            <div className="space-y-1">
                              <p>Records: {stats.record_count}</p>
                              <p>Status: Active (System)</p>
                            </div>
                          </CardDescription>
                        )}
                      </CardHeader>
                      <CardContent className="pt-0">
                        <Button asChild variant="outline" className="w-full">
                          <Link to={`/collections/${encodeURIComponent(collection.name)}`}>
                            Manage Records
                          </Link>
                        </Button>
                      </CardContent>
                    </Card>
                  );
                })}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default Collections; 