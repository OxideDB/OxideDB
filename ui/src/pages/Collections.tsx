import React, { useState, useEffect } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { Plus, Trash2, Database, Shield, AlertTriangle, Search, MoreHorizontal, Edit, FileText } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import PageLayout from '@/components/PageLayout';
import { AdminState, LoadingState } from '@/components/admin/AdminState';
import { StatusIndicator } from '@/components/admin/StatusIndicator';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { cn } from '@/lib/utils';
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
        <LoadingState label="Loading collections" />
      </PageLayout>
    );
  }

  const headerActions = (
    <>
      <div className="relative min-w-[220px] flex-1 sm:w-80 sm:flex-none">
        <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-muted-foreground" />
        <Input
          placeholder="Search collections..."
          value={searchTerm}
          onChange={(e) => setSearchTerm(e.target.value)}
          className="pl-10"
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
        <Alert variant="destructive">
          <AlertTriangle className="h-4 w-4" />
          <AlertDescription>
            {error}
            <Button
              onClick={() => setError(null)}
              variant="ghost"
              size="sm"
              className="mt-2 h-auto p-0 text-destructive hover:text-destructive"
            >
              Dismiss
            </Button>
          </AlertDescription>
        </Alert>
      )}

      {/* Collections Grid */}
      {filteredCollections.length === 0 ? (
        <AdminState
          title={searchTerm ? "No collections found" : "No collections"}
          description={
            searchTerm
              ? "Try a different name or clear the search field."
              : "Create a collection with a schema before adding records."
          }
          icon={Database}
          action={
            !searchTerm ? (
              <Button asChild>
                <Link to="/collections/new">
                  <Plus className="h-4 w-4" />
                  Create Collection
                </Link>
              </Button>
            ) : undefined
          }
        />
      ) : (
        <div className="grid gap-4 grid-cols-1 md:grid-cols-2 xl:grid-cols-3">
          {filteredCollections.map((collection) => {
            const stats = collectionStats[collection.name];
            const isSystemCollection = apiService.isSystemCollection(collection);
            const isAuthCollection = collection.collection_type === 'auth';
            const isSingleCollection = collection.collection_type === 'single';
            const fieldCount = Object.keys(collection.fields ?? {}).length;
            return (
              <Card 
                key={collection.id} 
                className={cn(
                  "group cursor-pointer transition-colors hover:border-primary/30 hover:bg-card/95",
                  isSystemCollection && "border-warning/30 bg-warning/5"
                )}
                onClick={() => navigate(`/collections/${encodeURIComponent(collection.name)}`)}
              >
                <CardHeader className="pb-3">
                  <div className="flex items-start justify-between">
                    <div className="flex items-center gap-2 min-w-0 flex-1">
                      {isSystemCollection ? (
                        <Shield className="h-5 w-5 text-warning flex-shrink-0" />
                      ) : isSingleCollection ? (
                        <FileText className="h-5 w-5 text-primary flex-shrink-0" />
                      ) : (
                        <Database className="h-5 w-5 text-primary flex-shrink-0" />
                      )}
                      <CardTitle className="text-lg truncate">{collection.name}</CardTitle>
                      <div className="flex items-center gap-2 ml-2">
                        {isSystemCollection && (
                          <StatusIndicator label="System" tone="warning" showDot={false} />
                        )}
                        {isAuthCollection && (
                          <StatusIndicator label="Auth" tone="info" showDot={false} />
                        )}
                        {isSingleCollection && (
                          <StatusIndicator label="Single" tone="success" showDot={false} />
                        )}
                      </div>
                    </div>
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button 
                          variant="ghost" 
                          size="sm" 
                          className="flex-shrink-0"
                          aria-label={`Actions for ${collection.name}`}
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
                            {isSingleCollection ? 'View Entry' : 'View Records'}
                          </Link>
                        </DropdownMenuItem>
                        {!isSystemCollection && (
                          <DropdownMenuItem 
                            className="text-destructive focus:text-destructive"
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
                      : isSingleCollection
                        ? "Single-entry collection for pages and static content"
                      : isSystemCollection 
                        ? "System collection for internal operations" 
                        : "User-defined collection for storing custom data"
                    }
                  </CardDescription>
                </CardHeader>
                <CardContent className="space-y-3">
                  <div className="grid grid-cols-3 gap-2 text-sm">
                    <div className="rounded-md border bg-muted/30 p-2">
                      <div className="text-xs text-muted-foreground">Records</div>
                      <div className="mt-1 font-medium tabular-nums">
                        {stats ? stats.record_count.toLocaleString() : '-'}
                      </div>
                    </div>
                    <div className="rounded-md border bg-muted/30 p-2">
                      <div className="text-xs text-muted-foreground">Fields</div>
                      <div className="mt-1 font-medium tabular-nums">{fieldCount}</div>
                    </div>
                    <div className="rounded-md border bg-muted/30 p-2">
                      <div className="text-xs text-muted-foreground">Size</div>
                      <div className="mt-1 truncate font-medium tabular-nums">
                        {stats
                          ? stats.size_kb < 1024 
                            ? `${stats.size_kb.toFixed(1)} KB` 
                            : stats.size_kb < 1024 * 1024 
                              ? `${(stats.size_kb / 1024).toFixed(1)} MB` 
                              : `${(stats.size_kb / (1024 * 1024)).toFixed(1)} GB`
                          : '-'}
                      </div>
                    </div>
                  </div>
                  {isSystemCollection && (
                    <div className="flex items-center gap-1.5 border-t pt-3 text-xs text-warning">
                      <AlertTriangle className="h-3.5 w-3.5" />
                      <span>Protected system collection</span>
                    </div>
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
