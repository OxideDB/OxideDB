import React, { useState, useEffect } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { Plus, Trash2, Database, Shield, AlertTriangle, Search, MoreHorizontal, Edit } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Input } from '@/components/ui/input';
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import PageLayout from '@/components/PageLayout';
import { apiService } from '../services/api';
import type { CollectionStats, CollectionSchema } from '../types/api';

const Collections: React.FC = () => {
  const navigate = useNavigate();
  const [collections, setCollections] = useState<CollectionSchema[]>([]);
  const [collectionStats, setCollectionStats] = useState<Record<string, CollectionStats>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [searchTerm, setSearchTerm] = useState("");

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

  // Filter collections based on search term
  const filteredCollections = collections.filter(
    (collection) =>
      collection.name.toLowerCase().includes(searchTerm.toLowerCase())
  );

  if (loading) {
    return (
      <PageLayout title="Collections" description="Manage your database collections">
        <div className="flex items-center justify-center h-64">
          <div className="text-muted-foreground">Loading collections...</div>
        </div>
      </PageLayout>
    );
  }

  const headerActions = (
    <>
      <div className="relative flex-1 max-w-full sm:max-w-sm transition-all duration-300 focus-within:max-w-full focus-within:flex-[2]">
        <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-muted-foreground" />
        <Input
          placeholder="Search collections..."
          value={searchTerm}
          onChange={(e) => setSearchTerm(e.target.value)}
          className="pl-10 transition-all duration-300"
        />
      </div>

      <Button asChild className="shrink-0 sm:w-auto">
        <Link to="/collections/new">
          <Plus className="h-4 w-4 sm:mr-2" />
          <span className="hidden sm:inline">Create Collection</span>
        </Link>
      </Button>
    </>
  );

  return (
    <PageLayout 
      title="Collections" 
      description="Manage your database collections"
      headerActions={headerActions}
    >

      {error && (
        <Card className="border-destructive">
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

      {/* Collections Grid */}
      {filteredCollections.length === 0 ? (
        <div className="text-center py-12">
          <Database className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h3 className="text-lg font-medium mb-2">
            {searchTerm ? "No collections found" : "No collections"}
          </h3>
          <p className="text-muted-foreground mb-4 px-4">
            {searchTerm 
              ? "Try adjusting your search terms." 
              : "Get started by creating a new collection with a defined schema."
            }
          </p>
          {!searchTerm && (
            <Button asChild>
              <Link to="/collections/new">
                <Plus className="h-4 w-4 mr-2" />
                Create Collection
              </Link>
            </Button>
          )}
        </div>
      ) : (
        <div className="grid gap-4 grid-cols-1 md:grid-cols-2 xl:grid-cols-3">
          {filteredCollections.map((collection) => {
            const stats = collectionStats[collection.name];
            const isSystemCollection = apiService.isSystemCollection(collection);
            const isAuthCollection = collection.collection_type === 'auth';
            return (
              <Card 
                key={collection.id} 
                className={`hover:shadow-md transition-shadow cursor-pointer ${isSystemCollection ? 'border-orange-200' : ''}`}
                onClick={() => navigate(`/collections/${encodeURIComponent(collection.name)}`)}
              >
                <CardHeader className="pb-3">
                  <div className="flex items-start justify-between">
                    <div className="flex items-center gap-2 min-w-0 flex-1">
                      {isSystemCollection ? (
                        <Shield className="h-5 w-5 text-orange-500 flex-shrink-0" />
                      ) : (
                        <Database className="h-5 w-5 text-blue-500 flex-shrink-0" />
                      )}
                      <CardTitle className="text-lg truncate">{collection.name}</CardTitle>
                      <div className="flex items-center gap-2 ml-2">
                        {isSystemCollection && (
                          <Badge variant="outline" className="text-orange-600 border-orange-200">
                            System
                          </Badge>
                        )}
                        {isAuthCollection && (
                          <Badge variant="outline" className="text-blue-600 border-blue-200">
                            Auth
                          </Badge>
                        )}
                      </div>
                    </div>
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button 
                          variant="ghost" 
                          size="sm" 
                          className="flex-shrink-0"
                          onClick={(e) => e.stopPropagation()}
                        >
                          <MoreHorizontal className="h-4 w-4" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem asChild>
                          <Link to={`/collections/${encodeURIComponent(collection.name)}/edit`}>
                            <Edit className="h-4 w-4 mr-2" />
                            Edit Schema
                          </Link>
                        </DropdownMenuItem>
                        <DropdownMenuItem asChild>
                          <Link to={`/collections/${encodeURIComponent(collection.name)}`}>
                            <Database className="h-4 w-4 mr-2" />
                            View Records
                          </Link>
                        </DropdownMenuItem>
                        {!isSystemCollection && (
                          <DropdownMenuItem 
                            className="text-red-600"
                            onClick={() => handleDeleteCollection(collection)}
                          >
                            <Trash2 className="h-4 w-4 mr-2" />
                            Delete Collection
                          </DropdownMenuItem>
                        )}
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                  <CardDescription className="line-clamp-2">
                    {isAuthCollection 
                      ? "Authentication collection for user management and security" 
                      : isSystemCollection 
                        ? "System collection for internal operations" 
                        : "User-defined collection for storing custom data"
                    }
                  </CardDescription>
                </CardHeader>
                <CardContent className="space-y-3">
                  {stats && (
                    <>
                      <div className="grid grid-cols-2 gap-3 text-sm">
                        <div className="flex items-center justify-between">
                          <span className="text-muted-foreground">Records</span>
                          <span className="font-medium">{stats.record_count.toLocaleString()}</span>
                        </div>
                        <div className="flex items-center justify-between">
                          <span className="text-muted-foreground">Status</span>
                          <Badge variant="secondary" className="bg-green-100 text-green-800">
                            Active
                          </Badge>
                        </div>
                      </div>
                      {isSystemCollection && (
                        <div className="flex items-center gap-1 text-xs text-orange-600 pt-2 border-t">
                          <AlertTriangle className="h-3 w-3" />
                          <span>Protected system collection</span>
                        </div>
                      )}
                    </>
                  )}
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}
    </PageLayout>
  );
};

export default Collections; 