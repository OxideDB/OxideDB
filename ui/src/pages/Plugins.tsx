import React, { useState, useEffect } from 'react';
import { apiService } from '../services/api';
import { Button } from '../components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../components/ui/card';
import { Badge } from '../components/ui/badge';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../components/ui/tabs';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from '../components/ui/dialog';
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, AlertDialogTrigger } from '../components/ui/alert-dialog';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../components/ui/select';
import { ScrollArea } from '../components/ui/scroll-area';
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '../components/ui/dropdown-menu';
import { toast } from '../components/ui/use-toast';
import PageLayout from '../components/PageLayout';
import { 
  Upload, 
  Play, 
  Pause, 
  Trash2, 
  Eye, 
  AlertTriangle, 
  CheckCircle, 
  XCircle,
  Shield,
  Activity,
  Code,
  Globe,
  Clock,
  Cpu,
  HardDrive,
  Minus,
  MoreHorizontal,
  Search
} from 'lucide-react';

interface PluginInfo {
  name: string;
  status: 'Enabled' | 'Disabled' | 'Error' | 'Loading' | 'Uninstalling';
  version: string;
  description: string;
  author: string;
  capabilities: string[];
  trust_level: 'Untrusted' | 'PartiallyTrusted' | 'FullyTrusted' | 'System';
  routes: PluginRoute[];
  executions: number;
  errors: number;
  last_execution?: string;
  resource_usage: ResourceUsage;
}

interface PluginDetails extends PluginInfo {
  audit_log: AuditEntry[];
  permissions?: unknown;
}

interface PluginRoute {
  plugin_name: string;
  method: string;
  path: string;
  handler_function: string;
  permissions?: unknown;
  has_custom_permissions: boolean;
}

interface ResourceUsage {
  memory_bytes: number;
  cpu_time_ms: number;
  api_calls: number;
  storage_bytes: number;
}

interface AuditEntry {
  timestamp: string;
  event_type: string;
  description: string;
  metadata?: unknown;
}

interface PluginAnalysisResult {
  is_valid: boolean;
  plugin_info?: PluginInfo;
  declared_capabilities: string[];
  recommended_trust_level?: string;
  security_info: PluginSecurityInfo;
  size_bytes: number;
  warnings: string[];
  errors: string[];
}

interface PluginSecurityInfo {
  binary_hash: string;
  hash_algorithm: string;
  signature_valid: boolean;
  security_advisories: string[];
  audit_info?: {
    audit_date: string;
    auditor: string;
    report_url?: string;
    status: string;
  };
}

