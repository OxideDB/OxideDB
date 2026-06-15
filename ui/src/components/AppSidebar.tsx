import { Archive, Database, Shield, Settings, BarChart3, FileText, Key, Eye, EyeOff, Activity, User, LogOut, FileSearch, Puzzle, RefreshCw } from "lucide-react"
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
import { useAuth } from "@/hooks/useAuth"
import { useSiteSettings } from "@/hooks/useSiteSettings"
import { apiService } from "@/services/api"
import { cn } from "@/lib/utils"
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
        title: "Backups",
        url: "/backups",
        icon: Archive,
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
  const { branding, settings } = useSiteSettings()
  const location = useLocation()
  const siteTitle = branding.site_title?.trim() || 'OxideDB'
  const subtitle = settings?.system_info.instance_name?.trim() || 'Admin console'

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

  const isRouteActive = (url: string) => {
    if (url === "/") {
      return location.pathname === "/" || location.pathname === "/dashboard";
    }

    return location.pathname === url || location.pathname.startsWith(`${url}/`);
  };

  return (
    <Sidebar collapsible="icon" className="border-r border-sidebar-border">
      <SidebarHeader className="border-b border-sidebar-border px-2 py-3">
        <div className="flex items-center justify-between gap-2 px-2">
          <div className="flex min-w-0 items-center gap-2">
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-primary text-primary-foreground shadow-sm">
              {branding.logo_url ? (
                <img
                  src={branding.logo_url}
                  alt={`${siteTitle} logo`}
                  className="h-5 w-5 object-contain"
                />
              ) : (
                <Database className="h-4 w-4" />
              )}
            </div>
            <div className="min-w-0 group-data-[collapsible=icon]:hidden">
              <div className="truncate text-sm font-semibold leading-5">{siteTitle}</div>
              <div className="truncate text-xs text-sidebar-foreground/60">{subtitle}</div>
            </div>
          </div>
          <div className="group-data-[collapsible=icon]:hidden">
            <ThemeToggle />
          </div>
        </div>
      </SidebarHeader>
      <SidebarContent className="gap-1 py-2">
        {staticNavigationItems.map((section) => (
          <SidebarGroup key={section.title} className="px-2 py-1">
            <SidebarGroupLabel className="px-2 text-[0.68rem] font-semibold uppercase tracking-wide text-sidebar-foreground/55">
              {section.title}
            </SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                {section.items.map((item) => {
                  const isActive = isRouteActive(item.url);
                  return (
                    <SidebarMenuItem key={item.title}>
                      <SidebarMenuButton asChild isActive={isActive} tooltip={item.title}>
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
        <SidebarGroup className="px-2 py-1">
          <SidebarGroupLabel className="flex items-center justify-between">
            <span className="text-[0.68rem] font-semibold uppercase tracking-wide text-sidebar-foreground/55">
              Collections
            </span>
            <div className="flex items-center space-x-1 group-data-[collapsible=icon]:hidden">
              <Button
                variant="ghost"
                size="sm"
                onClick={fetchCollections}
                disabled={loading}
                className="h-6 w-6 p-0 text-sidebar-foreground/70 hover:bg-sidebar-accent"
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
              <div className="mb-2 rounded-md border border-destructive/20 bg-destructive/10 px-2 py-1.5 text-xs text-destructive group-data-[collapsible=icon]:hidden">
                {error}
              </div>
            )}
            {loading && collections.length === 0 ? (
              <div className="px-2 py-1.5 text-xs text-sidebar-foreground/60 group-data-[collapsible=icon]:hidden">
                Loading collections...
              </div>
            ) : (
              <SidebarMenu>
                {collectionsToShow.length === 0 ? (
                  <div className="px-2 py-1.5 text-xs text-sidebar-foreground/60 group-data-[collapsible=icon]:hidden">
                    {showSystemCollections ? 'No collections found' : 'No user collections'}
                  </div>
                ) : (
                  collectionsToShow.map((collection) => {
                    const isSystemCollection = apiService.isSystemCollection(collection.schema)
                    const collectionUrl = `/collections/${encodeURIComponent(collection.schema.name)}`;
                    const isActive = location.pathname === collectionUrl || location.pathname.startsWith(`${collectionUrl}/`);
                    return (
                      <SidebarMenuItem key={collection.schema.id}>
                        <SidebarMenuButton asChild isActive={isActive} tooltip={collection.schema.name}>
                          <Link to={collectionUrl}>
                            {isSystemCollection ? (
                              <Shield className="h-4 w-4 text-warning" />
                            ) : (
                              <FileText className="h-4 w-4" />
                            )}
                            <span className={cn(isSystemCollection && "text-sidebar-foreground/70")}>
                              {collection.schema.name}
                            </span>
                            <span className="ml-auto text-xs tabular-nums text-sidebar-foreground/55 group-data-[collapsible=icon]:hidden">
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
          <div className="p-2">
            <div className="flex items-center gap-3 rounded-lg border border-sidebar-border bg-sidebar-accent/70 p-2">
              <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary">
                <User className="h-4 w-4" />
              </div>
              <div className="min-w-0 flex-1 group-data-[collapsible=icon]:hidden">
                <p className="truncate text-sm font-medium text-sidebar-foreground">
                  {user.email}
                </p>
                <div className="mt-1 flex items-center space-x-1">
                  <Badge 
                    variant={user.is_superuser ? "default" : "secondary"}
                    className="text-xs font-medium"
                  >
                    {user.is_superuser ? 'Superuser' : 'User'}
                  </Badge>
                </div>
              </div>
            </div>
          </div>
        )}
        
        <Separator />
        <div className="p-2">
          <Button
            onClick={handleLogout}
            variant="ghost"
            className="w-full justify-start text-sidebar-foreground/70 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
            title="Logout"
          >
            <LogOut className="mr-3 h-4 w-4" />
            <span className="group-data-[collapsible=icon]:hidden">Logout</span>
          </Button>
        </div>
      </div>
      
      <SidebarRail />
    </Sidebar>
  )
} 
