import React, { createContext, useContext, useState, useEffect, useCallback, useRef } from 'react';
import type { ReactNode } from 'react';
import { apiService } from '../services/api';
import type { User } from '../types/api';

interface AuthContextType {
  user: User | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  hasRefreshToken: boolean;
  authCollections: { name: string; identifier_field: string; registration_enabled: boolean; email_verification_required: boolean }[];
  login: (collection: string, identifier: string, credential: string) => Promise<void>;
  logout: () => Promise<void>;
  refreshUser: () => Promise<void>;
  refreshTokens: () => Promise<boolean>;
  loadAuthCollections: () => Promise<void>;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export const useAuth = () => {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
};

interface AuthProviderProps {
  children: ReactNode;
}

export const AuthProvider: React.FC<AuthProviderProps> = ({ children }) => {
  const [user, setUser] = useState<User | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [authCollections, setAuthCollections] = useState<{ name: string; identifier_field: string; registration_enabled: boolean; email_verification_required: boolean }[]>([]);
  const [collectionsLoading, setCollectionsLoading] = useState(false);
  const refreshIntervalRef = useRef<NodeJS.Timeout | null>(null);

  const isAuthenticated = !!user && apiService.isAuthenticated();
  const hasRefreshToken = apiService.hasRefreshToken();

  // Setup automatic token refresh interval
  useEffect(() => {
    if (isAuthenticated && hasRefreshToken) {
      // Refresh tokens every 23 hours (assuming 24h access token expiry)
      const refreshInterval = 23 * 60 * 60 * 1000; // 23 hours in milliseconds
      
      refreshIntervalRef.current = setInterval(async () => {
        try {
          const success = await apiService.refreshTokens();
          if (!success) {
            console.warn('Failed to refresh tokens, user may need to re-authenticate');
          }
        } catch (error) {
          console.error('Error during automatic token refresh:', error);
        }
      }, refreshInterval);

      return () => {
        if (refreshIntervalRef.current) {
          clearInterval(refreshIntervalRef.current);
        }
      };
    }
  }, [isAuthenticated, hasRefreshToken]);

  // Load auth collections - memoized to prevent continuous re-renders
  const loadAuthCollections = useCallback(async () => {
    // Prevent multiple simultaneous calls
    if (collectionsLoading) return;
    
    setCollectionsLoading(true);
    try {
      const collectionsData = await apiService.getAuthCollections();
      setAuthCollections(collectionsData.collections);
    } catch (error) {
      console.error('Failed to load auth collections:', error);
      setAuthCollections([]);
    } finally {
      setCollectionsLoading(false);
    }
  }, [collectionsLoading]);

  // Check if user is already authenticated on app start
  useEffect(() => {
    const initializeAuth = async () => {
      try {
        // Load auth collections first
        await loadAuthCollections();
        
        // Check if we have any token stored
        if (apiService.isAuthenticated()) {
          // Try to get user data directly first (this will trigger automatic refresh if needed)
          try {
            const userData = await apiService.getCurrentUser();
            setUser(userData);
          } catch (userError) {
            console.error('Failed to get user data:', userError);
            
            // If getting user data failed, try refresh token if available
            if (apiService.hasRefreshToken()) {
              try {
                const refreshSuccess = await apiService.refreshTokens();
                if (refreshSuccess) {
                  // Try getting user data again after refresh
                  const userData = await apiService.getCurrentUser();
                  setUser(userData);
                } else {
                  console.warn('Token refresh failed during initialization');
                  apiService.clearTokens();
                }
              } catch (refreshError) {
                console.error('Failed to refresh tokens on startup:', refreshError);
                apiService.clearTokens();
              }
            } else {
              console.warn('No refresh token available, clearing session');
              apiService.clearTokens();
            }
          }
        }
      } catch (error) {
        console.error('Auth initialization failed:', error);
        // Don't clear tokens here unless it's a definitive auth failure
        // The error might be network-related
      } finally {
        setIsLoading(false);
      }
    };

    initializeAuth();
  }, []); // Remove loadAuthCollections from dependencies since it's called directly

  const login = async (collection: string, identifier: string, credential: string): Promise<void> => {
    try {
      const authResponse = await apiService.login(collection, identifier, credential);
      
      // Create user object from auth response
      const userData: User = {
        id: authResponse.user_id,
        email: authResponse.email,
        is_superuser: authResponse.role === 'superuser',
        created_at: new Date().toISOString(), // We don't have this from login response
      };
      
      setUser(userData);
    } catch (error) {
      throw error; // Re-throw to let the login component handle it
    }
  };

  const logout = async (): Promise<void> => {
    try {
      // Clear the refresh interval
      if (refreshIntervalRef.current) {
        clearInterval(refreshIntervalRef.current);
        refreshIntervalRef.current = null;
      }
      
      await apiService.logout();
    } catch (error) {
      console.error('Logout error:', error);
    } finally {
      setUser(null);
    }
  };

  const refreshUser = async (): Promise<void> => {
    try {
      if (apiService.isAuthenticated()) {
        const userData = await apiService.getCurrentUser();
        setUser(userData);
      }
    } catch (error) {
      console.error('Failed to refresh user:', error);
      setUser(null);
      apiService.clearTokens();
    }
  };

  const refreshTokens = async (): Promise<boolean> => {
    try {
      const success = await apiService.refreshTokens();
      if (success) {
        // Optionally refresh user data after token refresh
        await refreshUser();
      }
      return success;
    } catch (error) {
      console.error('Failed to refresh tokens:', error);
      setUser(null);
      return false;
    }
  };

  const value: AuthContextType = {
    user,
    isAuthenticated,
    isLoading,
    hasRefreshToken,
    authCollections,
    login,
    logout,
    refreshUser,
    refreshTokens,
    loadAuthCollections,
  };

  return (
    <AuthContext.Provider value={value}>
      {children}
    </AuthContext.Provider>
  );
};