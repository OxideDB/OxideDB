import type { 
  ApiError, CollectionStats, CreateCollectionRequest, DbRecord, ApiHealthStatus,
  CollectionSchema, CollectionPermissionsInfo, CollectionPermissions, PermissionPresetType, 
  ApiResponse, AuthResponse, User, TokenValidationResponse,
  LogQueryParams, AuditQueryParams, LogResponse, LogEntry, SecurityAuditEvent,
  DashboardMetrics, RetentionStats, CreateLogRequest, CreateAuditRequest,
  CreateLogResponse, LoggingHealthResponse,
   FileMetadata, FileListRequest, 
  FileListResponse, VfsUsageStats,
  SiteSettings, SiteSettingsResponse, UpdateSiteSettingsRequest, SettingsHealthStatus
} from '../types/api';
import { capabilityNameToObject } from '../types/api';

// Plugin-related interfaces
interface PluginInfo {
  name: string;
  status: 'Enabled' | 'Disabled' | 'Error' | 'Loading' | 'Uninstalling';
  version: string;
  description: string;
  author: string;
  capabilities: string[];
  trust_level: 'Untrusted' | 'PartiallyTrusted' | 'FullyTrusted' | 'System';
  routes: PluginRoute[];
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
  access_token: string;
  refresh_token: string;
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
}

class ApiService {
  private baseUrl: string;
  private token: string | null = null;
  private refreshToken: string | null = null;
  private isRefreshing: boolean = false;
  private refreshPromise: Promise<string> | null = null;

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
    
    // Load tokens from storage with fallback to sessionStorage
    this.token = localStorage.getItem('auth_token') || sessionStorage.getItem('auth_token');
    this.refreshToken = localStorage.getItem('refresh_token') || sessionStorage.getItem('refresh_token');
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

    // Add authorization header if we have a token
    if (this.token) {
      headers['Authorization'] = `Bearer ${this.token}`;
    }

    let response = await fetch(url, {
      ...options,
      headers,
    });

