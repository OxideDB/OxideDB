import { createContext } from 'react';
import type { User } from '../types/api';

export interface AuthContextType {
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

export const AuthContext = createContext<AuthContextType | undefined>(undefined);
