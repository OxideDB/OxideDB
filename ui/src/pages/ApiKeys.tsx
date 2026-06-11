import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  AlertTriangle,
  Copy,
  KeyRound,
  RefreshCw,
  Save,
  ShieldCheck,
  Unlock,
} from 'lucide-react';
import PageLayout from '@/components/PageLayout';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
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
  ApiKeyOperationType,
  ApiKeyRuleInfo,
  ApiKeyRulesResponse,
  AuthOperation,
  CrudOperation,
} from '@/types/api';

const CRUD_OPERATIONS: CrudOperation[] = ['create', 'read', 'update', 'delete', 'list'];
const AUTH_OPERATIONS: AuthOperation[] = [
  'login',
  'register',
  'token_validation',
  'token_refresh',
  'logout',
  'get_current_user',
  'list_auth_collections',
];

const formatOperation = (operation: string) => operation.replace(/_/g, ' ');

const generateApiKey = () => {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  const raw = Array.from(bytes, (byte) => String.fromCharCode(byte)).join('');
  return `ox_${btoa(raw).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '')}`;
};

const emptySummary: ApiKeyRulesResponse = {
  rules: [],
  collections: [],
  total_rules: 0,
  protected_collections: 0,
  exact_key_rules: 0,
  hashed_key_rules: 0,
  wildcard_key_rules: 0,
};

