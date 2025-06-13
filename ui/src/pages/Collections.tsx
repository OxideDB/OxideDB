import React, { useState, useEffect } from 'react';
import { Link } from 'react-router-dom';
import { Plus, Trash2, BarChart3, Database } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { apiService } from '../services/api';
import type { CollectionStats } from '../types/api';

const Collections: React.FC = () => {
  const [collections, setCollections] = useState<string[]>([]);
  const [collectionStats, setCollectionStats] = useState<Record<string, CollectionStats>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [newCollectionName, setNewCollectionName] = useState('');
  const [creating, setCreating] = useState(false);

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
            const collectionStat = await apiService.getCollectionStats(collection);
            stats[collection] = collectionStat;
          } catch (err) {
            console.warn(`Failed to fetch stats for collection ${collection}:`, err);
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

  const handleCreateCollection = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newCollectionName.trim()) return;

    try {
      setCreating(true);
      await apiService.createCollection({ name: newCollectionName.trim() });
      setNewCollectionName('');
      setShowCreateModal(false);
      await fetchCollections(); // Refresh the list
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create collection');
    } finally {
      setCreating(false);
    }
  };

  const handleDeleteCollection = async (collectionName: string) => {
    if (!confirm(`Are you sure you want to delete the collection "${collectionName}"? This action cannot be undone.`)) {
      return;
    }

    try {
      await apiService.deleteCollection(collectionName);
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
        <Dialog open={showCreateModal} onOpenChange={setShowCreateModal}>
          <DialogTrigger asChild>
            <Button>
              <Plus className="h-4 w-4 mr-2" />
              New Collection
            </Button>
          </DialogTrigger>
          <DialogContent className="sm:max-w-[425px]">
            <DialogHeader>
              <DialogTitle>Create New Collection</DialogTitle>
            </DialogHeader>
            <form onSubmit={handleCreateCollection} className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="collectionName">Collection Name</Label>
                <Input
                  id="collectionName"
                  value={newCollectionName}
                  onChange={(e) => setNewCollectionName(e.target.value)}
                  placeholder="Enter collection name"
                  autoFocus
                  required
                />
              </div>
              <div className="flex justify-end space-x-2">
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => {
                    setShowCreateModal(false);
                    setNewCollectionName('');
                  }}
                  disabled={creating}
                >
                  Cancel
                </Button>
                <Button
                  type="submit"
                  disabled={creating || !newCollectionName.trim()}
                >
                  {creating ? 'Creating...' : 'Create'}
                </Button>
              </div>
            </form>
          </DialogContent>
        </Dialog>
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
            <CardDescription className="mt-2">Get started by creating a new collection.</CardDescription>
            <div className="mt-6">
              <Button onClick={() => setShowCreateModal(true)}>
                Create Collection
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
          {collections.map((collection) => {
            const stats = collectionStats[collection];
            return (
              <Card key={collection}>
                <CardHeader className="pb-4">
                  <div className="flex items-center justify-between">
                    <CardTitle className="text-lg">{collection}</CardTitle>
                    <div className="flex space-x-2">
                      <Button
                        asChild
                        variant="ghost"
                        size="icon"
                        className="h-8 w-8 text-primary hover:text-primary/80"
                      >
                        <Link
                          to={`/collections/${encodeURIComponent(collection)}`}
                          title="View records"
                        >
                          <BarChart3 className="h-4 w-4" />
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
                        <p>Created: {new Date(stats.created_at).toLocaleDateString()}</p>
                      </div>
                    </CardDescription>
                  )}
                </CardHeader>
                <CardContent className="pt-0">
                  <Button asChild variant="outline" className="w-full">
                    <Link to={`/collections/${encodeURIComponent(collection)}`}>
                      Manage Records
                    </Link>
                  </Button>
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}
    </div>
  );
};

export default Collections; 