import { Database, Shield, Settings, BarChart3, FileText, Key, Eye, EyeOff, Activity, User, LogOut, FileSearch, Puzzle, RefreshCw } from "lucide-react"
import { useState, useEffect } from "react"
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
import { apiService } from "@/services/api"
import type { CollectionSchema, CollectionStats } from "@/types/api"

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
      {
        title: "Logs",
        url: "/logs",
        icon: FileSearch,
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
        title: "Plugins",
        url: "/plugins",
        icon: Puzzle,
      },
      {
        title: "Settings",
        url: "/settings",
        icon: Settings,
      },
    ],
  },
]

interface CollectionWithStats {
  schema: CollectionSchema;
  stats?: CollectionStats;
}

export function AppSidebar() {
  const [showSystemCollections, setShowSystemCollections] = useState(false)
  const [collections, setCollections] = useState<CollectionWithStats[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const { user, logout } = useAuth()
  const location = useLocation()

  // Load collections data
  useEffect(() => {
    fetchCollections()
  }, [])

  const fetchCollections = async () => {
    try {
      setLoading(true)
      setError(null)
      const collectionsData = await apiService.getCollections()
      
      // Fetch stats for each collection in parallel
      const collectionsWithStats = await Promise.all(
        collectionsData.map(async (schema): Promise<CollectionWithStats> => {
          try {
            const stats = await apiService.getCollectionStats(schema.name)
            return { schema, stats }
          } catch (err) {
            console.warn(`Failed to fetch stats for collection ${schema.name}:`, err)
            return { schema }
          }
        })
      )
      
      setCollections(collectionsWithStats)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch collections')
      console.error('Error fetching collections:', err)
    } finally {
      setLoading(false)
    }
  }

  // Filter collections based on whether we want to show system collections
  const userCollections = collections.filter(({ schema }) => 
    !apiService.isSystemCollection(schema)
  )
  const systemCollections = collections.filter(({ schema }) => 
    apiService.isSystemCollection(schema)
  )
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
            <div className="flex items-center space-x-1">
              <Button
                variant="ghost"
                size="sm"
                onClick={fetchCollections}
                disabled={loading}
                className="h-6 w-6 p-0"
                title="Refresh collections"
              >
                <RefreshCw className={`h-3 w-3 ${loading ? 'animate-spin' : ''}`} />
              </Button>
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
            {error && (
              <div className="px-2 py-1 text-xs text-red-500 bg-red-50 dark:bg-red-950 rounded mb-2">
                {error}
              </div>
            )}
            {loading && collections.length === 0 ? (
              <div className="px-2 py-1 text-xs text-muted-foreground">
                Loading collections...
              </div>
            ) : (
              <SidebarMenu>
                {collectionsToShow.length === 0 ? (
                  <div className="px-2 py-1 text-xs text-muted-foreground">
                    {showSystemCollections ? 'No collections found' : 'No user collections'}
                  </div>
                ) : (
                  collectionsToShow.map((collection) => {
                    const isSystemCollection = apiService.isSystemCollection(collection.schema)
                    return (
                      <SidebarMenuItem key={collection.schema.id}>
                        <SidebarMenuButton asChild>
                          <Link to={`/collections/${encodeURIComponent(collection.schema.name)}`}>
                            {isSystemCollection ? (
                              <Shield className="h-4 w-4 text-orange-500" />
                            ) : (
                              <FileText className="h-4 w-4" />
                            )}
                            <span className={isSystemCollection ? "text-muted-foreground" : ""}>
                              {collection.schema.name}
                            </span>
                            <span className="ml-auto text-xs text-muted-foreground">
                              {collection.stats?.record_count.toLocaleString() || '?'}
                            </span>
                          </Link>
                        </SidebarMenuButton>
                      </SidebarMenuItem>
                    )
                  })
                )}
              </SidebarMenu>
            )}
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