    // If we get a 401 and have a refresh token, try to refresh
    if (response.status === 401 && this.refreshToken && !this.isRefreshing) {
      try {
        const newAccessToken = await this.performTokenRefresh();
        
        // Retry the original request with the new token
        headers['Authorization'] = `Bearer ${newAccessToken}`;
        response = await fetch(url, {
          ...options,
          headers,
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

  private async performTokenRefresh(): Promise<string> {
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

  private async doTokenRefresh(): Promise<string> {
    if (!this.refreshToken) {
      throw new Error('No refresh token available');
    }

    const response = await fetch(`${this.baseUrl}/auth/refresh`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ refresh_token: this.refreshToken }),
    });

    if (!response.ok) {
      throw new Error('Token refresh failed');
    }

    const data: ApiResponse<RefreshTokenResponse> = await response.json();
    
    // Update stored tokens
    this.token = data.data.access_token;
    this.refreshToken = data.data.refresh_token;
    
    localStorage.setItem('auth_token', this.token);
    localStorage.setItem('refresh_token', this.refreshToken);

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

  async delete<T>(endpoint: string): Promise<T> {
    return this.request<T>(endpoint, { method: 'DELETE' });
  }

  // Auth methods
  setTokens(accessToken: string, refreshToken?: string) {
    this.token = accessToken;
    localStorage.setItem('auth_token', accessToken);
    sessionStorage.setItem('auth_token', accessToken);
    
    if (refreshToken) {
      this.refreshToken = refreshToken;
      localStorage.setItem('refresh_token', refreshToken);
      sessionStorage.setItem('refresh_token', refreshToken);
    }
  }

  clearTokens() {
    this.token = null;
    this.refreshToken = null;
    localStorage.removeItem('auth_token');
    localStorage.removeItem('refresh_token');
    sessionStorage.removeItem('auth_token');
    sessionStorage.removeItem('refresh_token');
  }

  isAuthenticated(): boolean {
    return !!this.token;
  }

  hasRefreshToken(): boolean {
    return !!this.refreshToken;
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
    });
    
    // Store the tokens automatically
    this.setTokens(response.data.token, response.data.refresh_token || undefined);
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
        return false;
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
      if (!this.refreshToken) {
        return false;
      }
      
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
    
    const query = searchParams.toString();
    const endpoint = `/collections/${encodeURIComponent(collection)}/records${query ? `?${query}` : ''}`;
    
    // Note: list_records returns PaginatedResponse, not ApiResponse
    const response = await this.request<PaginatedResponse<DbRecord>>(endpoint);
    return response.data;
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

  async analyzePlugin(pluginFile: File): Promise<PluginAnalysisResult> {
    const formData = new FormData();
    formData.append('plugin_package', pluginFile);

    const response = await fetch(`${this.baseUrl}/plugins/analyze`, {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${this.token}`
      },
      body: formData
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(errorText || `HTTP ${response.status}`);
    }

    const result = await response.json();
    return result.data;
  }

  async installPlugin(pluginFile: File, trustLevel: string, capabilities: string[]): Promise<void> {
    // Convert capability names to proper capability objects
    const capabilityObjects = capabilities.map(capName => {
      try {
        return capabilityNameToObject(capName);
      } catch (error) {
        console.error(`Failed to convert capability '${capName}':`, error);
        throw new Error(`Invalid capability: ${capName}`);
      }
    });

    const formData = new FormData();
    formData.append('plugin_package', pluginFile);
    formData.append('trust_level', trustLevel);
    formData.append('capabilities', JSON.stringify(capabilityObjects));

    const response = await fetch(`${this.baseUrl}/plugins`, {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${this.token}`
      },
      body: formData
    });

    if (!response.ok) {
      const errorText = await response.text();
      throw new Error(errorText || `HTTP ${response.status}`);
    }
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
    onProgress?: (progress: number) => void
  ): Promise<FileMetadata> {
    const formData = new FormData();
    formData.append('file', file);
    formData.append('collection', collection);
    if (path) {
      formData.append('path', path);
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
   * Get all site settings
   */
  async getSiteSettings(includeHealth?: boolean): Promise<SiteSettings> {
    const params = includeHealth ? '?include_health=true' : '';
    const response = await this.get<ApiResponse<SiteSettingsResponse>>(`/admin/settings${params}`);
    return response.data.settings!;
  }

  /**
   * Update site settings (partial update)
   */
  async updateSiteSettings(request: UpdateSiteSettingsRequest): Promise<void> {
    await this.put<ApiResponse<SiteSettingsResponse>>('/admin/settings', request);
  }

  /**
   * Reset site settings to defaults
   */
  async resetSiteSettings(): Promise<void> {
    await this.post<ApiResponse<SiteSettingsResponse>>('/admin/settings/reset');
  }

  /**
   * Get a specific settings section
   */
  async getSettingsSection(section: string): Promise<unknown> {
    const response = await this.get<ApiResponse<unknown>>(`/admin/settings/${encodeURIComponent(section)}`);
    return response.data;
  }

  /**
   * Update a specific settings section
   */
  async updateSettingsSection(section: string, data: unknown): Promise<void> {
    await this.put<ApiResponse<SiteSettingsResponse>>(`/admin/settings/${encodeURIComponent(section)}`, data);
  }

  /**
   * Test email configuration
   */
  async testEmailConfiguration(): Promise<boolean> {
    const response = await this.post<ApiResponse<SiteSettingsResponse>>('/admin/settings/email/test');
    return response.data.success;
  }

  /**
   * Get settings health status
   */
  async getSettingsHealth(): Promise<SettingsHealthStatus> {
    const response = await this.get<ApiResponse<SettingsHealthStatus>>('/admin/settings/health');
    return response.data;
  }
}

export const apiService = new ApiService();
