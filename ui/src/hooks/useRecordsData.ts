import { useState, useEffect, useCallback } from 'react';
import { apiService } from '../services/api';
import type { RecordQueryParams } from '../services/api';
import type { DbRecord, CollectionSchema, CollectionStats } from '../types/api';

interface UseRecordsDataReturn {
  records: DbRecord[];
  schema: CollectionSchema | null;
  stats: CollectionStats | null;
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
  clearError: () => void;
}

/**
 * Custom hook for managing records data fetching and state
 * Handles loading states, error handling, and data refetching
 */
export const useRecordsData = (
  collection: string | undefined,
  recordParams?: RecordQueryParams
): UseRecordsDataReturn => {
  const [records, setRecords] = useState<DbRecord[]>([]);
  const [schema, setSchema] = useState<CollectionSchema | null>(null);
  const [stats, setStats] = useState<CollectionStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    if (!collection) return;

    try {
      setLoading(true);
      setError(null);
      
      // Fetch records, schema, and stats in parallel
      const [recordsData, schemaData, statsData] = await Promise.all([
        apiService.getRecords(collection, recordParams),
        apiService.getCollectionSchema(collection).catch(() => null), // Don't fail if schema doesn't exist
        apiService.getCollectionStats(collection).catch(() => null), // Don't fail if stats don't exist
      ]);
      
      setRecords(recordsData);
      setSchema(schemaData);
      setStats(statsData);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to fetch data';
      setError(errorMessage);
      console.error('Failed to fetch records data:', err);
    } finally {
      setLoading(false);
    }
  }, [collection, recordParams]);

  const refetch = useCallback(async () => {
    await fetchData();
  }, [fetchData]);

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  useEffect(() => {
    if (collection) {
      fetchData();
    }
  }, [collection, fetchData]);

  return {
    records,
    schema,
    stats,
    loading,
    error,
    refetch,
    clearError,
  };
};
