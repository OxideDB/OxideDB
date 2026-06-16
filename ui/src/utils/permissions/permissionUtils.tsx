import React from 'react';
import { 
  Shield, 
  Users, 
  Lock, 
  Unlock, 
  Eye, 
  Edit3, 
  Trash2, 
  Plus,
  RotateCw,
  Settings,
  Database,
  UserPlus,
  LogOut,
  User
} from 'lucide-react';
import type {
  CrudOperation,
  AuthOperation,
  PermissionLevel,
  CrudOperationRule,
  AuthOperationRule
} from '@/types/api';

export const getPermissionLevelDisplay = (level: PermissionLevel): { 
  text: string; 
  color: string; 
  icon: React.ReactNode 
} => {
  if (typeof level === 'object' && 'rule' in level) {
    return { 
      text: 'Custom Rule', 
      color: 'bg-purple-100 text-purple-800 dark:bg-purple-950/70 dark:text-purple-200 dark:border-purple-800/70',
      icon: <Settings className="w-3 h-3" /> 
    };
  }
  
  switch (level) {
    case 'none':
      return { 
        text: 'None', 
        color: 'bg-gray-100 text-gray-800 dark:bg-muted dark:text-muted-foreground dark:border-border',
        icon: <Lock className="w-3 h-3" /> 
      };
    case 'public':
      return { 
        text: 'Public', 
        color: 'bg-green-100 text-green-800 dark:bg-green-950/70 dark:text-green-200 dark:border-green-800/70',
        icon: <Unlock className="w-3 h-3" /> 
      };
    case 'authenticatedonly':
      return { 
        text: 'Authenticated', 
        color: 'bg-blue-100 text-blue-800 dark:bg-blue-950/70 dark:text-blue-200 dark:border-blue-800/70',
        icon: <Users className="w-3 h-3" /> 
      };
    case 'superuseronly':
      return { 
        text: 'Superuser Only', 
        color: 'bg-red-100 text-red-800 dark:bg-red-950/70 dark:text-red-200 dark:border-red-800/70',
        icon: <Shield className="w-3 h-3" /> 
      };
    default:
      return { 
        text: 'Unknown', 
        color: 'bg-gray-100 text-gray-800 dark:bg-muted dark:text-muted-foreground dark:border-border',
        icon: <Lock className="w-3 h-3" /> 
      };
  }
};

export const getCrudOperationIcon = (operation: CrudOperation) => {
  switch (operation) {
    case 'create': return <Plus className="w-4 h-4" />;
    case 'read': return <Eye className="w-4 h-4" />;
    case 'update': return <Edit3 className="w-4 h-4" />;
    case 'delete': return <Trash2 className="w-4 h-4" />;
    case 'list': return <Eye className="w-4 h-4" />;
    default: return <Settings className="w-4 h-4" />;
  }
};

export const getAuthOperationIcon = (operation: AuthOperation) => {
  switch (operation) {
    case 'login': return <Lock className="w-4 h-4" />;
    case 'register': return <UserPlus className="w-4 h-4" />;
    case 'token_validation': return <Shield className="w-4 h-4" />;
    case 'token_refresh': return <RotateCw className="w-4 h-4" />;
    case 'logout': return <LogOut className="w-4 h-4" />;
    case 'get_current_user': return <User className="w-4 h-4" />;
    case 'list_auth_collections': return <Database className="w-4 h-4" />;
    default: return <Settings className="w-4 h-4" />;
  }
};

export const getOperationColor = (operation: string) => {
  switch (operation) {
    case "read":
    case "list":
      return "bg-blue-100 text-blue-800 dark:bg-blue-950/70 dark:text-blue-200 dark:border-blue-800/70";
    case "create":
    case "update":
      return "bg-green-100 text-green-800 dark:bg-green-950/70 dark:text-green-200 dark:border-green-800/70";
    case "delete":
      return "bg-red-100 text-red-800 dark:bg-red-950/70 dark:text-red-200 dark:border-red-800/70";
    default:
      return "bg-gray-100 text-gray-800 dark:bg-muted dark:text-muted-foreground dark:border-border";
  }
};

// Helper function to sort CRUD operations in a consistent order
export const sortCrudOperations = (
  operations: Partial<Record<CrudOperation, CrudOperationRule>>
): Array<[CrudOperation, CrudOperationRule]> => {
  if (!operations) return [];
  const operationOrder = ['create', 'read', 'update', 'delete', 'list'];
  return (Object.entries(operations) as Array<[CrudOperation, CrudOperationRule]>).sort(([a], [b]) => {
    const indexA = operationOrder.indexOf(a);
    const indexB = operationOrder.indexOf(b);
    // If operation not in order list, put it at the end
    if (indexA === -1 && indexB === -1) return a.localeCompare(b);
    if (indexA === -1) return 1;
    if (indexB === -1) return -1;
    return indexA - indexB;
  });
};

// Helper function to sort Auth operations in a consistent order
export const sortAuthOperations = (
  operations: Partial<Record<AuthOperation, AuthOperationRule>>
): Array<[AuthOperation, AuthOperationRule]> => {
  if (!operations) return [];
  const operationOrder = [
    'register', 
    'login', 
    'token_validation', 
    'token_refresh', 
    'get_current_user', 
    'list_auth_collections', 
    'logout'
  ];
  return (Object.entries(operations) as Array<[AuthOperation, AuthOperationRule]>).sort(([a], [b]) => {
    const indexA = operationOrder.indexOf(a);
    const indexB = operationOrder.indexOf(b);
    // If operation not in order list, put it at the end
    if (indexA === -1 && indexB === -1) return a.localeCompare(b);
    if (indexA === -1) return 1;
    if (indexB === -1) return -1;
    return indexA - indexB;
  });
};
