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

class ApiService {
  private baseUrl: string;
  private token: string | null = null;

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
    this.token = localStorage.getItem('auth_token');
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

    if (this.token) {
      headers['Authorization'] = `Bearer ${this.token}`;
    }

    const response = await fetch(url, {
      ...options,
      headers,
    });

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
  setToken(token: string) {
    this.token = token;
    localStorage.setItem('auth_token', token);
  }

  clearToken() {
    this.token = null;
    localStorage.removeItem('auth_token');
  }

  isAuthenticated(): boolean {
    return !!this.token;
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
    
    // Store the token automatically
    this.setToken(response.data.token);
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
      // Always clear token, even if logout request fails
      this.clearToken();
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
    } catch {
      this.clearToken();
      return false;
    }
  }

  async getCurrentUser(): Promise<User> {
    const response = await this.get<ApiResponse<User>>('/auth/me');
    return response.data;
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
    return schema.collection_type === 'auth';
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