const Plugins: React.FC = () => {
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [selectedPlugin, setSelectedPlugin] = useState<PluginDetails | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [searchTerm, setSearchTerm] = useState('');
  const [installDialogOpen, setInstallDialogOpen] = useState(false);
  const [detailsDialogOpen, setDetailsDialogOpen] = useState(false);

  // Installation form state
  const [installForm, setInstallForm] = useState({
    zipFile: null as File | null,
    trustLevel: 'Untrusted' as string,
    capabilities: [] as string[]
  });

  // Analysis state
  const [analysisResult, setAnalysisResult] = useState<PluginAnalysisResult | null>(null);
  const [analyzeDialogOpen, setAnalyzeDialogOpen] = useState(false);
  const [isAnalyzing, setIsAnalyzing] = useState(false);

  useEffect(() => {
    fetchPlugins();
  }, []);

  const fetchPlugins = async () => {
    try {
      setLoading(true);
      setError(null);
      const plugins = await apiService.getPlugins();
      setPlugins(plugins);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to fetch plugins';
      setError(errorMessage);
      console.error('Error fetching plugins:', err);
    } finally {
      setLoading(false);
    }
  };

  const fetchPluginDetails = async (pluginName: string) => {
    try {
      const pluginDetails = await apiService.getPluginDetails(pluginName);
      setSelectedPlugin(pluginDetails);
      setDetailsDialogOpen(true);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to fetch plugin details';
      toast({
        title: "Error",
        description: errorMessage,
        variant: "destructive"
      });
      console.error('Error fetching plugin details:', err);
    }
  };

  const handleAnalyzePlugin = async () => {
    if (!installForm.zipFile) {
      toast({
        title: "Validation Error",
        description: "Please select a ZIP package to analyze",
        variant: "destructive"
      });
      return;
    }

    setIsAnalyzing(true);

    try {
      const result = await apiService.analyzePlugin(installForm.zipFile);
      setAnalysisResult(result);
      setAnalyzeDialogOpen(true);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to analyze plugin';
      toast({
        title: "Analysis Failed",
        description: errorMessage,
        variant: "destructive"
      });
      console.error('Error analyzing plugin:', err);
    } finally {
      setIsAnalyzing(false);
    }
  };

  const handleInstallPlugin = async () => {
    if (!installForm.zipFile) {
      toast({
        title: "Validation Error",
        description: "Please select a ZIP package to install",
        variant: "destructive"
      });
      return;
    }

    try {
      await apiService.installPlugin(installForm.zipFile, installForm.trustLevel, installForm.capabilities);
      toast({
        title: "Success",
        description: "Plugin installed successfully"
      });
      setInstallDialogOpen(false);
      setInstallForm({
        zipFile: null,
        trustLevel: 'Untrusted',
        capabilities: []
      });
      await fetchPlugins();
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to install plugin';
      toast({
        title: "Installation Failed",
        description: errorMessage,
        variant: "destructive"
      });
      console.error('Error installing plugin:', err);
    }
  };

  const togglePlugin = async (pluginName: string, enable: boolean) => {
    const action = enable ? 'enable' : 'disable';
    
    try {
      if (enable) {
        await apiService.enablePlugin(pluginName);
      } else {
        await apiService.disablePlugin(pluginName);
      }
      toast({
        title: "Success",
        description: `Plugin ${enable ? 'enabled' : 'disabled'} successfully`
      });
      await fetchPlugins();
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : `Failed to ${action} plugin`;
      toast({
        title: "Error",
        description: errorMessage,
        variant: "destructive"
      });
      console.error(`Error ${action}ing plugin:`, err);
    }
  };

  const uninstallPlugin = async (pluginName: string) => {
    try {
      await apiService.uninstallPlugin(pluginName);
      toast({
        title: "Success",
        description: "Plugin uninstalled successfully"
      });
      await fetchPlugins();
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Failed to uninstall plugin';
      toast({
        title: "Uninstall Failed",
        description: errorMessage,
        variant: "destructive"
      });
      console.error('Error uninstalling plugin:', err);
    }
  };

  const getStatusBadge = (status: string) => {
    switch (status) {
      case 'Enabled':
        return <Badge variant="default" className="bg-green-500"><CheckCircle className="w-3 h-3 mr-1" />Enabled</Badge>;
      case 'Disabled':
        return <Badge variant="secondary"><Pause className="w-3 h-3 mr-1" />Disabled</Badge>;
      case 'Error':
        return <Badge variant="destructive"><XCircle className="w-3 h-3 mr-1" />Error</Badge>;
      case 'Loading':
        return <Badge variant="outline"><Activity className="w-3 h-3 mr-1" />Loading</Badge>;
      default:
        return <Badge variant="outline">{status}</Badge>;
    }
  };

  const getTrustLevelBadge = (trustLevel: string) => {
    switch (trustLevel) {
      case 'System':
        return <Badge className="bg-blue-500"><Shield className="w-3 h-3 mr-1" />System</Badge>;
      case 'FullyTrusted':
        return <Badge className="bg-green-600"><Shield className="w-3 h-3 mr-1" />Fully Trusted</Badge>;
      case 'PartiallyTrusted':
        return <Badge className="bg-yellow-500"><Shield className="w-3 h-3 mr-1" />Partially Trusted</Badge>;
      case 'Untrusted':
        return <Badge variant="destructive"><AlertTriangle className="w-3 h-3 mr-1" />Untrusted</Badge>;
      default:
        return <Badge variant="outline">{trustLevel}</Badge>;
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  // Filter plugins based on search term
  const filteredPlugins = plugins.filter(
    (plugin) =>
      plugin.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
      plugin.description.toLowerCase().includes(searchTerm.toLowerCase()) ||
      plugin.author.toLowerCase().includes(searchTerm.toLowerCase())
  );

  if (loading) {
    return (
      <PageLayout title="Plugin Manager" description="Manage your OxideDB plugins">
        <div className="flex items-center justify-center h-64">
          <div className="text-muted-foreground">Loading plugins...</div>
        </div>
      </PageLayout>
    );
  }

  const headerActions = (
    <>
      <div className="relative flex-1 max-w-full sm:max-w-sm">
        <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-muted-foreground" />
        <Input
          placeholder="Search plugins..."
          value={searchTerm}
          onChange={(e) => setSearchTerm(e.target.value)}
          className="pl-10"
        />
      </div>

      <Dialog open={installDialogOpen} onOpenChange={setInstallDialogOpen}>
        <DialogTrigger asChild>
          <Button className="w-full sm:w-auto">
            <Upload className="w-4 h-4 mr-2" />
            Install Plugin
          </Button>
        </DialogTrigger>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Install New Plugin</DialogTitle>
            <DialogDescription>Upload a ZIP plugin package containing plugin.toml and WASM file</DialogDescription>
          </DialogHeader>
          
          <div className="space-y-4">
            <div>
              <Label htmlFor="zip-file">Plugin ZIP Package</Label>
              <Input
                id="zip-file"
                type="file"
                accept=".zip"
                onChange={(e) => setInstallForm({ ...installForm, zipFile: e.target.files?.[0] || null })}
              />
              <p className="text-sm text-muted-foreground mt-1">
                Select a ZIP package containing plugin.toml and WASM file
              </p>
            </div>
            
            <div>
              <Label htmlFor="trust-level">Trust Level</Label>
              <Select value={installForm.trustLevel} onValueChange={(value) => setInstallForm({ ...installForm, trustLevel: value })}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="Untrusted">Untrusted</SelectItem>
                  <SelectItem value="PartiallyTrusted">Partially Trusted</SelectItem>
                  <SelectItem value="FullyTrusted">Fully Trusted</SelectItem>
                </SelectContent>
              </Select>
            </div>
            
            <div className="flex gap-2">
              <Button 
                onClick={handleAnalyzePlugin} 
                variant="outline" 
                disabled={!installForm.zipFile || isAnalyzing}
                className="flex-1"
              >
                {isAnalyzing ? "Analyzing..." : "Analyze Package"}
              </Button>
              <Button 
                onClick={handleInstallPlugin} 
                disabled={!installForm.zipFile}
                className="flex-1"
              >
                Install
              </Button>
            </div>
            <Button variant="ghost" onClick={() => setInstallDialogOpen(false)} className="w-full">
              Cancel
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );

  return (
    <PageLayout
      title="Plugin Manager"
      description="Manage your OxideDB plugins"
      headerActions={headerActions}
    >
      {error && (
        <Card className="border-destructive mb-6">
          <CardContent className="p-4">
            <div className="text-destructive">{error}</div>
            <Button
              onClick={() => setError(null)}
              variant="ghost"
              size="sm"
              className="text-destructive text-sm mt-2 hover:text-destructive/80 p-0 h-auto"
            >
              Dismiss
            </Button>
          </CardContent>
        </Card>
      )}

      {filteredPlugins.length === 0 ? (
        <div className="text-center py-12">
          <Code className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
          <h3 className="text-lg font-medium mb-2">
            {searchTerm ? "No plugins found" : "No Plugins Installed"}
          </h3>
          <p className="text-muted-foreground mb-4 px-4">
            {searchTerm 
              ? "Try adjusting your search terms." 
              : "Get started by installing your first plugin"
            }
          </p>
          {!searchTerm && (
            <Button onClick={() => setInstallDialogOpen(true)}>
              <Upload className="w-4 h-4 mr-2" />
              Install Plugin
            </Button>
          )}
        </div>
      ) : (
        <div className="grid gap-4 grid-cols-1 md:grid-cols-2 xl:grid-cols-3">
          {filteredPlugins.map((plugin) => (
            <Card key={plugin.name} className="hover:shadow-md transition-shadow">
              <CardHeader className="pb-3">
                <div className="flex justify-between items-start">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-3 mb-2">
                      <CardTitle className="text-xl truncate">{plugin.name}</CardTitle>
                      {getStatusBadge(plugin.status)}
                      {getTrustLevelBadge(plugin.trust_level)}
                    </div>
                    <CardDescription className="line-clamp-2">{plugin.description}</CardDescription>
                    <div className="flex items-center gap-4 mt-2 text-sm text-muted-foreground">
                      <span>v{plugin.version}</span>
                      <span>by {plugin.author}</span>
                      <span>{plugin.executions} executions</span>
                      {plugin.errors > 0 && (
                        <span className="text-red-500">{plugin.errors} errors</span>
                      )}
                    </div>
                  </div>
                  
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button variant="ghost" size="sm" className="flex-shrink-0">
                        <MoreHorizontal className="h-4 w-4" />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                      <DropdownMenuItem onClick={() => fetchPluginDetails(plugin.name)}>
                        <Eye className="w-4 h-4 mr-2" />
                        View Details
                      </DropdownMenuItem>
                      
                      {plugin.status === 'Enabled' ? (
                        <DropdownMenuItem onClick={() => togglePlugin(plugin.name, false)}>
                          <Pause className="w-4 h-4 mr-2" />
                          Disable
                        </DropdownMenuItem>
                      ) : (
                        <DropdownMenuItem onClick={() => togglePlugin(plugin.name, true)}>
                          <Play className="w-4 h-4 mr-2" />
                          Enable
                        </DropdownMenuItem>
                      )}
                      
                      <AlertDialog>
                        <AlertDialogTrigger asChild>
                          <DropdownMenuItem className="text-red-600" onSelect={(e) => e.preventDefault()}>
                            <Trash2 className="w-4 h-4 mr-2" />
                            Uninstall
                          </DropdownMenuItem>
                        </AlertDialogTrigger>
                        <AlertDialogContent>
                          <AlertDialogHeader>
                            <AlertDialogTitle>Uninstall Plugin</AlertDialogTitle>
                            <AlertDialogDescription>
                              Are you sure you want to uninstall "{plugin.name}"? This action cannot be undone.
                            </AlertDialogDescription>
                          </AlertDialogHeader>
                          <AlertDialogFooter>
                            <AlertDialogCancel>Cancel</AlertDialogCancel>
                            <AlertDialogAction onClick={() => uninstallPlugin(plugin.name)}>
                              Uninstall
                            </AlertDialogAction>
                          </AlertDialogFooter>
                        </AlertDialogContent>
                      </AlertDialog>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
              </CardHeader>
              
              <CardContent className="space-y-3">
                <div className="grid grid-cols-2 gap-3 text-sm">
                  <div className="flex items-center justify-between">
                    <span className="text-muted-foreground">Memory</span>
                    <span className="font-mono">{formatBytes(plugin.resource_usage.memory_bytes)}</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-muted-foreground">CPU Time</span>
                    <span className="font-mono">{plugin.resource_usage.cpu_time_ms}ms</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-muted-foreground">API Calls</span>
                    <span className="font-mono">{plugin.resource_usage.api_calls}</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-muted-foreground">Routes</span>
                    <span className="font-mono">{plugin.routes.length}</span>
                  </div>
                </div>
                
                {plugin.capabilities.length > 0 && (
                  <div>
                    <div className="text-sm text-muted-foreground mb-2">Capabilities:</div>
                    <div className="flex flex-wrap gap-1">
                      {plugin.capabilities.slice(0, 3).map((cap, index) => (
                        <Badge key={index} variant="outline" className="text-xs">
                          {cap}
                        </Badge>
                      ))}
                      {plugin.capabilities.length > 3 && (
                        <Badge variant="outline" className="text-xs">
                          +{plugin.capabilities.length - 3} more
                        </Badge>
                      )}
                    </div>
                  </div>
                )}
              </CardContent>
            </Card>
          ))}
        </div>
      )}

      {/* Plugin Details Dialog */}
      <Dialog open={detailsDialogOpen} onOpenChange={setDetailsDialogOpen}>
        <DialogContent className="max-w-4xl max-h-[80vh]">
          {selectedPlugin && (
            <>
              <DialogHeader>
                <DialogTitle className="flex items-center gap-3">
                  {selectedPlugin.name}
                  {getStatusBadge(selectedPlugin.status)}
                  {getTrustLevelBadge(selectedPlugin.trust_level)}
                </DialogTitle>
                <DialogDescription>{selectedPlugin.description}</DialogDescription>
              </DialogHeader>
              
              <Tabs defaultValue="overview" className="mt-4">
                <TabsList className="grid w-full grid-cols-4">
                  <TabsTrigger value="overview">Overview</TabsTrigger>
                  <TabsTrigger value="routes">Routes</TabsTrigger>
                  <TabsTrigger value="capabilities">Capabilities</TabsTrigger>
                  <TabsTrigger value="resources">Resources</TabsTrigger>
                </TabsList>
                
                <TabsContent value="overview" className="space-y-4">
                  <div className="grid grid-cols-2 gap-4">
                    <div>
                      <Label>Version</Label>
                      <div className="font-mono">{selectedPlugin.version}</div>
                    </div>
                    <div>
                      <Label>Author</Label>
                      <div>{selectedPlugin.author}</div>
                    </div>
                    <div>
                      <Label>Executions</Label>
                      <div className="font-mono">{selectedPlugin.executions}</div>
                    </div>
                    <div>
                      <Label>Errors</Label>
                      <div className="font-mono text-red-500">{selectedPlugin.errors}</div>
                    </div>
                    <div>
                      <Label>Trust Level</Label>
                      <div>{getTrustLevelBadge(selectedPlugin.trust_level)}</div>
                    </div>
                    <div>
                      <Label>Status</Label>
                      <div>{getStatusBadge(selectedPlugin.status)}</div>
                    </div>
                  </div>
                  
                  <div className="space-y-3">
                    <div>
                      <Label>Description</Label>
                      <div className="text-sm">{selectedPlugin.description}</div>
                    </div>
                    
                    {selectedPlugin.routes.length > 0 && (
                      <div>
                        <Label>HTTP Routes</Label>
                        <div className="text-sm text-muted-foreground">
                          {selectedPlugin.routes.length} route{selectedPlugin.routes.length !== 1 ? 's' : ''} registered
                        </div>
                      </div>
                    )}
                  </div>
                </TabsContent>
                
                <TabsContent value="routes">
                  <ScrollArea className="h-64">
                    {selectedPlugin.routes.length === 0 ? (
                      <div className="text-center text-muted-foreground py-8">
                        No HTTP routes registered
                      </div>
                    ) : (
                      <div className="space-y-2">
                        {selectedPlugin.routes.map((route, index) => (
                          <Card key={index}>
                            <CardContent className="p-3">
                              <div className="flex items-center justify-between">
                                <div>
                                  <Badge variant="outline" className="mr-2">{route.method}</Badge>
                                  <code className="text-sm">{route.path}</code>
                                </div>
                                <div className="text-sm text-muted-foreground">
                                  {route.handler_function}
                                </div>
                              </div>
                            </CardContent>
                          </Card>
                        ))}
                      </div>
                    )}
                  </ScrollArea>
                </TabsContent>
                
                <TabsContent value="capabilities">
                  <ScrollArea className="h-64">
                    <div className="grid gap-2">
                      {selectedPlugin.capabilities.map((capability, index) => (
                        <div key={index} className="flex items-center justify-between p-2 border rounded">
                          <span className="font-mono text-sm">{capability}</span>
                          <Button variant="outline" size="sm">
                            <Minus className="w-3 h-3" />
                          </Button>
                        </div>
                      ))}
                    </div>
                  </ScrollArea>
                </TabsContent>
                
                <TabsContent value="resources">
                  <div className="grid grid-cols-2 gap-4">
                    <Card>
                      <CardHeader className="pb-2">
                        <CardTitle className="text-sm flex items-center">
                          <Cpu className="w-4 h-4 mr-2" />
                          Memory Usage
                        </CardTitle>
                      </CardHeader>
                      <CardContent>
                        <div className="text-2xl font-mono">
                          {formatBytes(selectedPlugin.resource_usage.memory_bytes)}
                        </div>
                      </CardContent>
                    </Card>
                    
                    <Card>
                      <CardHeader className="pb-2">
                        <CardTitle className="text-sm flex items-center">
                          <Clock className="w-4 h-4 mr-2" />
                          CPU Time
                        </CardTitle>
                      </CardHeader>
                      <CardContent>
                        <div className="text-2xl font-mono">
                          {selectedPlugin.resource_usage.cpu_time_ms}ms
                        </div>
                      </CardContent>
                    </Card>
                    
                    <Card>
                      <CardHeader className="pb-2">
                        <CardTitle className="text-sm flex items-center">
                          <Globe className="w-4 h-4 mr-2" />
                          API Calls
                        </CardTitle>
                      </CardHeader>
                      <CardContent>
                        <div className="text-2xl font-mono">
                          {selectedPlugin.resource_usage.api_calls}
                        </div>
                      </CardContent>
                    </Card>
                    
                    <Card>
                      <CardHeader className="pb-2">
                        <CardTitle className="text-sm flex items-center">
                          <HardDrive className="w-4 h-4 mr-2" />
                          Storage
                        </CardTitle>
                      </CardHeader>
                      <CardContent>
                        <div className="text-2xl font-mono">
                          {formatBytes(selectedPlugin.resource_usage.storage_bytes)}
                        </div>
                      </CardContent>
                    </Card>
                  </div>
                </TabsContent>
              </Tabs>
            </>
          )}
        </DialogContent>
      </Dialog>

      {/* Plugin Analysis Result Dialog */}
      <Dialog open={analyzeDialogOpen} onOpenChange={setAnalyzeDialogOpen}>
        <DialogContent className="max-w-2xl max-h-[80vh]">
          {analysisResult && (
            <>
              <DialogHeader>
                <DialogTitle className="flex items-center gap-2">
                  Plugin Analysis Results
                  {analysisResult.is_valid ? (
                    <CheckCircle className="w-5 h-5 text-green-500" />
                  ) : (
                    <XCircle className="w-5 h-5 text-red-500" />
                  )}
                </DialogTitle>
                <DialogDescription>
                  Security and compatibility analysis for the plugin package
                </DialogDescription>
              </DialogHeader>
              
              <ScrollArea className="max-h-96">
                <div className="space-y-4">
                  {/* Plugin Info */}
                  {analysisResult.plugin_info && (
                    <Card>
                      <CardHeader className="pb-2">
                        <CardTitle className="text-sm">Plugin Information</CardTitle>
                      </CardHeader>
                      <CardContent className="space-y-2">
                        <div className="grid grid-cols-2 gap-2 text-sm">
                          <div><strong>Name:</strong> {analysisResult.plugin_info.name}</div>
                          <div><strong>Version:</strong> {analysisResult.plugin_info.version}</div>
                          <div><strong>Author:</strong> {analysisResult.plugin_info.author}</div>
                          <div><strong>Size:</strong> {formatBytes(analysisResult.size_bytes)}</div>
                        </div>
                        <div className="text-sm">
                          <strong>Description:</strong> {analysisResult.plugin_info.description}
                        </div>
                      </CardContent>
                    </Card>
                  )}

                  {/* Security Information */}
                  <Card>
                    <CardHeader className="pb-2">
                      <CardTitle className="text-sm flex items-center">
                        <Shield className="w-4 h-4 mr-2" />
                        Security Information
                      </CardTitle>
                    </CardHeader>
                    <CardContent className="space-y-2">
                      <div className="grid grid-cols-2 gap-2 text-sm">
                        <div className="flex items-center">
                          <strong>Signature:</strong>
                          <span className={`ml-2 ${analysisResult.security_info.signature_valid ? 'text-green-600' : 'text-yellow-600'}`}>
                            {analysisResult.security_info.signature_valid ? 'Valid' : 'Not verified'}
                          </span>
                        </div>
                        <div>
                          <strong>Hash:</strong> 
                          <code className="ml-2 text-xs">{analysisResult.security_info.binary_hash.slice(0, 16)}...</code>
                        </div>
                      </div>
                      
                      {analysisResult.security_info.security_advisories.length > 0 && (
                        <div>
                          <strong>Security Advisories:</strong>
                          <ul className="list-disc list-inside text-sm text-yellow-600">
                            {analysisResult.security_info.security_advisories.map((advisory, idx) => (
                              <li key={idx}>{advisory}</li>
                            ))}
                          </ul>
                        </div>
                      )}
                    </CardContent>
                  </Card>

                  {/* Capabilities */}
                  <Card>
                    <CardHeader className="pb-2">
                      <CardTitle className="text-sm">Declared Capabilities</CardTitle>
                    </CardHeader>
                    <CardContent>
                      <div className="flex flex-wrap gap-1">
                        {analysisResult.declared_capabilities.map((cap, idx) => (
                          <Badge key={idx} variant="outline" className="text-xs">
                            {cap}
                          </Badge>
                        ))}
                      </div>
                      {analysisResult.recommended_trust_level && (
                        <div className="mt-2 text-sm">
                          <strong>Recommended Trust Level:</strong> 
                          <Badge className="ml-2" variant="secondary">
                            {analysisResult.recommended_trust_level}
                          </Badge>
                        </div>
                      )}
                    </CardContent>
                  </Card>

                  {/* Warnings and Errors */}
                  {(analysisResult.warnings.length > 0 || analysisResult.errors.length > 0) && (
                    <Card>
                      <CardHeader className="pb-2">
                        <CardTitle className="text-sm flex items-center">
                          <AlertTriangle className="w-4 h-4 mr-2" />
                          Issues Found
                        </CardTitle>
                      </CardHeader>
                      <CardContent className="space-y-2">
                        {analysisResult.errors.length > 0 && (
                          <div>
                            <strong className="text-red-600">Errors:</strong>
                            <ul className="list-disc list-inside text-sm text-red-600">
                              {analysisResult.errors.map((error, idx) => (
                                <li key={idx}>{error}</li>
                              ))}
                            </ul>
                          </div>
                        )}
                        {analysisResult.warnings.length > 0 && (
                          <div>
                            <strong className="text-yellow-600">Warnings:</strong>
                            <ul className="list-disc list-inside text-sm text-yellow-600">
                              {analysisResult.warnings.map((warning, idx) => (
                                <li key={idx}>{warning}</li>
                              ))}
                            </ul>
                          </div>
                        )}
                      </CardContent>
                    </Card>
                  )}
                </div>
              </ScrollArea>
              
              <div className="flex gap-2 pt-4">
                <Button
                  onClick={() => {
                    setAnalyzeDialogOpen(false);
                    if (analysisResult.is_valid && analysisResult.plugin_info) {
                      // Pre-fill installation form with analysis results
                      if (analysisResult.recommended_trust_level) {
                        setInstallForm(prev => ({
                          ...prev,
                          trustLevel: analysisResult.recommended_trust_level!,
                          capabilities: analysisResult.declared_capabilities
                        }));
                      }
                      setInstallDialogOpen(true);
                    }
                  }}
                  disabled={!analysisResult.is_valid}
                  className="flex-1"
                >
                  Proceed to Install
                </Button>
                <Button variant="outline" onClick={() => setAnalyzeDialogOpen(false)} className="flex-1">
                  Close
                </Button>
              </div>
            </>
          )}
        </DialogContent>
      </Dialog>
    </PageLayout>
  );
};

export default Plugins; 