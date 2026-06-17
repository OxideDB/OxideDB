import type {
  ApiError, CollectionStats, CollectionStatsEntry, CreateCollectionRequest, DbRecord, ApiHealthStatus,
  CollectionSchema, CollectionPermissionsInfo, CollectionPermissions, PermissionPresetType,
  ApiResponse, AuthResponse, User, TokenValidationResponse,
  LogQueryParams, AuditQueryParams, LogResponse, LogEntry, SecurityAuditEvent,
  DashboardMetrics, RetentionStats, CreateLogRequest, CreateAuditRequest,
  CreateLogResponse, LoggingHealthResponse,
  DashboardStats, SystemStats, FileMetadata, FileListRequest, FileMoveRequest,
  FileListResponse, VfsUsageStats,
  SiteSettings, SiteSettingsResponse, UpdateSiteSettingsRequest, SettingsHealthStatus,
  PublicSiteSettings,
  ApiKeyRulesResponse, UpsertApiKeyRuleRequest, UpsertApiKeyRuleResponse,
  RevokeApiKeyRuleRequest, BackupExportResponse, BackupManifestResponse,
  BackupQueryOptions, BackupRestoreRequest, BackupRestoreResponse,
  PluginAdminPage, PluginCapability, PluginRecordField
} from '../types/api';

// Plugin-related interfaces
interface PluginInfo {
  name: string;
  status: 'Enabled' | 'Disabled' | 'Error' | 'Loading' | 'Uninstalling';
  version: string;
  description: string;
  author: string;
  capabilities: PluginCapability[];
  trust_level: 'Untrusted' | 'PartiallyTrusted' | 'FullyTrusted' | 'System';
  routes: PluginRoute[];
  admin_pages: PluginAdminPage[];
  executions: number;
  errors: number;
  last_execution?: string;
  resource_usage: ResourceUsage;
}

interface PluginDetails extends PluginInfo {
  audit_log: AuditEntry[];
  permissions?: unknown;
}

interface PluginRoute {
  plugin_name: string;
  method: string;
  path: string;
  handler_function: string;
  permissions?: unknown;
  has_custom_permissions: boolean;
}

interface ResourceUsage {
  memory_bytes: number;
  cpu_time_ms: number;
  api_calls: number;
  storage_bytes: number;
}

interface AuditEntry {
  timestamp: string;
  event_type: string;
  description: string;
  metadata?: unknown;
}

interface PluginAnalysisResult {
  is_valid: boolean;
  plugin_info?: PluginInfo;
  declared_capabilities: string[];
  recommended_trust_level?: string;
  security_info: PluginSecurityInfo;
  size_bytes: number;
  warnings: string[];
  errors: string[];
}

interface PluginSecurityInfo {
  binary_hash: string;
  hash_algorithm: string;
  signature_valid: boolean;
  security_advisories: string[];
  audit_info?: {
    audit_date: string;
    auditor: string;
    report_url?: string;
    status: string;
  };
}

interface PluginInstallNotice {
  severity: 'Info' | 'Warning';
  message: string;
}

interface PluginInstallResult {
  plugin_info: PluginInfo;
  applied_trust_level: 'Untrusted' | 'PartiallyTrusted' | 'FullyTrusted' | 'System';
  applied_capabilities: PluginCapability[];
  declared_capabilities: string[];
  security_info: PluginSecurityInfo;
  size_bytes: number;
  notices: PluginInstallNotice[];
}

// Import PaginatedResponse from generated bindings
interface PaginatedResponse<T> {
  data: T[];
  pagination: {
    page: number;
    per_page: number;
    total: number;
    total_pages: number;
    has_next: boolean;
    has_prev: boolean;
  };
  meta?: unknown;
  success: boolean;
}

// Refresh token response type
interface RefreshTokenResponse {
  access_token?: string;
  refresh_token?: string;
  expires_in: number;
  refresh_expires_in: number;
}

export type RecordFilterOp =
  | 'eq'
  | 'ne'
  | 'contains'
  | 'exists'
  | 'not_exists'
  | 'gt'
  | 'gte'
  | 'lt'
  | 'lte';

export interface RecordQueryParams {
  limit?: number;
  offset?: number;
  sort_field?: string;
  sort_ascending?: boolean;
  populate_relationships?: boolean;
  populate_fields?: string;
  filter_field?: string;
  filter_op?: RecordFilterOp;
  filter_value?: string;
  search?: string;
  include_total?: boolean;
}

class ApiService {
  private baseUrl: string;
  private token: string | null = null;
  private refreshToken: string | null = null;
  private cookieSessionActive: boolean = false;
  private accessTokenExpiresAt: number | null = null;
  private isRefreshing: boolean = false;
  private refreshPromise: Promise<string | null> | null = null;

  constructor(baseUrl?: string) {
    // Auto-detect base URL based on environment
    if (baseUrl) {
      this.baseUrl = baseUrl;
    } else if (import.meta.env.PROD) {
      // Running in production, served from the same origin
      this.baseUrl = window.location.origin;
    } else {
      // Development mode
      this.baseUrl = 'http://localhost:8080';
    }
    
    this.clearPersistedAuthTokens();
  }

  private clearPersistedAuthTokens() {
    localStorage.removeItem('auth_token');
    localStorage.removeItem('refresh_token');
    sessionStorage.removeItem('auth_token');
    sessionStorage.removeItem('refresh_token');
  }