const ApiKeys: React.FC = () => {
  const [summary, setSummary] = useState<ApiKeyRulesResponse>(emptySummary);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedCollection, setSelectedCollection] = useState('');
  const [selectedOperationType, setSelectedOperationType] = useState<ApiKeyOperationType>('crud');
  const [selectedCrudOperation, setSelectedCrudOperation] = useState<CrudOperation>('list');
  const [selectedAuthOperation, setSelectedAuthOperation] = useState<AuthOperation>('login');
  const [apiKey, setApiKey] = useState(generateApiKey);

  const loadRules = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const response = await apiService.getApiKeyRules();
      const sorted = {
        ...response,
        collections: [...response.collections].sort((a, b) => a.name.localeCompare(b.name)),
        rules: [...response.rules].sort((a, b) => (
          `${a.collection}-${a.operation_type}-${a.operation}`
            .localeCompare(`${b.collection}-${b.operation_type}-${b.operation}`)
        )),
      };

      setSummary(sorted);
      setSelectedCollection((current) => (
        current && sorted.collections.some((collection) => collection.name === current)
          ? current
          : sorted.collections[0]?.name || ''
      ));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load API key rules');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadRules();
  }, [loadRules]);

  const selectedCollectionInfo = useMemo(
    () => summary.collections.find((info) => info.name === selectedCollection),
    [summary.collections, selectedCollection],
  );

  useEffect(() => {
    if (selectedCollectionInfo?.collection_type !== 'auth' && selectedOperationType === 'auth') {
      setSelectedOperationType('crud');
    }
  }, [selectedCollectionInfo?.collection_type, selectedOperationType]);

  const selectedOperation = selectedOperationType === 'crud'
    ? selectedCrudOperation
    : selectedAuthOperation;

  const applyApiKeyRule = async () => {
    if (!selectedCollectionInfo) return;

    const trimmedKey = apiKey.trim();
    if (!trimmedKey) {
      toast({
        title: 'API key required',
        description: 'Generate or enter a key before saving the rule.',
        variant: 'destructive',
      });
      return;
    }

    try {
      setSaving(true);
      const response = await apiService.upsertApiKeyRule({
        collection: selectedCollectionInfo.name,
        operation_type: selectedOperationType,
        operation: selectedOperation,
        key: trimmedKey,
      });

      setSummary(response.summary);
      setApiKey(response.api_key);
      await copyHeader(response.api_key);
      toast({
        title: 'API key rule saved',
        description: `${selectedCollectionInfo.name} ${formatOperation(selectedOperation)} now requires x-api-key. The key was copied and will not be recoverable later.`,
      });
    } catch (err) {
      toast({
        title: 'Rule save failed',
        description: err instanceof Error ? err.message : 'Failed to save API key rule',
        variant: 'destructive',
      });
    } finally {
      setSaving(false);
    }
  };

  const relaxRule = async (rule: ApiKeyRuleInfo) => {
    try {
      setSaving(true);
      const response = await apiService.revokeApiKeyRule({
        collection: rule.collection,
        operation_type: rule.operation_type,
        operation: rule.operation,
        fallback_permission: 'authenticatedonly',
      });

      setSummary(response);
      toast({
        title: 'Rule updated',
        description: `${rule.collection} ${formatOperation(rule.operation)} now requires a signed-in user.`,
      });
    } catch (err) {
      toast({
        title: 'Rule update failed',
        description: err instanceof Error ? err.message : 'Failed to update API key rule',
        variant: 'destructive',
      });
    } finally {
      setSaving(false);
    }
  };

  const copyHeader = async (key: string) => {
    try {
      await navigator.clipboard.writeText(`x-api-key: ${key}`);
      toast({
        title: 'Header copied',
        description: 'The x-api-key header is ready to paste into a client request.',
      });
    } catch (err) {
      toast({
        title: 'Copy failed',
        description: err instanceof Error ? err.message : 'Could not copy the header.',
        variant: 'destructive',
      });
    }
  };

  const headerActions = (
    <Button onClick={loadRules} disabled={loading || saving} variant="outline">
      <RefreshCw className={`mr-2 h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
      Refresh
    </Button>
  );

  return (
    <PageLayout
      title="API Keys"
      description="Manage header-based access rules"
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
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Header Rules</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{summary.total_rules}</div>
            <p className="text-xs text-muted-foreground">Operations gated by x-api-key</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Collections</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{summary.protected_collections}</div>
            <p className="text-xs text-muted-foreground">Collections with key rules</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium">Hashed Keys</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{summary.hashed_key_rules}</div>
            <p className="text-xs text-muted-foreground">{summary.exact_key_rules} legacy plaintext rules</p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <KeyRound className="h-5 w-5" />
            Apply API Key Rule
          </CardTitle>
          <CardDescription>
            Bind a generated key to one collection operation.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 lg:grid-cols-[minmax(180px,1fr)_150px_minmax(150px,1fr)_minmax(240px,1.4fr)_auto]">
          <div className="space-y-2">
            <Label>Collection</Label>
            <Select
              value={selectedCollection}
              onValueChange={setSelectedCollection}
              disabled={summary.collections.length === 0 || loading}
            >
              <SelectTrigger>
                <SelectValue placeholder="Select collection" />
              </SelectTrigger>
              <SelectContent>
                {summary.collections.map((info) => (
                  <SelectItem key={info.name} value={info.name}>
                    {info.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <Label>Type</Label>
            <Select value={selectedOperationType} onValueChange={(value) => setSelectedOperationType(value as ApiKeyOperationType)}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="crud">CRUD</SelectItem>
                {selectedCollectionInfo?.collection_type === 'auth' && (
                  <SelectItem value="auth">Auth</SelectItem>
                )}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <Label>Operation</Label>
            {selectedOperationType === 'crud' ? (
              <Select value={selectedCrudOperation} onValueChange={(value) => setSelectedCrudOperation(value as CrudOperation)}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {CRUD_OPERATIONS.map((operation) => (
                    <SelectItem key={operation} value={operation}>
                      {formatOperation(operation)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            ) : (
              <Select value={selectedAuthOperation} onValueChange={(value) => setSelectedAuthOperation(value as AuthOperation)}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {AUTH_OPERATIONS.map((operation) => (
                    <SelectItem key={operation} value={operation}>
                      {formatOperation(operation)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          </div>

          <div className="space-y-2">
            <Label>Key Value</Label>
            <div className="flex gap-2">
              <Input value={apiKey} onChange={(event) => setApiKey(event.target.value)} className="font-mono" />
              <Button type="button" variant="outline" size="icon" onClick={() => setApiKey(generateApiKey())} title="Generate key">
                <RefreshCw className="h-4 w-4" />
              </Button>
            </div>
          </div>

          <div className="flex items-end">
            <Button onClick={applyApiKeyRule} disabled={saving || loading || !selectedCollectionInfo} className="w-full">
              <Save className="mr-2 h-4 w-4" />
              Save Rule
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <ShieldCheck className="h-5 w-5" />
            Active API Key Rules
          </CardTitle>
          <CardDescription>Operations currently using x-api-key rules.</CardDescription>
        </CardHeader>
        <CardContent>
          {loading ? (
            <div className="flex h-40 items-center justify-center text-sm text-muted-foreground">
              Loading API key rules...
            </div>
          ) : summary.rules.length === 0 ? (
            <div className="flex flex-col items-center justify-center rounded-lg border border-dashed py-12 text-center">
              <KeyRound className="mb-3 h-10 w-10 text-muted-foreground" />
              <h3 className="font-semibold">No API key rules</h3>
              <p className="mt-1 text-sm text-muted-foreground">Create a rule above to require an x-api-key header.</p>
            </div>
          ) : (
            <div className="overflow-x-auto rounded-md border">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Collection</TableHead>
                    <TableHead>Operation</TableHead>
                    <TableHead>Key</TableHead>
                    <TableHead>Rule</TableHead>
                    <TableHead className="text-right">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {summary.rules.map((rule) => (
                    <TableRow key={`${rule.collection}-${rule.operation_type}-${rule.operation}`}>
                      <TableCell>
                        <div className="font-medium">{rule.collection}</div>
                        <div className="text-xs text-muted-foreground">{rule.collection_type}</div>
                      </TableCell>
                      <TableCell>
                        <div className="flex flex-wrap gap-2">
                          <Badge variant="outline">{rule.operation_type.toUpperCase()}</Badge>
                          <Badge variant="secondary">{formatOperation(rule.operation)}</Badge>
                        </div>
                      </TableCell>
                      <TableCell className="font-mono text-sm">
                        <div>{rule.key_preview || 'any non-empty key'}</div>
                        {rule.hashed && (
                          <div className="text-xs text-muted-foreground">stored as hash</div>
                        )}
                        {rule.legacy_plaintext && (
                          <div className="text-xs text-destructive">legacy plaintext</div>
                        )}
                      </TableCell>
                      <TableCell>
                        <code className="break-all rounded bg-muted px-2 py-1 text-xs">{rule.rule}</code>
                      </TableCell>
                      <TableCell>
                        <div className="flex justify-end gap-2">
                          <Button
                            variant="outline"
                            size="icon"
                            disabled={!rule.exact_key}
                            onClick={() => rule.exact_key && copyHeader(rule.exact_key)}
                            title={rule.exact_key ? 'Copy x-api-key header' : 'Stored keys are not recoverable'}
                          >
                            <Copy className="h-4 w-4" />
                          </Button>
                          <Button
                            variant="outline"
                            size="icon"
                            onClick={() => relaxRule(rule)}
                            disabled={saving}
                            title="Require signed-in users instead"
                          >
                            <Unlock className="h-4 w-4" />
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}
        </CardContent>
      </Card>
    </PageLayout>
  );
};

export default ApiKeys;
