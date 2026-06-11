import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  AlertTriangle,
  Archive,
  CheckCircle2,
  Database,
  Download,
  FileJson,
  HardDrive,
  RefreshCw,
  RotateCcw,
  Upload,
} from 'lucide-react';
import PageLayout from '@/components/PageLayout';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Checkbox } from '@/components/ui/checkbox';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { toast } from '@/components/ui/use-toast';
import { apiService } from '@/services/api';
import type {
  BackupCollectionSummary,
  BackupExportResponse,
  BackupManifestResponse,
  BackupRestoreResponse,
} from '@/types/api';

const formatSize = (sizeKb: number) => {
  if (sizeKb >= 1024 * 1024) {
    return `${(sizeKb / 1024 / 1024).toFixed(2)} GB`;
  }

  if (sizeKb >= 1024) {
    return `${(sizeKb / 1024).toFixed(2)} MB`;
  }

  return `${sizeKb.toFixed(1)} KB`;
};

const formatDateTime = (value?: string) => {
  if (!value) return 'Not generated';

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(value));
};

const canExportCollection = (collection: BackupCollectionSummary) => (
  collection.excluded_reason !== 'system collection'
);

const backupFileName = (generatedAt: string) => {
  const stamp = new Date(generatedAt).toISOString().replace(/[:.]/g, '-');
  return `oxidedb-backup-${stamp}.json`;
};

const isBackupSnapshot = (value: unknown): value is BackupExportResponse => {
  if (!value || typeof value !== 'object') return false;

  const snapshot = value as BackupExportResponse;
  return typeof snapshot.format_version === 'number'
    && typeof snapshot.generated_at === 'string'
    && Array.isArray(snapshot.collections);
};

const restoreStatusLabel = (status: string) => (
  status.replace(/^would_/, '').replace(/_/g, ' ')
);

const restoreStatusVariant = (
  status: string,
): 'default' | 'secondary' | 'destructive' | 'outline' => {
  if (status.startsWith('skipped')) return 'secondary';
  if (status.includes('replace')) return 'destructive';
  return status.includes('merge') ? 'outline' : 'default';
};