  private async request<T>(
    endpoint: string,
    options: RequestInit = {}
  ): Promise<T> {
    const url = `${this.baseUrl}${endpoint}`;
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      ...(options.headers as Record<string, string>),
    };

    if (this.shouldRefreshBeforeRequest(endpoint)) {
      await this.refreshIfAccessTokenExpiring();
    }

    // Add authorization header if we have a token
    if (this.token) {
      headers['Authorization'] = `Bearer ${this.token}`;
    }

    let response = await fetch(url, {
      ...options,
      headers,
      credentials: 'include',
    });

    // If we get a 401, try cookie/bearer refresh once before failing.
    // Concurrent requests share the same refresh promise and retry after it settles.
    if (response.status === 401 && this.shouldRefreshAfterUnauthorized(endpoint)) {
      try {
        const newAccessToken = await this.performTokenRefresh();
        
        // Retry the original request with the new token
        if (newAccessToken) {
          headers['Authorization'] = `Bearer ${newAccessToken}`;
        } else {
          delete headers.Authorization;
        }
        response = await fetch(url, {
          ...options,
          headers,
          credentials: 'include',
        });
      } catch (refreshError) {
        console.error('Token refresh failed:', refreshError);
        this.clearTokens();
        throw new Error('Session expired. Please log in again.');
      }
    }

    if (!response.ok) {
      let errorMessage = `HTTP ${response.status}`;
      try {
        const errorData: ApiError = await response.json();
        errorMessage = errorData.message || errorData.error || errorMessage;
      } catch {
        // If parsing JSON fails, use the status text
        errorMessage = response.statusText || errorMessage;
      }
      throw new Error(errorMessage);
    }

    // Handle empty responses (like 204 No Content)
    if (response.status === 204 || response.headers.get('content-length') === '0') {
      return {} as T;
    }

