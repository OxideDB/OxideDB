import React from 'react';
import { Link, useLocation, Outlet } from 'react-router-dom';
import { Database, Settings, Activity, LogOut } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';
import { ThemeToggle } from './theme-toggle';
import { cn } from '@/lib/utils';

const Layout: React.FC = () => {
  const location = useLocation();

  const navigation = [
    { name: 'Collections', href: '/collections', icon: Database },
    { name: 'Health', href: '/health', icon: Activity },
    { name: 'Settings', href: '/settings', icon: Settings },
  ];

  const handleLogout = () => {
    // For now, just redirect to login
    // In a real app, this would clear auth tokens
    window.location.href = '/login';
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