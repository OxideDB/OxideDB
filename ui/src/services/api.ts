import type { ApiError, CollectionStats, CreateCollectionRequest, DbRecord, HealthStatus, CollectionSchema, CollectionPermissionsInfo, CollectionPermissions, PermissionPresetType, ApiResponse, AuthResponse, User, TokenValidationResponse } from '../types/api';

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
  meta?: any;
  success: boolean;
}

// Refresh token response type
interface RefreshTokenResponse {
  access_token: string;
  refresh_token: string;
  expires_in: number;
  refresh_expires_in: number;
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
    let headers: Record<string, string> = {
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
    const response = await this.get<ApiResponse<User>>('/auth/me');
    return response.data;
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
  async getHealth(): Promise<HealthStatus & { version?: string }> {
    const response = await this.request<ApiResponse<HealthStatus & { version?: string }>>('/health');
    return response.data;
  }

  // Collection methods
  async getCollections(): Promise<CollectionSchema[]> {
    const response = await this.request<ApiResponse<CollectionSchema[]>>('/collections');
    return response.data;
  }

  async createCollection(schema: CreateCollectionRequest): Promise<void> {
    return this.request<void>('/collections', {
      method: 'POST',
      body: JSON.stringify(schema),
    });
  }

  async deleteCollection(collection: string): Promise<void> {
    return this.request<void>(`/collections/${encodeURIComponent(collection)}`, {
      method: 'DELETE',
    });
  }

  // Helper method to check if a collection is a system collection
  isSystemCollection(schema: CollectionSchema): boolean {
    return schema.collection_type === 'auth' || schema.name.startsWith('_');
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
    return this.request<void>(`/collections/${encodeURIComponent(collection)}/schema`, {
      method: 'PUT',
      body: JSON.stringify(schema),
    });
  }

  // Record methods
  async getRecords(collection: string, params?: { limit?: number; offset?: number }): Promise<DbRecord[]> {
    const searchParams = new URLSearchParams();
    if (params?.limit) searchParams.set('limit', params.limit.toString());
    if (params?.offset) searchParams.set('offset', params.offset.toString());
    
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
}

export const apiService = new ApiService();