    return response.json();
  }

  private async fetchErrorMessage(response: Response): Promise<string> {
    const fallback = response.statusText || `HTTP ${response.status}`;
    const errorText = await response.text();

    if (!errorText) {
      return fallback;
    }

    try {
      const errorData: ApiError = JSON.parse(errorText);
      return errorData.message || errorData.error || fallback;
    } catch {
      return errorText;
    }
  }

  private shouldRefreshAfterUnauthorized(endpoint: string): boolean {
    const path = this.endpointPath(endpoint);

    if (
      path === '/health' ||
      path === '/settings/public' ||
      path === '/auth/refresh' ||
      path === '/auth/logout' ||
      path === '/auth/validate' ||
      path === '/auth/collections'
    ) {
      return false;
    }

    return !/^\/auth\/[^/]+\/(?:login|register)$/.test(path);
  }

  private shouldRefreshBeforeRequest(endpoint: string): boolean {
    return this.shouldRefreshAfterUnauthorized(endpoint) && this.hasRefreshToken();
  }

  private endpointPath(endpoint: string): string {
    return endpoint.split('?')[0];
  }

  private async refreshIfAccessTokenExpiring(): Promise<void> {
    if (!this.accessTokenExpiresAt) {
      return;
    }

    const refreshBufferMs = 30_000;
    if (Date.now() < this.accessTokenExpiresAt - refreshBufferMs) {
      return;
    }

    await this.performTokenRefresh();
  }

  private setAccessTokenExpiry(expiresInSeconds?: number | null) {
    this.accessTokenExpiresAt = expiresInSeconds
      ? Date.now() + expiresInSeconds * 1000
      : null;
  }

  private setAccessTokenExpiryFromUnix(expiresAtSeconds?: number | null) {
    this.accessTokenExpiresAt = expiresAtSeconds ? expiresAtSeconds * 1000 : null;
  }

  private async performTokenRefresh(): Promise<string | null> {
    // If already refreshing, wait for the existing refresh
    if (this.isRefreshing && this.refreshPromise) {
      return this.refreshPromise;
    }

    this.isRefreshing = true;
    this.refreshPromise = this.doTokenRefresh();

    try {
      const newToken = await this.refreshPromise;
      return newToken;
    } finally {
      this.isRefreshing = false;
      this.refreshPromise = null;
    }
  }

  private async doTokenRefresh(): Promise<string | null> {
    const response = await fetch(`${this.baseUrl}/auth/refresh`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      credentials: 'include',
      body: JSON.stringify({
        refresh_token: this.refreshToken || undefined,
        cookie_session: true,
      }),
    });

    if (!response.ok) {
      throw new Error('Token refresh failed');
    }

    const data: ApiResponse<RefreshTokenResponse> = await response.json();
    
    // Cookie sessions intentionally omit token material from the JSON body.
    this.token = data.data.access_token || null;
    this.refreshToken = data.data.refresh_token || null;
    this.cookieSessionActive = true;
    this.setAccessTokenExpiry(data.data.expires_in);

    return this.token;
  }

  // Generic HTTP methods
  async get<T>(endpoint: string): Promise<T> {
    return this.request<T>(endpoint, { method: 'GET' });
  }

  async post<T>(endpoint: string, data?: unknown): Promise<T> {
    return this.request<T>(endpoint, {
      method: 'POST',
      body: data ? JSON.stringify(data) : undefined,
    });
  }

  async put<T>(endpoint: string, data?: unknown): Promise<T> {
    return this.request<T>(endpoint, {
      method: 'PUT',
      body: data ? JSON.stringify(data) : undefined,
    });
  }

  async patch<T>(endpoint: string, data?: unknown): Promise<T> {
    return this.request<T>(endpoint, {
      method: 'PATCH',
      body: data ? JSON.stringify(data) : undefined,
    });
  }

  async delete<T>(endpoint: string): Promise<T> {
    return this.request<T>(endpoint, { method: 'DELETE' });
  }

  resolveUrl(endpoint: string): string {
    return new URL(endpoint, this.baseUrl).toString();
  }

  // Auth methods
  setTokens(accessToken: string, refreshToken?: string, expiresIn?: number) {
    this.token = accessToken;
    this.cookieSessionActive = true;
    this.setAccessTokenExpiry(expiresIn);
    this.clearPersistedAuthTokens();
    
    if (refreshToken) {
      this.refreshToken = refreshToken;
    }
  }

  clearTokens() {
    this.token = null;
    this.refreshToken = null;
    this.cookieSessionActive = false;
    this.accessTokenExpiresAt = null;
    this.clearPersistedAuthTokens();
  }

  isAuthenticated(): boolean {
    return this.cookieSessionActive || !!this.token;
  }

  hasRefreshToken(): boolean {
    return this.cookieSessionActive || !!this.refreshToken;
  }

  // Get available auth collections
  async getAuthCollections(): Promise<{ collections: { name: string; identifier_field: string; registration_enabled: boolean; email_verification_required: boolean }[] }> {
    const response = await this.get<ApiResponse<{ collections: { name: string; identifier_field: string; registration_enabled: boolean; email_verification_required: boolean }[] }>>('/auth/collections');
    return response.data;
  }

  async login(collection: string, identifier: string, credential: string): Promise<AuthResponse> {
    const response = await this.post<ApiResponse<AuthResponse>>(`/auth/${encodeURIComponent(collection)}/login`, {
      identifier,
      credential,
      cookie_session: true,
    });
    
    if (response.data.token) {
      this.setTokens(
        response.data.token,
        response.data.refresh_token || undefined,
        response.data.expires_in
      );
    } else {
      this.token = null;
      this.refreshToken = null;
      this.cookieSessionActive = true;
      this.setAccessTokenExpiry(response.data.expires_in);
      this.clearPersistedAuthTokens();
    }
    return response.data;
  }

  async register(collection: string, identifier: string, credential: string, additionalData?: Record<string, unknown>): Promise<void> {
    await this.post<void>(`/auth/${encodeURIComponent(collection)}/register`, {
      identifier,
      credential,
      additional_data: additionalData,
    });
  }

  async logout(): Promise<void> {
    try {
      await this.post<void>('/auth/logout');
    } finally {
      // Always clear tokens, even if logout request fails
      this.clearTokens();
    }
  }

  async validateToken(): Promise<boolean> {
    try {
      if (!this.token) {
        await this.getCurrentUser();
        return true;
      }
      
      const response = await this.post<ApiResponse<TokenValidationResponse>>('/auth/validate', {
        token: this.token
      });
      
      return response.data.valid;
    } catch (error) {
      console.error('Token validation failed:', error);
      // Don't automatically clear tokens here - let the caller decide
      // The error might be network-related, not an auth failure
      return false;
    }
  }

  async getCurrentUser(): Promise<User> {
    const response = await this.get<ApiResponse<{
      user_id: string;
      email: string;
      role: string;
      auth_collection: string;
      expires_at: number;
      custom_claims?: unknown;
    }>>('/auth/me');
    this.cookieSessionActive = true;
    this.setAccessTokenExpiryFromUnix(response.data.expires_at);
    
    // Map the backend response to frontend User interface
    return {
      id: response.data.user_id,
      email: response.data.email,
      is_superuser: response.data.role === 'superuser',
      created_at: new Date().toISOString(), // We don't get this from backend, use current time
    };
  }

  // Manual token refresh (can be called by components)
  async refreshTokens(): Promise<boolean> {
    try {
      await this.performTokenRefresh();
      return true;
    } catch {
      this.clearTokens();
      return false;
    }
  }

  // Health check
  async getHealth(): Promise<ApiHealthStatus> {
    const response = await this.request<ApiResponse<ApiHealthStatus>>('/health');
    return response.data;
  }

  async getDashboardStats(): Promise<DashboardStats> {
    const response = await this.get<ApiResponse<DashboardStats>>('/dashboard/stats');
    return response.data;
  }

  async getSystemStats(): Promise<SystemStats> {
    const response = await this.get<ApiResponse<SystemStats>>('/dashboard/system');
    return response.data;
  }

  // Collection methods
  async getCollections(): Promise<CollectionSchema[]> {
    const response = await this.request<ApiResponse<CollectionSchema[]>>('/collections');
    return response.data;
  }

  async createCollection(schema: CreateCollectionRequest): Promise<void> {
    // Convert BigInt values to numbers for JSON serialization
    const serializedSchema = JSON.stringify(schema, (key, value) => {
      return typeof value === 'bigint' ? Number(value) : value;
    });
    
    return this.request<void>('/collections', {
      method: 'POST',
      body: serializedSchema,
    });
  }

  async deleteCollection(collection: string): Promise<void> {
    return this.request<void>(`/collections/${encodeURIComponent(collection)}`, {
      method: 'DELETE',
    });
  }

  // Helper method to check if a collection is a system collection
  isSystemCollection(schema: CollectionSchema): boolean {
    return schema.name.startsWith('_');
  }

  async getCollectionStats(collection: string): Promise<CollectionStats> {
    const response = await this.request<ApiResponse<CollectionStats>>(`/collections/${encodeURIComponent(collection)}/stats`);
    return response.data;
  }

  /**
   * Fetch statistics for all collections in a single request.
   *
   * Replaces the N+1 fan-out where the sidebar and collections page each
   * issued one `getCollectionStats` call per collection. Returns a map keyed
   * by collection name.
   */
  async getAllCollectionStats(): Promise<Record<string, CollectionStatsEntry>> {
    const response = await this.request<ApiResponse<Record<string, CollectionStatsEntry>>>('/collections/stats');
    return response.data;
  }

  async getCollectionSchema(collection: string): Promise<CollectionSchema> {
    const response = await this.request<ApiResponse<CollectionSchema>>(`/collections/${encodeURIComponent(collection)}/schema`);
    return response.data;
  }

  async updateCollectionSchema(collection: string, schema: CollectionSchema): Promise<void> {
    // Convert BigInt values to numbers for JSON serialization
    const serializedSchema = JSON.stringify(schema, (key, value) => {
      return typeof value === 'bigint' ? Number(value) : value;
    });
    
    return this.request<void>(`/collections/${encodeURIComponent(collection)}/schema`, {
      method: 'PUT',
      body: serializedSchema,
    });
  }

  // Record methods
  async getRecords(collection: string, params?: RecordQueryParams): Promise<DbRecord[]> {
    const response = await this.getRecordsPaginated(collection, {
      include_total: false,
      ...params,
    });
    return response.data;
  }

  /**
   * Fetch records with the full paginated response preserved.
   *
   * Unlike `getRecords` (which discards pagination metadata), this returns the
   * `total`, `has_next`, and page info so callers can render accurate page
   * counts and "load more" controls instead of guessing.
   */
  async getRecordsPaginated(collection: string, params?: RecordQueryParams): Promise<PaginatedResponse<DbRecord>> {
    const searchParams = new URLSearchParams();
    if (params?.limit !== undefined) searchParams.set('limit', params.limit.toString());
    if (params?.offset !== undefined) searchParams.set('offset', params.offset.toString());
    if (params?.sort_field) searchParams.set('sort_field', params.sort_field);
    if (params?.sort_ascending !== undefined) searchParams.set('sort_ascending', params.sort_ascending.toString());
    if (params?.populate_relationships !== undefined) searchParams.set('populate_relationships', params.populate_relationships.toString());
    if (params?.populate_fields) searchParams.set('populate_fields', params.populate_fields);
    if (params?.filter_field) searchParams.set('filter_field', params.filter_field);
    if (params?.filter_op) searchParams.set('filter_op', params.filter_op);
    if (params?.filter_value !== undefined) searchParams.set('filter_value', params.filter_value);
    if (params?.search) searchParams.set('search', params.search);
    if (params?.include_total !== undefined) searchParams.set('include_total', params.include_total.toString());

    const query = searchParams.toString();
    const endpoint = `/collections/${encodeURIComponent(collection)}/records${query ? `?${query}` : ''}`;

    // Note: list_records returns PaginatedResponse, not ApiResponse
    const response = await this.request<PaginatedResponse<DbRecord>>(endpoint);
    return response;
  }

  async createRecord(collection: string, data: Record<string, unknown>): Promise<DbRecord> {
    const response = await this.request<ApiResponse<DbRecord>>(`/collections/${encodeURIComponent(collection)}/records`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
    return response.data;
  }

  async getRecord(collection: string, id: string): Promise<DbRecord> {
    const response = await this.request<ApiResponse<DbRecord>>(`/collections/${encodeURIComponent(collection)}/records/${encodeURIComponent(id)}`);
    return response.data;
  }

  async updateRecord(collection: string, id: string, data: Record<string, unknown>): Promise<DbRecord> {
    const response = await this.request<ApiResponse<DbRecord>>(`/collections/${encodeURIComponent(collection)}/records/${encodeURIComponent(id)}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
    return response.data;
  }

  async deleteRecord(collection: string, id: string): Promise<DbRecord> {
    const response = await this.request<ApiResponse<DbRecord>>(`/collections/${encodeURIComponent(collection)}/records/${encodeURIComponent(id)}`, {
      method: 'DELETE',
    });
    return response.data;
  }

  // Permissions methods
  async getPermissions(): Promise<ApiResponse<CollectionPermissionsInfo[]>> {
    return this.get<ApiResponse<CollectionPermissionsInfo[]>>('/permissions');
  }

  async updateCollectionPermissions(collection: string, permissions: CollectionPermissions): Promise<void> {
    return this.put<void>(`/collections/${encodeURIComponent(collection)}/permissions`, permissions);
  }

  async resetCollectionPermissions(collection: string): Promise<void> {
    return this.post<void>(`/collections/${encodeURIComponent(collection)}/permissions/reset`, {});
  }

  async applyPermissionPreset(collection: string, preset: PermissionPresetType): Promise<void> {
    return this.post<void>(`/collections/${encodeURIComponent(collection)}/permissions/preset`, preset);
  }

  // API key rule methods
  async getApiKeyRules(): Promise<ApiKeyRulesResponse> {
    const response = await this.get<ApiResponse<ApiKeyRulesResponse>>('/api/admin/api-keys');
    return response.data;
  }

  async upsertApiKeyRule(request: UpsertApiKeyRuleRequest): Promise<UpsertApiKeyRuleResponse> {
    const response = await this.post<ApiResponse<UpsertApiKeyRuleResponse>>('/api/admin/api-keys', request);
    return response.data;
  }

  async revokeApiKeyRule(request: RevokeApiKeyRuleRequest): Promise<ApiKeyRulesResponse> {
    const response = await this.post<ApiResponse<ApiKeyRulesResponse>>('/api/admin/api-keys/revoke', request);
    return response.data;
  }

  // Backup and export methods
  private buildBackupQuery(options?: BackupQueryOptions): string {
    const searchParams = new URLSearchParams();

    if (options?.include_system !== undefined) {
      searchParams.set('include_system', options.include_system.toString());
    }

    if (options?.include_vfs !== undefined) {
      searchParams.set('include_vfs', options.include_vfs.toString());
    }

    if (options?.collections?.length) {
      searchParams.set('collections', options.collections.join(','));
    }

    const query = searchParams.toString();
    return query ? `?${query}` : '';
  }

  async getBackupManifest(options?: BackupQueryOptions): Promise<BackupManifestResponse> {
    const response = await this.get<ApiResponse<BackupManifestResponse>>(
      `/api/admin/backups/manifest${this.buildBackupQuery(options)}`
    );
    return response.data;
  }

  async exportBackup(options?: BackupQueryOptions): Promise<BackupExportResponse> {
    const response = await this.get<ApiResponse<BackupExportResponse>>(
      `/api/admin/backups/export${this.buildBackupQuery(options)}`
    );
    return response.data;
  }

  async downloadBackup(options?: BackupQueryOptions): Promise<Blob> {
    const response = await fetch(
      `${this.baseUrl}/api/admin/backups/export/stream${this.buildBackupQuery(options)}`,
      {
        method: 'GET',
        headers: this.token ? { 'Authorization': `Bearer ${this.token}` } : {},
        credentials: 'include',
      }
    );

    if (!response.ok) {
      throw new Error(await this.fetchErrorMessage(response));
    }

    return response.blob();
  }

  async restoreBackup(request: BackupRestoreRequest): Promise<BackupRestoreResponse> {
    const response = await this.post<ApiResponse<BackupRestoreResponse>>(
      '/api/admin/backups/restore',
      request
    );
    return response.data;
  }

  // Logging API methods
  async getLogs(params?: LogQueryParams): Promise<LogResponse<LogEntry>> {
    const searchParams = new URLSearchParams();
    if (params?.level) searchParams.set('level', params.level);
    if (params?.start_time) searchParams.set('start_time', params.start_time);
    if (params?.end_time) searchParams.set('end_time', params.end_time);
    if (params?.correlation_id) searchParams.set('correlation_id', params.correlation_id);
    if (params?.module) searchParams.set('module', params.module);
    if (params?.user_id) searchParams.set('user_id', params.user_id);
    if (params?.collection) searchParams.set('collection', params.collection);
    if (params?.search) searchParams.set('search', params.search);
    if (params?.limit) searchParams.set('limit', params.limit.toString());
    if (params?.offset) searchParams.set('offset', params.offset.toString());
    if (params?.sort) searchParams.set('sort', params.sort);
    
    const query = searchParams.toString();
    const endpoint = `/logs${query ? `?${query}` : ''}`;
    
    const response = await this.get<ApiResponse<LogResponse<LogEntry>>>(endpoint);
    return response.data;
  }

  async getAuditEvents(params?: AuditQueryParams): Promise<LogResponse<SecurityAuditEvent>> {
    const searchParams = new URLSearchParams();
    if (params?.severity) searchParams.set('severity', params.severity);
    if (params?.start_time) searchParams.set('start_time', params.start_time);
    if (params?.end_time) searchParams.set('end_time', params.end_time);
    if (params?.event_type) searchParams.set('event_type', params.event_type);
    if (params?.actor) searchParams.set('actor', params.actor);
    if (params?.target) searchParams.set('target', params.target);
    if (params?.correlation_id) searchParams.set('correlation_id', params.correlation_id);
    if (params?.min_risk_score) searchParams.set('min_risk_score', params.min_risk_score.toString());
    if (params?.limit) searchParams.set('limit', params.limit.toString());
    if (params?.offset) searchParams.set('offset', params.offset.toString());
    if (params?.sort) searchParams.set('sort', params.sort);
    
    const query = searchParams.toString();
    const endpoint = `/logs/audit${query ? `?${query}` : ''}`;
    
    const response = await this.get<ApiResponse<LogResponse<SecurityAuditEvent>>>(endpoint);
    return response.data;
  }

  async getDashboardMetrics(): Promise<DashboardMetrics> {
    const response = await this.get<ApiResponse<DashboardMetrics>>('/logs/dashboard');
    return response.data;
  }

  async getRecentLogs(limit?: number): Promise<LogEntry[]> {
    const searchParams = new URLSearchParams();
    if (limit) searchParams.set('limit', limit.toString());
    
    const query = searchParams.toString();
    const endpoint = `/logs/recent${query ? `?${query}` : ''}`;
    
    const response = await this.get<ApiResponse<LogEntry[]>>(endpoint);
    return response.data;
  }

  async getRetentionStats(): Promise<RetentionStats | null> {
    const response = await this.get<ApiResponse<RetentionStats | null>>('/logs/retention');
    return response.data;
  }

  async getLogsByCorrelation(correlationId: string): Promise<LogResponse<LogEntry>> {
    const response = await this.get<ApiResponse<LogResponse<LogEntry>>>(`/logs/correlation/${encodeURIComponent(correlationId)}`);
    return response.data;
  }

  async getUserLogs(userId: string, params?: LogQueryParams): Promise<LogResponse<LogEntry>> {
    const searchParams = new URLSearchParams();
    if (params?.level) searchParams.set('level', params.level);
    if (params?.start_time) searchParams.set('start_time', params.start_time);
    if (params?.end_time) searchParams.set('end_time', params.end_time);
    if (params?.correlation_id) searchParams.set('correlation_id', params.correlation_id);
    if (params?.module) searchParams.set('module', params.module);
    if (params?.collection) searchParams.set('collection', params.collection);
    if (params?.search) searchParams.set('search', params.search);
    if (params?.limit) searchParams.set('limit', params.limit.toString());
    if (params?.offset) searchParams.set('offset', params.offset.toString());
    if (params?.sort) searchParams.set('sort', params.sort);
    
    const query = searchParams.toString();
    const endpoint = `/logs/user/${encodeURIComponent(userId)}${query ? `?${query}` : ''}`;
    
    const response = await this.get<ApiResponse<LogResponse<LogEntry>>>(endpoint);
    return response.data;
  }

  async getCollectionLogs(collection: string, params?: LogQueryParams): Promise<LogResponse<LogEntry>> {
    const searchParams = new URLSearchParams();
    if (params?.level) searchParams.set('level', params.level);
    if (params?.start_time) searchParams.set('start_time', params.start_time);
    if (params?.end_time) searchParams.set('end_time', params.end_time);
    if (params?.correlation_id) searchParams.set('correlation_id', params.correlation_id);
    if (params?.module) searchParams.set('module', params.module);
    if (params?.user_id) searchParams.set('user_id', params.user_id);
    if (params?.search) searchParams.set('search', params.search);
    if (params?.limit) searchParams.set('limit', params.limit.toString());
    if (params?.offset) searchParams.set('offset', params.offset.toString());
    if (params?.sort) searchParams.set('sort', params.sort);
    
    const query = searchParams.toString();
    const endpoint = `/logs/collection/${encodeURIComponent(collection)}${query ? `?${query}` : ''}`;
    
    const response = await this.get<ApiResponse<LogResponse<LogEntry>>>(endpoint);
    return response.data;
  }

  async createLogEntry(request: CreateLogRequest): Promise<CreateLogResponse> {
    const response = await this.post<ApiResponse<CreateLogResponse>>('/logs/create', request);
    return response.data;
  }

  async createAuditEvent(request: CreateAuditRequest): Promise<CreateLogResponse> {
    const response = await this.post<ApiResponse<CreateLogResponse>>('/logs/audit/create', request);
    return response.data;
  }

  async flushLogs(): Promise<void> {
    await this.post<ApiResponse<string>>('/logs/flush');
  }

  async getLoggingHealth(): Promise<LoggingHealthResponse> {
    const response = await this.get<ApiResponse<LoggingHealthResponse>>('/logs/health');
    return response.data;
  }

  // Plugin methods
  async getPlugins(): Promise<PluginInfo[]> {
    const response = await this.get<ApiResponse<PluginInfo[]>>('/plugins');
    return response.data || [];
  }

  async getPluginDetails(pluginName: string): Promise<PluginDetails> {
    const response = await this.get<ApiResponse<PluginDetails>>(`/plugins/${encodeURIComponent(pluginName)}`);
    return response.data;
  }

  async getPluginAdminPages(): Promise<PluginAdminPage[]> {
    const response = await this.get<ApiResponse<PluginAdminPage[]>>('/api/admin/plugin-pages');
    return response.data || [];
  }

  async getPluginRecordFields(collection: string): Promise<PluginRecordField[]> {
    const params = new URLSearchParams({ collection });
    const response = await this.get<ApiResponse<PluginRecordField[]>>(
      `/api/admin/plugin-record-fields?${params.toString()}`
    );
    return response.data || [];
  }

  async analyzePlugin(pluginFile: File): Promise<PluginAnalysisResult> {
    const formData = new FormData();
    formData.append('plugin_package', pluginFile);

    const response = await fetch(`${this.baseUrl}/plugins/analyze`, {
      method: 'POST',
      headers: this.token ? { 'Authorization': `Bearer ${this.token}` } : {},
      credentials: 'include',
      body: formData
    });

    if (!response.ok) {
      throw new Error(await this.fetchErrorMessage(response));
    }

    const result = await response.json();
    return result.data;
  }

  async installPlugin(pluginFile: File): Promise<PluginInstallResult> {
    const formData = new FormData();
    formData.append('plugin_package', pluginFile);

    const response = await fetch(`${this.baseUrl}/plugins`, {
      method: 'POST',
      headers: this.token ? { 'Authorization': `Bearer ${this.token}` } : {},
      credentials: 'include',
      body: formData
    });

    if (!response.ok) {
      throw new Error(await this.fetchErrorMessage(response));
    }

    const result = await response.json();
    return result.data;
  }

  async enablePlugin(pluginName: string): Promise<void> {
    await this.post(`/plugins/${encodeURIComponent(pluginName)}/enable`, {});
  }

  async disablePlugin(pluginName: string): Promise<void> {
    await this.post(`/plugins/${encodeURIComponent(pluginName)}/disable`, {});
  }

  async uninstallPlugin(pluginName: string): Promise<void> {
    await this.delete(`/plugins/${encodeURIComponent(pluginName)}`);
  }

  // VFS File Operations

  /**
   * Upload a file to the VFS for a specific collection
   */
  async uploadFile(
    collection: string,
    file: File,
    path?: string,
    onProgress?: (progress: number) => void,
    options?: {
      overwrite?: boolean;
      tags?: string[];
      custom_metadata?: Record<string, string>;
    }
  ): Promise<FileMetadata> {
    const formData = new FormData();
    formData.append('file', file);
    formData.append('collection', collection);
    if (path) {
      formData.append('path', path);
    }
    if (options?.overwrite !== undefined) {
      formData.append('overwrite', String(options.overwrite));
    }
    if (options?.tags?.length) {
      formData.append('tags', JSON.stringify(options.tags));
    }
    if (options?.custom_metadata) {
      formData.append('custom_metadata', JSON.stringify(options.custom_metadata));
    }

    return new Promise((resolve, reject) => {
      const xhr = new XMLHttpRequest();
      
      // Track upload progress
      if (onProgress) {
        xhr.upload.onprogress = (event) => {
          if (event.lengthComputable) {
            const progress = Math.round((event.loaded / event.total) * 100);
            onProgress(progress);
          }
        };
      }

      xhr.onload = async () => {
        if (xhr.status >= 200 && xhr.status < 300) {
          try {
            const response = JSON.parse(xhr.responseText);
            resolve(response.data);
          } catch (error) {
            console.error('Failed to parse response:', error);
            reject(new Error('Failed to parse response'));
          }
        } else {
          reject(new Error(`Upload failed: HTTP ${xhr.status}`));
        }
      };

      xhr.onerror = () => {
        reject(new Error('Upload failed'));
      };

      xhr.open('POST', `${this.baseUrl}/collections/${encodeURIComponent(collection)}/files`);
      xhr.withCredentials = true;
      if (this.token) {
        xhr.setRequestHeader('Authorization', `Bearer ${this.token}`);
      }
      xhr.send(formData);
    });
  }

  /**
   * Download a file from the VFS
   */
  async downloadFile(collection: string, fileId: string): Promise<Blob> {
    const response = await fetch(
      `${this.baseUrl}/collections/${encodeURIComponent(collection)}/files/${encodeURIComponent(fileId)}`,
      {
        headers: this.token ? { 'Authorization': `Bearer ${this.token}` } : {},
        credentials: 'include',
      }
    );

    if (!response.ok) {
      throw new Error(`Download failed: HTTP ${response.status}`);
    }

    return response.blob();
  }

  /**
   * Get file metadata without downloading content
   */
  async getFileMetadata(collection: string, fileId: string): Promise<FileMetadata> {
    const response = await this.get<ApiResponse<FileMetadata>>(
      `/collections/${encodeURIComponent(collection)}/files/${encodeURIComponent(fileId)}/metadata`
    );
    return response.data;
  }

  /**
   * Delete a file from the VFS
   */
  async deleteFile(collection: string, fileId: string): Promise<void> {
    await this.delete(`/collections/${encodeURIComponent(collection)}/files/${encodeURIComponent(fileId)}`);
  }

  /**
   * Move or rename a file within a collection
   */
  async moveFile(collection: string, fileId: string, request: FileMoveRequest): Promise<FileMetadata> {
    const response = await this.patch<ApiResponse<FileMetadata>>(
      `/collections/${encodeURIComponent(collection)}/files/${encodeURIComponent(fileId)}`,
      request
    );
    return response.data;
  }

  /**
   * List files in a collection
   */
  async listFiles(collection: string, params?: FileListRequest): Promise<FileListResponse> {
    const searchParams = new URLSearchParams();
    if (params?.directory) searchParams.set('directory', params.directory);
    if (params?.recursive !== undefined) searchParams.set('recursive', params.recursive.toString());
    if (params?.mime_filter) searchParams.set('mime_filter', params.mime_filter);
    if (params?.tag_filter) searchParams.set('tag_filter', params.tag_filter.join(','));
    if (params?.offset) searchParams.set('offset', params.offset.toString());
    if (params?.limit) searchParams.set('limit', params.limit.toString());

    const query = searchParams.toString();
    const endpoint = `/collections/${encodeURIComponent(collection)}/files${query ? `?${query}` : ''}`;
    
    const response = await this.get<ApiResponse<FileListResponse>>(endpoint);
    return response.data;
  }

  /**
   * Get VFS usage statistics for a collection
   */
  async getVfsUsage(collection: string): Promise<VfsUsageStats> {
    const response = await this.get<ApiResponse<VfsUsageStats>>(
      `/collections/${encodeURIComponent(collection)}/vfs/usage`
    );
    return response.data;
  }

  /**
   * Upload multiple files to the VFS
   */
  async uploadMultipleFiles(
    collection: string,
    files: File[],
    onProgress?: (fileId: string, progress: number) => void,
    onFileComplete?: (fileId: string, metadata: FileMetadata) => void,
    onFileError?: (fileId: string, error: string) => void
  ): Promise<FileMetadata[]> {
    const results: FileMetadata[] = [];
    
    // Upload files in parallel with limited concurrency
    const concurrency = 3;
    const chunks: File[][] = [];
    
    for (let i = 0; i < files.length; i += concurrency) {
      chunks.push(files.slice(i, i + concurrency));
    }

    for (const chunk of chunks) {
      const promises = chunk.map(async (file) => {
        const fileId = `${file.name}-${Date.now()}`;
        try {
          const metadata = await this.uploadFile(
            collection,
            file,
            undefined,
            (progress) => onProgress?.(fileId, progress)
          );
          onFileComplete?.(fileId, metadata);
          return metadata;
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : 'Upload failed';
          onFileError?.(fileId, errorMessage);
          throw error;
        }
      });

      const chunkResults = await Promise.allSettled(promises);
      chunkResults.forEach((result) => {
        if (result.status === 'fulfilled') {
          results.push(result.value);
        }
      });
    }

    return results;
  }

  // User Preferences Operations

  /**
   * Store or update a user preference
   */
  async storeUserPreference(key: string, value: unknown): Promise<void> {
    await this.put(`/user/preferences/${encodeURIComponent(key)}`, value);
  }

  /**
   * Get a specific user preference by key
   */
  async getUserPreference(key: string): Promise<unknown | null> {
    try {
      const response = await this.get<ApiResponse<{ preference?: { preference_value: unknown } }>>(`/user/preferences/${encodeURIComponent(key)}`);
      return response.data.preference?.preference_value || null;
    } catch (error) {
      // Return null if preference doesn't exist
      if (error instanceof Error && error.message.includes('not found')) {
        return null;
      }
      throw error;
    }
  }

  /**
   * Get all user preferences
   */
  async getAllUserPreferences(): Promise<Record<string, unknown>> {
    const response = await this.get<ApiResponse<{ preferences: Array<{ preference_key: string; preference_value: unknown }> }>>('/user/preferences');
    const preferences: Record<string, unknown> = {};
    
    response.data.preferences.forEach(pref => {
      preferences[pref.preference_key] = pref.preference_value;
    });
    
    return preferences;
  }

  /**
   * Delete a specific user preference
   */
  async deleteUserPreference(key: string): Promise<void> {
    await this.delete(`/user/preferences/${encodeURIComponent(key)}`);
  }

  /**
   * Delete all user preferences
   */
  async deleteAllUserPreferences(): Promise<void> {
    await this.delete('/user/preferences');
  }

  /**
   * Store field customization settings for a collection
   */
  async storeFieldCustomization(collection: string, customization: unknown): Promise<void> {
    const key = `field_customization_${collection}`;
    await this.storeUserPreference(key, customization);
  }

  /**
   * Get field customization settings for a collection
   */
  async getFieldCustomization(collection: string): Promise<unknown | null> {
    const key = `field_customization_${collection}`;
    const result = await this.getUserPreference(key);
    return result;
  }

  /**
   * Delete field customization settings for a collection
   */
  async deleteFieldCustomization(collection: string): Promise<void> {
    const key = `field_customization_${collection}`;
    await this.deleteUserPreference(key);
  }

  // Site Settings Operations

  /**
   * Get public site settings safe for unauthenticated UI bootstrapping
   */
  async getPublicSiteSettings(): Promise<PublicSiteSettings> {
    const response = await this.get<ApiResponse<PublicSiteSettings>>('/settings/public');
    return response.data;
  }

  /**
   * Get all site settings
   */
  async getSiteSettings(includeHealth?: boolean): Promise<SiteSettings> {
    const params = includeHealth ? '?include_health=true' : '';
    const response = await this.get<ApiResponse<SiteSettingsResponse>>(`/api/admin/settings${params}`);
    return response.data.settings!;
  }

  /**
   * Update site settings (partial update)
   */
  async updateSiteSettings(request: UpdateSiteSettingsRequest): Promise<void> {
    await this.put<ApiResponse<SiteSettingsResponse>>('/api/admin/settings', request);
  }

  /**
   * Reset site settings to defaults
   */
  async resetSiteSettings(): Promise<void> {
    await this.post<ApiResponse<SiteSettingsResponse>>('/api/admin/settings/reset');
  }

  /**
   * Get a specific settings section
   */
  async getSettingsSection(section: string): Promise<unknown> {
    const response = await this.get<ApiResponse<unknown>>(`/api/admin/settings/${encodeURIComponent(section)}`);
    return response.data;
  }

  /**
   * Update a specific settings section
   */
  async updateSettingsSection(section: string, data: unknown): Promise<void> {
    await this.put<ApiResponse<SiteSettingsResponse>>(`/api/admin/settings/${encodeURIComponent(section)}`, data);
  }

  /**
   * Test email configuration
   */
  async testEmailConfiguration(): Promise<boolean> {
    const response = await this.post<ApiResponse<SiteSettingsResponse>>('/api/admin/settings/email/test');
    return response.data.success;
  }

  /**
   * Get settings health status
   */
  async getSettingsHealth(): Promise<SettingsHealthStatus> {
    const response = await this.get<ApiResponse<SettingsHealthStatus>>('/api/admin/settings/health');
    return response.data;
  }
}

export const apiService = new ApiService();
