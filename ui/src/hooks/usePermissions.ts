import { useState, useEffect, useCallback } from 'react';
import { apiService } from '../services/api';
import type { CollectionPermissions, PermissionPresetType } from '../types/api';

interface UsePermissionsReturn {
  permissions: CollectionPermissions | null;
  editingPermissions: CollectionPermissions | null;
  setEditingPermissions: (permissions: CollectionPermissions | null) => void;
  updatePermissions: (permissions: CollectionPermissions) => Promise<void>;
  resetPermissions: () => Promise<void>;
  applyPreset: (preset: PermissionPresetType) => Promise<void>;
}

/**
 * Custom hook for managing collection permissions
 * Handles permissions fetching, editing, and updates
 */
export const usePermissions = (collection: string | undefined): UsePermissionsReturn => {
  const [permissions, setPermissions] = useState<CollectionPermissions | null>(null);
  const [editingPermissions, setEditingPermissions] = useState<CollectionPermissions | null>(null);

  const fetchPermissions = useCallback(async () => {
    if (!collection) return;

    try {
      const response = await apiService.getPermissions();
      const collectionPermissions = response.data.find(p => p.collection_name === collection)?.permissions || null;
      setPermissions(collectionPermissions);
    } catch (err) {
      console.error('Failed to fetch permissions:', err);
      // Don't set error state here as it's not critical for the main functionality
    }
  }, [collection]);

  const updatePermissions = useCallback(async (updatedPermissions: CollectionPermissions) => {
    if (!collection) return;
    
    try {
      await apiService.updateCollectionPermissions(collection, updatedPermissions);
      await fetchPermissions(); // Reload to get updated data
      setEditingPermissions(null);
    } catch (err) {
      console.error('Failed to update permissions:', err);
      throw err; // Re-throw to let the component handle the error
    }
  }, [collection, fetchPermissions]);

  const resetPermissions = useCallback(async () => {
    if (!collection) return;
    
    try {
      await apiService.resetCollectionPermissions(collection);
      await fetchPermissions();
    } catch (err) {
      console.error('Failed to reset permissions:', err);
      throw err;
    }
  }, [collection, fetchPermissions]);

  const applyPreset = useCallback(async (preset: PermissionPresetType) => {
    if (!collection) return;
    
    try {
      await apiService.applyPermissionPreset(collection, preset);
      await fetchPermissions();
    } catch (err) {
      console.error('Failed to apply preset:', err);
      throw err;
    }
  }, [collection, fetchPermissions]);

  useEffect(() => {
    if (collection) {
      fetchPermissions();
    }
  }, [fetchPermissions]);

  return {
    permissions,
    editingPermissions,
    setEditingPermissions,
    updatePermissions,
    resetPermissions,
    applyPreset,
  };
}; 