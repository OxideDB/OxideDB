import React from 'react';
import { Navigate, useLocation } from 'react-router-dom';
import { useAuth } from '../contexts/AuthContext';
import { Database } from 'lucide-react';

interface ProtectedRouteProps {
  children: React.ReactNode;
  requireSuperuser?: boolean;
}

const ProtectedRoute: React.FC<ProtectedRouteProps> = ({ 
  children, 
  requireSuperuser = false 
}) => {
  const { isAuthenticated, isLoading, user } = useAuth();
  const location = useLocation();

  // Show loading spinner while checking authentication
  if (isLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-background">
        <div className="text-center space-y-4">
          <div className="flex justify-center">
            <div className="p-3 bg-primary/10 rounded-full">
              <Database className="h-8 w-8 text-primary animate-pulse" />
            </div>
          </div>
          <div className="text-muted-foreground">
            Loading...
          </div>
        </div>
      </div>
    );
  }

  // Redirect to login if not authenticated
  if (!isAuthenticated) {
    return (
      <Navigate 
        to="/login" 
        state={{ from: location }} 
        replace 
      />
    );
  }

  // Check superuser requirement
  if (requireSuperuser && user && !user.is_superuser) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-background px-4">
        <div className="text-center space-y-4 max-w-md">
          <div className="flex justify-center">
            <div className="p-3 bg-destructive/10 rounded-full">
              <Database className="h-8 w-8 text-destructive" />
            </div>
          </div>
          <div>
            <h1 className="text-2xl font-bold text-foreground mb-2">
              Access Denied
            </h1>
            <p className="text-muted-foreground">
              You need superuser privileges to access this page.
            </p>
          </div>
        </div>
      </div>
    );
  }

  return <>{children}</>;
};

export default ProtectedRoute;