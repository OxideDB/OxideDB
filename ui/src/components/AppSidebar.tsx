import { Database, Shield, Settings, BarChart3, FileText, Key, Eye, EyeOff, Activity, User, LogOut } from "lucide-react"
import { useState } from "react"
import { Link, useLocation } from "react-router-dom"

import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
} from "@/components/ui/sidebar"
import { Switch } from "@/components/ui/switch"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Separator } from "@/components/ui/separator"
import { ThemeToggle } from "@/components/theme-toggle"
import { useAuth } from "@/contexts/AuthContext"

const staticNavigationItems = [
  {
    title: "Overview",
    items: [
      {
        title: "Dashboard",
        url: "/",
        icon: BarChart3,
      },
    ],
  },
  {
    title: "Database Management",
    items: [
      {
        title: "Collections",
        url: "/collections",
        icon: Database,
      },
      {
        title: "Health",
        url: "/health",
        icon: Activity,
      },
    ],
  },
  {
    title: "Access Control",
    items: [
      {
        title: "Permissions",
        url: "/permissions",
        icon: Shield,
      },
      {
        title: "API Keys",
        url: "/api-keys",
        icon: Key,
      },
    ],
  },
  {
    title: "System",
    items: [
      {
        title: "Settings",
        url: "/settings",
        icon: Settings,
      },
    ],
  },
]

const userCollections = [
  { name: "users", recordCount: 1247 },
  { name: "products", recordCount: 856 },
  { name: "orders", recordCount: 2341 },
  { name: "analytics", recordCount: 15623 },
]

const systemCollections = [
  { name: "_users", recordCount: 23 },
  { name: "_permissions", recordCount: 45 },
  { name: "_api_keys", recordCount: 12 },
  { name: "_audit_logs", recordCount: 8934 },
]

export function AppSidebar() {
  const [showSystemCollections, setShowSystemCollections] = useState(false)
  const { user, logout } = useAuth()
  const location = useLocation()

  const collectionsToShow = showSystemCollections ? [...userCollections, ...systemCollections] : userCollections

  const handleLogout = async () => {
    try {
      await logout();
    } catch (error) {
      console.error('Logout failed:', error);
      window.location.href = '/login';
    }
  };

  return (
    <Sidebar>
      <SidebarHeader className="border-b border-sidebar-border">
        <div className="flex items-center justify-between px-4 py-2">
          <div className="flex items-center gap-2">
            <Database className="h-6 w-6 text-orange-500" />
            <span className="font-semibold text-lg">OxideDB</span>
          </div>
          <ThemeToggle />
        </div>
      </SidebarHeader>
      <SidebarContent>
        {staticNavigationItems.map((section) => (
          <SidebarGroup key={section.title}>
            <SidebarGroupLabel>{section.title}</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                {section.items.map((item) => {
                  const isActive = location.pathname === item.url || 
                                 (item.url === '/collections' && location.pathname.startsWith('/collections'));
                  return (
                    <SidebarMenuItem key={item.title}>
                      <SidebarMenuButton asChild isActive={isActive}>
                        <Link to={item.url}>
                          <item.icon className="h-4 w-4" />
                          <span>{item.title}</span>
                        </Link>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  )
                })}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        ))}

        {/* Collections Section */}
        <SidebarGroup>
          <SidebarGroupLabel className="flex items-center justify-between">
            <span>Collections</span>
            <div className="flex items-center space-x-2">
              <Switch
                id="show-system"
                checked={showSystemCollections}
                onCheckedChange={setShowSystemCollections}
                className="scale-75"
              />
              <Label htmlFor="show-system" className="text-xs cursor-pointer">
                {showSystemCollections ? <Eye className="h-3 w-3" /> : <EyeOff className="h-3 w-3" />}
              </Label>
            </div>
          </SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              {collectionsToShow.map((collection) => (
                <SidebarMenuItem key={collection.name}>
                  <SidebarMenuButton asChild>
                    <Link to={`/collections/${collection.name}`}>
                      <FileText className="h-4 w-4" />
                      <span className={collection.name.startsWith("_") ? "text-muted-foreground" : ""}>
                        {collection.name}
                      </span>
                      <span className="ml-auto text-xs text-muted-foreground">{collection.recordCount}</span>
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      
      {/* User Info and Logout */}
      <div className="mt-auto border-t border-sidebar-border">
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
        <div className="p-3">
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
      
      <SidebarRail />
    </Sidebar>
  )
} 