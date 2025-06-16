import type { ApiError, CollectionStats, CreateCollectionRequest, DbRecord, HealthStatus, CollectionSchema, CollectionPermissionsInfo, CollectionPermissions, PermissionPresetType, ApiResponse, AuthResponse, User } from '../types/api';

class ApiService {
  private baseUrl: string;
  private token: string | null = null;

  constructor(baseUrl?: string) {
    // Auto-detect base URL based on environment
    if (baseUrl) {
      this.baseUrl = baseUrl;
    } else if (window.location.pathname.startsWith('/admin')) {
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

  async login(email: string, password: string): Promise<AuthResponse> {
    const response = await this.post<ApiResponse<AuthResponse>>('/auth/login', {
      email,
      password,
    });
    
    // Store the token automatically
    this.setToken(response.data.token);
    return response.data;
  }

  async register(email: string, password: string, isSuper: boolean = false): Promise<void> {
    await this.post<void>('/auth/register', {
      email,
      password,
      is_superuser: isSuper,
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
      await this.get<void>('/auth/validate');
      return true;
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
  async getHealth(): Promise<HealthStatus> {
    return this.request<HealthStatus>('/health');
  }

  // Collection methods
  async getCollections(): Promise<CollectionSchema[]> {
    return this.request<CollectionSchema[]>('/collections');
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
    return this.request<CollectionStats>(`/collections/${encodeURIComponent(collection)}/stats`);
  }

  async getCollectionSchema(collection: string): Promise<CollectionSchema> {
    return this.request<CollectionSchema>(`/collections/${encodeURIComponent(collection)}/schema`);
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
    
    return this.request<DbRecord[]>(endpoint);
  }

  async createRecord(collection: string, data: Record<string, unknown>): Promise<DbRecord> {
    return this.request<DbRecord>(`/collections/${encodeURIComponent(collection)}/records`, {
      method: 'POST',
      body: JSON.stringify(data),
    });
  }

  async getRecord(collection: string, id: string): Promise<DbRecord> {
    return this.request<DbRecord>(`/collections/${encodeURIComponent(collection)}/records/${encodeURIComponent(id)}`);
  }

  async updateRecord(collection: string, id: string, data: Record<string, unknown>): Promise<DbRecord> {
    return this.request<DbRecord>(`/collections/${encodeURIComponent(collection)}/records/${encodeURIComponent(id)}`, {
      method: 'PUT',
      body: JSON.stringify(data),
    });
  }

  async deleteRecord(collection: string, id: string): Promise<DbRecord> {
    return this.request<DbRecord>(`/collections/${encodeURIComponent(collection)}/records/${encodeURIComponent(id)}`, {
      method: 'DELETE',
    });
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