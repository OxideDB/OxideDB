import React, { createContext, useContext, useState, useEffect } from 'react';
import type { ReactNode } from 'react';
import { apiService } from '../services/api';
import type { User } from '../types/api';

interface AuthContextType {
  user: User | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  login: (email: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  refreshUser: () => Promise<void>;
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

  const isAuthenticated = !!user && apiService.isAuthenticated();

  // Check if user is already authenticated on app start
  useEffect(() => {
    const initializeAuth = async () => {
      try {
        // Check if we have a token and it's valid
        if (apiService.isAuthenticated()) {
          const isValid = await apiService.validateToken();
          if (isValid) {
            // Token is valid, fetch user data
            const userData = await apiService.getCurrentUser();
            setUser(userData);
          } else {
            // Token is invalid, clear it
            apiService.clearToken();
          }
        }
      } catch (error) {
        console.error('Auth initialization failed:', error);
        apiService.clearToken();
      } finally {
        setIsLoading(false);
      }
    };

    initializeAuth();
  }, []);

  const login = async (email: string, password: string): Promise<void> => {
    try {
      const authResponse = await apiService.login(email, password);
      
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
      apiService.clearToken();
    }
  };

  const value: AuthContextType = {
    user,
    isAuthenticated,
    isLoading,
    login,
    logout,
    refreshUser,
  };

  return (
    <AuthContext.Provider value={value}>
      {children}
    </AuthContext.Provider>
  );
};