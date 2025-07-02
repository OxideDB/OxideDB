import { useState, useEffect } from 'react';
import { apiService } from '@/services/api';
import type { 
  CollectionPermissionsInfo, 
  CollectionPermissions,
  PermissionPresetType
} from '@/types/api';

export const usePermissions = () => {
  const [permissionsData, setPermissionsData] = useState<CollectionPermissionsInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadPermissions = async () => {
    try {
      setLoading(true);
      setError(null);
      const response = await apiService.getPermissions();
      // Sort collections alphabetically by name for consistent ordering
      const sortedData = response.data.sort((a, b) => 
        a.collection_name.localeCompare(b.collection_name)
      );
      setPermissionsData(sortedData);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load permissions');
    } finally {
      setLoading(false);
    }
  };

  const updateCollectionPermissions = async (collection: string, permissions: CollectionPermissions) => {
    try {
      await apiService.updateCollectionPermissions(collection, permissions);
      await loadPermissions(); // Reload to get updated data
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update permissions');
      throw err;
    }
  };

  const resetPermissions = async (collection: string) => {
    try {
      await apiService.resetCollectionPermissions(collection);
      await loadPermissions();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to reset permissions');
      throw err;
    }
  };

  const applyPreset = async (collection: string, preset: PermissionPresetType) => {
    try {
      await apiService.applyPermissionPreset(collection, preset);
      await loadPermissions();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to apply preset');
      throw err;
    }
  };

  useEffect(() => {
    loadPermissions();
  }, []);

  return {
    permissionsData,
    loading,
    error,
    loadPermissions,
    updateCollectionPermissions,
    resetPermissions,
    applyPreset,
  };
};
