import { useState, useEffect, useCallback } from 'react';
import { apiService } from '../services/api';
import type { RecordQueryParams } from '../services/api';
import type { DbRecord, CollectionSchema, CollectionStats } from '../types/api';

interface UseRecordsDataReturn {
  records: DbRecord[];
  schema: CollectionSchema | null;
  stats: CollectionStats | null;
  /** Total record count for the current filter, when known. */
  totalCount: number | null;
  /** Whether more pages exist beyond the current offset. */
  hasMore: boolean;
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
  clearError: () => void;
}

/**
 * Custom hook for managing records data fetching and state
 * Handles loading states, error handling, and data refetching
 *
 * The record list is fetched with its pagination metadata preserved (total
 * count + has_more) so callers can render accurate page counts. Schema and
 * stats are fetched separately and only when the collection changes — they
 * are independent of record query params (limit/offset/sort/filter) and were
 * previously refetched on every pagination change.
 */
export const useRecordsData = (
  collection: string | undefined,
  recordParams?: RecordQueryParams
): UseRecordsDataReturn => {
  const [records, setRecords] = useState<DbRecord[]>([]);
  const [schema, setSchema] = useState<CollectionSchema | null>(null);
  const [stats, setStats] = useState<CollectionStats | null>(null);
  const [totalCount, setTotalCount] = useState<number | null>(null);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Fetch schema + stats only when the collection changes (not on every
  // record-param change like pagination/sort/filter).
  useEffect(() => {
    if (!collection) {
      setSchema(null);
      setStats(null);
      return;
    }
    let cancelled = false;
    Promise.all([
      apiService.getCollectionSchema(collection).catch(() => null),
      apiService.getCollectionStats(collection).catch(() => null),
    ]).then(([schemaData, statsData]) => {
      if (cancelled) return;
      setSchema(schemaData);
      setStats(statsData);
    });
    return () => {
      cancelled = true;
    };
  }, [collection]);

  // Fetch the record list whenever the collection or record params change.
  const fetchRecords = useCallback(async () => {
    if (!collection) return;

    try {
      setLoading(true);
      setError(null);

      const response = await apiService.getRecordsPaginated(collection, recordParams);
      setRecords(response.data);
      setTotalCount(response.pagination.total ?? null);
      setHasMore(response.pagination.has_next);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to fetch data';
      setError(errorMessage);
      console.error('Failed to fetch records data:', err);
    } finally {
      setLoading(false);
    }
  }, [collection, recordParams]);

  const refetch = useCallback(async () => {
    await fetchRecords();
  }, [fetchRecords]);

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  useEffect(() => {
    if (collection) {
      fetchRecords();
    }
  }, [collection, fetchRecords]);

  return {
    records,
    schema,
    stats,
    totalCount,
    hasMore,
    loading,
    error,
    refetch,
    clearError,
  };
};