const Backups: React.FC = () => {
  const [manifest, setManifest] = useState<BackupManifestResponse | null>(null);
  const [includeSystem, setIncludeSystem] = useState(false);
  const [selectedCollections, setSelectedCollections] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [restoreSnapshot, setRestoreSnapshot] = useState<BackupExportResponse | null>(null);
  const [restoreFileName, setRestoreFileName] = useState<string | null>(null);
  const [restoreIncludeSystem, setRestoreIncludeSystem] = useState(false);
  const [replaceExisting, setReplaceExisting] = useState(false);
  const [restorePreview, setRestorePreview] = useState<BackupRestoreResponse | null>(null);
  const [restoreLoading, setRestoreLoading] = useState(false);
  const [restoreOperation, setRestoreOperation] = useState<'preview' | 'restore' | null>(null);
  const [restoreError, setRestoreError] = useState<string | null>(null);
  const restoreFileInputRef = useRef<HTMLInputElement | null>(null);

  const backupOptions = useMemo(() => ({
    include_system: includeSystem,
    collections: selectedCollections.length ? selectedCollections : undefined,
  }), [includeSystem, selectedCollections]);

  const restoreRequestOptions = useMemo(() => ({
    include_system: restoreIncludeSystem,
    replace_existing: replaceExisting,
  }), [restoreIncludeSystem, replaceExisting]);

  const selectedSet = useMemo(
    () => new Set(selectedCollections),
    [selectedCollections],
  );

  const loadManifest = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const response = await apiService.getBackupManifest(backupOptions);
      setManifest(response);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load backup manifest');
    } finally {
      setLoading(false);
    }
  }, [backupOptions]);

  useEffect(() => {
    loadManifest();
  }, [loadManifest]);

  const eligibleNames = useMemo(() => (
    manifest?.collections
      .filter(canExportCollection)
      .map((collection) => collection.name) || []
  ), [manifest]);

  const isCollectionChecked = (collection: BackupCollectionSummary) => {
    if (!canExportCollection(collection)) return false;
    return selectedCollections.length === 0
      ? collection.included
      : selectedSet.has(collection.name);
  };

  const toggleCollection = (collection: BackupCollectionSummary, checked: boolean) => {
    if (!manifest || !canExportCollection(collection)) return;

    const baseSelection = selectedCollections.length
      ? new Set(selectedCollections.filter((name) => eligibleNames.includes(name)))
      : new Set(eligibleNames);

    if (checked) {
      baseSelection.add(collection.name);
    } else {
      baseSelection.delete(collection.name);
    }

    const nextSelection = eligibleNames.filter((name) => baseSelection.has(name));
    if (nextSelection.length === 0) {
      toast({
        title: 'Selection required',
        description: 'Keep at least one collection in the export.',
        variant: 'destructive',
      });
      return;
    }

    setSelectedCollections(
      nextSelection.length === eligibleNames.length ? [] : nextSelection,
    );
  };

  const selectAllEligible = () => {
    setSelectedCollections([]);
  };

  const downloadBackup = async () => {
    try {
      setExporting(true);
      const blob = await apiService.downloadBackup(backupOptions);
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = backupFileName(new Date().toISOString());
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);

      toast({
        title: 'Backup exported',
        description: manifest
          ? `${manifest.included_collections} collections and ${manifest.total_records.toLocaleString()} records downloaded.`
          : `Backup downloaded (${formatSize(blob.size / 1024)}).`,
      });
    } catch (err) {
      toast({
        title: 'Backup export failed',
        description: err instanceof Error ? err.message : 'Failed to export backup',
        variant: 'destructive',
      });
    } finally {
      setExporting(false);
    }
  };

  const handleRestoreFileChange = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    try {
      setRestoreError(null);
      setRestorePreview(null);

      const parsed = JSON.parse(await file.text()) as unknown;
      if (!isBackupSnapshot(parsed)) {
        throw new Error('The selected file is not an OxideDB backup snapshot.');
      }

      setRestoreSnapshot(parsed);
      setRestoreFileName(file.name);

      toast({
        title: 'Backup file loaded',
        description: `${parsed.total_collections.toLocaleString()} collections and ${parsed.total_records.toLocaleString()} records found.`,
      });
    } catch (err) {
      setRestoreSnapshot(null);
      setRestoreFileName(null);
      setRestoreError(err instanceof Error ? err.message : 'Failed to read backup file');
      toast({
        title: 'Backup file rejected',
        description: err instanceof Error ? err.message : 'Failed to read backup file',
        variant: 'destructive',
      });
    } finally {
      event.target.value = '';
    }
  };

  const previewRestore = async () => {
    if (!restoreSnapshot) return;

    try {
      setRestoreLoading(true);
      setRestoreOperation('preview');
      setRestoreError(null);
      const response = await apiService.restoreBackup({
        snapshot: restoreSnapshot,
        dry_run: true,
        ...restoreRequestOptions,
      });
      setRestorePreview(response);
      toast({
        title: 'Restore preview ready',
        description: `${response.created_records.toLocaleString()} records to create, ${response.skipped_records.toLocaleString()} to skip.`,
      });
    } catch (err) {
      setRestoreError(err instanceof Error ? err.message : 'Failed to preview restore');
      toast({
        title: 'Restore preview failed',
        description: err instanceof Error ? err.message : 'Failed to preview restore',
        variant: 'destructive',
      });
    } finally {
      setRestoreLoading(false);
      setRestoreOperation(null);
    }
  };

  const runRestore = async () => {
    if (!restoreSnapshot) return;

    try {
      setRestoreLoading(true);
      setRestoreOperation('restore');
      setRestoreError(null);
      const response = await apiService.restoreBackup({
        snapshot: restoreSnapshot,
        dry_run: false,
        ...restoreRequestOptions,
      });
      setRestorePreview(response);
      await loadManifest();
      toast({
        title: 'Backup restored',
        description: `${response.created_records.toLocaleString()} records restored across ${response.collections.length.toLocaleString()} collections.`,
      });
    } catch (err) {
      setRestoreError(err instanceof Error ? err.message : 'Failed to restore backup');
      toast({
        title: 'Restore failed',
        description: err instanceof Error ? err.message : 'Failed to restore backup',
        variant: 'destructive',
      });
    } finally {
      setRestoreLoading(false);
      setRestoreOperation(null);
    }
  };

  const updateRestoreIncludeSystem = (checked: boolean) => {
    setRestoreIncludeSystem(checked);
    setRestorePreview(null);
  };

  const updateReplaceExisting = (checked: boolean) => {
    setReplaceExisting(checked);
    setRestorePreview(null);
  };

  const clearRestoreSnapshot = () => {
    setRestoreSnapshot(null);
    setRestoreFileName(null);
    setRestorePreview(null);
    setRestoreError(null);
  };

  const headerActions = (
    <>
      <Button onClick={loadManifest} disabled={loading || exporting} variant="outline">
        <RefreshCw className={`mr-2 h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
        Refresh
      </Button>
      <Button
        onClick={downloadBackup}
        disabled={loading || exporting || !manifest || manifest.included_collections === 0}
      >
        <Download className="mr-2 h-4 w-4" />
        {exporting ? 'Exporting...' : 'Download JSON'}
      </Button>
    </>
  );

  return (
    <PageLayout
      title="Backups"
      description="Preview and export database snapshots"
      headerActions={headerActions}
    >
      {error && (
        <Alert variant="destructive">
          <AlertTriangle className="h-4 w-4" />
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Collections</CardTitle>
            <Database className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {manifest?.included_collections.toLocaleString() || 0}
            </div>
            <p className="text-xs text-muted-foreground">
              of {manifest?.total_collections.toLocaleString() || 0} available
            </p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Records</CardTitle>
            <Archive className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {manifest?.total_records.toLocaleString() || 0}
            </div>
            <p className="text-xs text-muted-foreground">included in export</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Estimated Size</CardTitle>
            <HardDrive className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {formatSize(manifest?.total_size_kb || 0)}
            </div>
            <p className="text-xs text-muted-foreground">
              generated {formatDateTime(manifest?.generated_at)}
            </p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
          <div>
            <CardTitle>Snapshot Contents</CardTitle>
            <p className="mt-1 text-sm text-muted-foreground">
              {selectedCollections.length
                ? `${selectedCollections.length} collections selected`
                : 'All eligible collections selected'}
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-3">
            <div className="flex items-center gap-2">
              <Switch
                id="include-system"
                checked={includeSystem}
                onCheckedChange={setIncludeSystem}
                disabled={loading || exporting}
              />
              <Label htmlFor="include-system" className="cursor-pointer text-sm">
                Include system
              </Label>
            </div>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={selectAllEligible}
              disabled={loading || exporting || selectedCollections.length === 0}
            >
              <CheckCircle2 className="mr-2 h-4 w-4" />
              Select All
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {loading && !manifest ? (
            <div className="flex h-48 items-center justify-center text-sm text-muted-foreground">
              Loading backup manifest...
            </div>
          ) : manifest?.collections.length === 0 ? (
            <div className="flex h-48 items-center justify-center text-sm text-muted-foreground">
              No collections found.
            </div>
          ) : (
            <div className="overflow-x-auto rounded-md border">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead className="w-12"></TableHead>
                    <TableHead>Collection</TableHead>
                    <TableHead>Type</TableHead>
                    <TableHead>Schema</TableHead>
                    <TableHead className="text-right">Records</TableHead>
                    <TableHead className="text-right">Size</TableHead>
                    <TableHead>Status</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {manifest?.collections.map((collection) => {
                    const exportable = canExportCollection(collection);
                    const checked = isCollectionChecked(collection);

                    return (
                      <TableRow key={collection.name}>
                        <TableCell>
                          <Checkbox
                            checked={checked}
                            disabled={!exportable || loading || exporting}
                            onCheckedChange={(value) => toggleCollection(collection, value === true)}
                            aria-label={`Include ${collection.name}`}
                          />
                        </TableCell>
                        <TableCell>
                          <div className="font-medium">{collection.name}</div>
                        </TableCell>
                        <TableCell>
                          <Badge variant={collection.collection_type === 'auth' ? 'secondary' : 'outline'}>
                            {collection.collection_type}
                          </Badge>
                        </TableCell>
                        <TableCell>v{collection.schema_version}</TableCell>
                        <TableCell className="text-right">
                          {collection.record_count.toLocaleString()}
                        </TableCell>
                        <TableCell className="text-right">
                          {formatSize(collection.size_kb)}
                        </TableCell>
                        <TableCell>
                          {collection.included ? (
                            <Badge>Included</Badge>
                          ) : (
                            <Badge variant="secondary">
                              {collection.excluded_reason || 'Excluded'}
                            </Badge>
                          )}
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
          <div>
            <CardTitle>Restore Snapshot</CardTitle>
            <p className="mt-1 text-sm text-muted-foreground">
              {restoreFileName || 'No backup file selected'}
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <input
              ref={restoreFileInputRef}
              type="file"
              accept="application/json,.json"
              className="hidden"
              onChange={handleRestoreFileChange}
            />
            <Button
              type="button"
              variant="outline"
              onClick={() => restoreFileInputRef.current?.click()}
              disabled={restoreLoading}
            >
              <Upload className="mr-2 h-4 w-4" />
              Choose JSON
            </Button>
            {restoreSnapshot && (
              <Button
                type="button"
                variant="outline"
                onClick={clearRestoreSnapshot}
                disabled={restoreLoading}
              >
                Clear
              </Button>
            )}
            <Button
              type="button"
              variant="outline"
              onClick={previewRestore}
              disabled={!restoreSnapshot || restoreLoading}
            >
              <FileJson className="mr-2 h-4 w-4" />
              {restoreOperation === 'preview' ? 'Previewing...' : 'Preview'}
            </Button>
            <Button
              type="button"
              variant={replaceExisting ? 'destructive' : 'default'}
              onClick={runRestore}
              disabled={!restoreSnapshot || !restorePreview?.dry_run || restoreLoading}
            >
              <RotateCcw className="mr-2 h-4 w-4" />
              {restoreOperation === 'restore' ? 'Restoring...' : 'Restore'}
            </Button>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          {restoreError && (
            <Alert variant="destructive">
              <AlertTriangle className="h-4 w-4" />
              <AlertDescription>{restoreError}</AlertDescription>
            </Alert>
          )}

          <div className="flex flex-wrap items-center gap-4">
            <div className="flex items-center gap-2">
              <Switch
                id="restore-include-system"
                checked={restoreIncludeSystem}
                onCheckedChange={updateRestoreIncludeSystem}
                disabled={restoreLoading}
              />
              <Label htmlFor="restore-include-system" className="cursor-pointer text-sm">
                Include system
              </Label>
            </div>
            <div className="flex items-center gap-2">
              <Switch
                id="replace-existing"
                checked={replaceExisting}
                onCheckedChange={updateReplaceExisting}
                disabled={restoreLoading}
              />
              <Label htmlFor="replace-existing" className="cursor-pointer text-sm">
                Replace existing
              </Label>
            </div>
          </div>

          {replaceExisting && (
            <Alert variant="destructive">
              <AlertTriangle className="h-4 w-4" />
              <AlertDescription>
                Existing collections with matching names will be deleted and recreated from the backup.
              </AlertDescription>
            </Alert>
          )}

          {restoreSnapshot && (
            <div className="grid gap-4 rounded-md border p-4 md:grid-cols-3">
              <div>
                <div className="text-sm font-medium">Backup Date</div>
                <div className="mt-1 text-sm text-muted-foreground">
                  {formatDateTime(restoreSnapshot.generated_at)}
                </div>
              </div>
              <div>
                <div className="text-sm font-medium">Collections</div>
                <div className="mt-1 text-sm text-muted-foreground">
                  {restoreSnapshot.total_collections.toLocaleString()}
                </div>
              </div>
              <div>
                <div className="text-sm font-medium">Records</div>
                <div className="mt-1 text-sm text-muted-foreground">
                  {restoreSnapshot.total_records.toLocaleString()}
                </div>
              </div>
            </div>
          )}

          {restorePreview && (
            <div className="space-y-4">
              <div className="grid gap-4 rounded-md border p-4 md:grid-cols-4">
                <div>
                  <div className="text-sm font-medium">
                    {restorePreview.dry_run ? 'Would Create' : 'Created'}
                  </div>
                  <div className="mt-1 text-sm text-muted-foreground">
                    {restorePreview.created_records.toLocaleString()} records
                  </div>
                </div>
                <div>
                  <div className="text-sm font-medium">
                    {restorePreview.dry_run ? 'Would Replace' : 'Replaced'}
                  </div>
                  <div className="mt-1 text-sm text-muted-foreground">
                    {restorePreview.replaced_collections.toLocaleString()} collections
                  </div>
                </div>
                <div>
                  <div className="text-sm font-medium">Skipped</div>
                  <div className="mt-1 text-sm text-muted-foreground">
                    {restorePreview.skipped_records.toLocaleString()} records
                  </div>
                </div>
                <div>
                  <div className="text-sm font-medium">Warnings</div>
                  <div className="mt-1 text-sm text-muted-foreground">
                    {restorePreview.warnings.length.toLocaleString()}
                  </div>
                </div>
              </div>

              {restorePreview.warnings.length > 0 && (
                <Alert>
                  <AlertTriangle className="h-4 w-4" />
                  <AlertDescription>
                    {restorePreview.warnings.slice(0, 3).join(' ')}
                    {restorePreview.warnings.length > 3 ? ' Additional warnings are listed by collection.' : ''}
                  </AlertDescription>
                </Alert>
              )}

              <div className="overflow-x-auto rounded-md border">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Collection</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead className="text-right">Source</TableHead>
                      <TableHead className="text-right">
                        {restorePreview.dry_run ? 'Create' : 'Created'}
                      </TableHead>
                      <TableHead className="text-right">Skipped</TableHead>
                      <TableHead>Warnings</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {restorePreview.collections.map((collection) => (
                      <TableRow key={collection.name}>
                        <TableCell>
                          <div className="font-medium">{collection.name}</div>
                          <div className="text-xs text-muted-foreground">
                            {collection.collection_type}
                          </div>
                        </TableCell>
                        <TableCell>
                          <Badge variant={restoreStatusVariant(collection.status)}>
                            {restoreStatusLabel(collection.status)}
                          </Badge>
                        </TableCell>
                        <TableCell className="text-right">
                          {collection.source_records.toLocaleString()}
                        </TableCell>
                        <TableCell className="text-right">
                          {collection.records_created.toLocaleString()}
                        </TableCell>
                        <TableCell className="text-right">
                          {collection.records_skipped.toLocaleString()}
                        </TableCell>
                        <TableCell className="max-w-sm text-sm text-muted-foreground">
                          {collection.warnings.join(' ') || 'None'}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </div>
            </div>
          )}
        </CardContent>
      </Card>
    </PageLayout>
  );
};

export default Backups;
