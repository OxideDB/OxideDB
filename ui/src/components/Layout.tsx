import React from 'react';
import { Outlet, Link, useLocation } from 'react-router-dom';
import { Database, Activity, Settings, LogOut, User, Shield } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';
import { ThemeToggle } from './theme-toggle';
import { useAuth } from '../hooks/useAuth';

const Layout: React.FC = () => {
  const location = useLocation();
  const { user, logout } = useAuth();

  const navigation = [
    { name: 'Collections', href: '/collections', icon: Database },
    { name: 'Health', href: '/health', icon: Activity },
    { name: 'Permissions', href: '/permissions', icon: Shield },
    { name: 'Settings', href: '/settings', icon: Settings },
  ];

  const handleLogout = async () => {
    try {
      await logout();
      // Redirect will be handled by ProtectedRoute
    } catch (error) {
      console.error('Logout failed:', error);
      // Force redirect even if logout fails
      window.location.href = '/login';
    }
  };

  return (
    <div className="h-full flex bg-background">
      {/* Sidebar */}
      <div className="flex flex-col w-64 border-r bg-card">
        <div className="flex items-center justify-between h-16 px-4">
          <h1 className="text-xl font-bold text-primary">OxideDB Admin</h1>
        </div>
        <Separator />
        
        <nav className="flex-1 px-3 py-4 space-y-1">
          {navigation.map((item) => {
            const Icon = item.icon;
            const isActive = location.pathname === item.href || 
                           (item.href === '/collections' && location.pathname.startsWith('/collections'));
            
            return (
              <Button
                key={item.name}
                asChild
                variant={isActive ? "default" : "ghost"}
                className={cn(
                  "w-full justify-start",
                  isActive && "bg-primary text-primary-foreground"
                )}
              >
                <Link to={item.href}>
                  <Icon className="mr-3 h-4 w-4" />
                  {item.name}
                </Link>
              </Button>
            );
          })}
        </nav>

        <Separator />
        
        {/* User Info */}
        {user && (
          <div className="p-3">
            <div className="flex items-center space-x-3 p-2 rounded-lg bg-muted/50">
              <div className="flex-shrink-0">
                <div className="w-8 h-8 bg-primary/10 rounded-full flex items-center justify-center">
                  <User className="h-4 w-4 text-primary" />
                </div>
              </div>
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium text-foreground truncate">
                  {user.email}
                </p>
                <div className="flex items-center space-x-1 mt-1">
                  <Badge 
                    variant={user.is_superuser ? "default" : "secondary"}
                    className="text-xs"
                  >
                    {user.is_superuser ? 'Superuser' : 'User'}
                  </Badge>
                </div>
              </div>
            </div>
          </div>
        )}
        
        <Separator />
        <div className="p-3 space-y-2">
          <div className="flex justify-center">
            <ThemeToggle />
          </div>
          <Button
            onClick={handleLogout}
            variant="ghost"
            className="w-full justify-start text-muted-foreground hover:text-foreground"
          >
            <LogOut className="mr-3 h-4 w-4" />
            Logout
          </Button>
        </div>
      </div>

      {/* Main content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        <main className="flex-1 overflow-x-hidden overflow-y-auto bg-background">
          <div className="px-6 py-8">
            <Outlet />
          </div>
        </main>
      </div>
    </div>
  );
};

export default Layout;
