import React, { useState, useEffect } from 'react';
import { apiService } from '../services/api';
import { getCapabilityName, type PluginAdminPage, type PluginCapability } from '../types/api';
import { Button } from '../components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../components/ui/card';
import { Badge } from '../components/ui/badge';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../components/ui/tabs';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from '../components/ui/dialog';
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, AlertDialogTrigger } from '../components/ui/alert-dialog';
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
  capabilities: PluginCapability[];
  trust_level: 'Untrusted' | 'PartiallyTrusted' | 'FullyTrusted' | 'System';
  routes: PluginRoute[];
  admin_pages: PluginAdminPage[];
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

interface PluginInstallNotice {
  severity: 'Info' | 'Warning';
  message: string;
}

interface PluginInstallResult {
  plugin_info: PluginInfo;
  applied_trust_level: 'Untrusted' | 'PartiallyTrusted' | 'FullyTrusted' | 'System';
  applied_capabilities: PluginCapability[];
  declared_capabilities: string[];
  security_info: PluginSecurityInfo;
  size_bytes: number;
  notices: PluginInstallNotice[];
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
    zipFile: null as File | null
  });
  const [installReview, setInstallReview] = useState<PluginAnalysisResult | null>(null);
  const [installResult, setInstallResult] = useState<PluginInstallResult | null>(null);
  const [isInstalling, setIsInstalling] = useState(false);

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

  const analysisTrustLevel = (analysis: PluginAnalysisResult) => {
    const recommendedTrust = analysis.recommended_trust_level || analysis.plugin_info?.trust_level || 'Untrusted';
    return recommendedTrust === 'System' ? 'FullyTrusted' : recommendedTrust;
  };

  const reviewReasonsForAnalysis = (analysis: PluginAnalysisResult): PluginInstallNotice[] => {
    const notices: PluginInstallNotice[] = [];
    const trustLevel = analysisTrustLevel(analysis);
    const requestedTrust = analysis.recommended_trust_level || analysis.plugin_info?.trust_level;

    if (!analysis.security_info.signature_valid) {
      notices.push({
        severity: 'Warning',
        message: 'Package signature is not verified. Review the source before installing.',
      });
    }

    if (requestedTrust === 'System') {
      notices.push({
        severity: 'Warning',
        message: 'Package requested System trust. OxideDB will install it as FullyTrusted, which should be reviewed before install.',
      });
    } else if (trustLevel === 'FullyTrusted') {
      notices.push({
        severity: 'Warning',
        message: 'Package will install with FullyTrusted scope. Review the requested capabilities before installing.',
      });
    }

    return notices;
  };

  const handleInstallPlugin = async (reviewAccepted = false) => {
    if (!installForm.zipFile) {
      toast({
        title: "Validation Error",
        description: "Please select a ZIP package to install",
        variant: "destructive"
      });
      return;
    }

    setIsInstalling(true);
    setInstallResult(null);

    try {
      if (!reviewAccepted) {
        const analysis = await apiService.analyzePlugin(installForm.zipFile);
        const reviewReasons = reviewReasonsForAnalysis(analysis);

        if (!analysis.is_valid) {
          toast({
            title: "Package Review Failed",
            description: analysis.errors[0] || 'Plugin package is not valid for installation',
            variant: "destructive"
          });
          return;
        }

        if (reviewReasons.length > 0) {
          setInstallReview(analysis);
          return;
        }
      }

      const result = await apiService.installPlugin(installForm.zipFile);
      setInstallResult(result);
      setInstallReview(null);
      toast({
        title: "Plugin Installed",
        description: `${result.plugin_info.name} installed with ${result.applied_trust_level} trust`
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
    } finally {
      setIsInstalling(false);
    }
  };

  const resetInstallDialog = () => {
    setInstallForm({ zipFile: null });
    setInstallReview(null);
    setInstallResult(null);
    setIsInstalling(false);
  };

  const handleInstallDialogOpenChange = (open: boolean) => {
    setInstallDialogOpen(open);
    if (!open) {
      resetInstallDialog();
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

  const formatCapability = (capability: PluginCapability) => getCapabilityName(capability);

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

      <Dialog open={installDialogOpen} onOpenChange={handleInstallDialogOpenChange}>
        <DialogTrigger asChild>
          <Button className="w-full sm:w-auto">
            <Upload className="w-4 h-4 mr-2" />
            Install Plugin
          </Button>
        </DialogTrigger>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>
              {installResult
                ? "Plugin Installed"
                : installReview
                  ? "Review Plugin Install"
                  : "Install New Plugin"}
            </DialogTitle>
            <DialogDescription>
              {installResult
                ? "Review the trust scope and package notices from the automatic installation"
                : installReview
                  ? "This package needs your review before installation continues"
                  : "Upload a ZIP plugin package containing plugin.toml and WASM file"}
            </DialogDescription>
          </DialogHeader>
          
          {installResult ? (
            <div className="space-y-4">
              <div className="rounded-md border p-4">
                <div className="flex flex-wrap items-start gap-3">
                  <CheckCircle className="h-5 w-5 text-green-600 mt-0.5" />
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <div className="font-medium truncate">{installResult.plugin_info.name}</div>
                      <Badge variant="outline">v{installResult.plugin_info.version}</Badge>
                      {getTrustLevelBadge(installResult.applied_trust_level)}
                    </div>
                    <div className="text-sm text-muted-foreground mt-1">
                      by {installResult.plugin_info.author}
                    </div>
                    <p className="text-sm mt-2">{installResult.plugin_info.description}</p>
                  </div>
                </div>
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 text-sm">
                <div className="rounded-md border p-3">
                  <div className="text-muted-foreground">Trust</div>
                  <div className="font-medium mt-1">{installResult.applied_trust_level}</div>
                </div>
                <div className="rounded-md border p-3">
                  <div className="text-muted-foreground">Signature</div>
                  <div
                    className={
                      installResult.security_info.signature_valid
                        ? "text-green-600 font-medium mt-1"
                        : "text-yellow-600 font-medium mt-1"
                    }
                  >
                    {installResult.security_info.signature_valid ? "Verified" : "Not verified"}
                  </div>
                </div>
                <div className="rounded-md border p-3">
                  <div className="text-muted-foreground">Package</div>
                  <div className="font-medium mt-1">{formatBytes(installResult.size_bytes)}</div>
                </div>
              </div>

              <div>
                <Label>Applied Capabilities</Label>
                <ScrollArea className="mt-2 max-h-28 rounded-md border p-3">
                  <div className="flex flex-wrap gap-1">
                    {installResult.applied_capabilities.length === 0 ? (
                      <span className="text-sm text-muted-foreground">No capabilities requested</span>
                    ) : (
                      installResult.applied_capabilities.map((capability, index) => (
                        <Badge key={index} variant="outline" className="text-xs">
                          {formatCapability(capability)}
                        </Badge>
                      ))
                    )}
                  </div>
                </ScrollArea>
              </div>

              {installResult.notices.length > 0 && (
                <div>
                  <Label>Notices</Label>
                  <div className="mt-2 space-y-2">
                    {installResult.notices.map((notice, index) => (
                      <div key={index} className="flex gap-2 rounded-md border p-3 text-sm">
                        {notice.severity === 'Warning' ? (
                          <AlertTriangle className="h-4 w-4 text-yellow-600 mt-0.5 flex-shrink-0" />
                        ) : (
                          <CheckCircle className="h-4 w-4 text-green-600 mt-0.5 flex-shrink-0" />
                        )}
                        <span>{notice.message}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              <Button onClick={() => handleInstallDialogOpenChange(false)} className="w-full">
                Done
              </Button>
            </div>
          ) : installReview ? (
            <div className="space-y-4">
              <div className="rounded-md border border-yellow-500/50 p-4">
                <div className="flex gap-3">
                  <AlertTriangle className="h-5 w-5 text-yellow-600 mt-0.5 flex-shrink-0" />
                  <div className="space-y-2">
                    <div className="font-medium">Review before installing</div>
                    <div className="space-y-2 text-sm">
                      {reviewReasonsForAnalysis(installReview).map((notice, index) => (
                        <p key={index}>{notice.message}</p>
                      ))}
                    </div>
                  </div>
                </div>
              </div>

              {installReview.plugin_info && (
                <div className="rounded-md border p-4">
                  <div className="flex flex-wrap items-center gap-2">
                    <div className="font-medium">{installReview.plugin_info.name}</div>
                    <Badge variant="outline">v{installReview.plugin_info.version}</Badge>
                    {getTrustLevelBadge(analysisTrustLevel(installReview))}
                  </div>
                  <div className="text-sm text-muted-foreground mt-1">
                    by {installReview.plugin_info.author}
                  </div>
                  <p className="text-sm mt-2">{installReview.plugin_info.description}</p>
                </div>
              )}

              <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 text-sm">
                <div className="rounded-md border p-3">
                  <div className="text-muted-foreground">Trust</div>
                  <div className="font-medium mt-1">{analysisTrustLevel(installReview)}</div>
                </div>
                <div className="rounded-md border p-3">
                  <div className="text-muted-foreground">Signature</div>
                  <div
                    className={
                      installReview.security_info.signature_valid
                        ? "text-green-600 font-medium mt-1"
                        : "text-yellow-600 font-medium mt-1"
                    }
                  >
                    {installReview.security_info.signature_valid ? "Verified" : "Not verified"}
                  </div>
                </div>
                <div className="rounded-md border p-3">
                  <div className="text-muted-foreground">Package</div>
                  <div className="font-medium mt-1">{formatBytes(installReview.size_bytes)}</div>
                </div>
              </div>

              <div>
                <Label>Declared Capabilities</Label>
                <ScrollArea className="mt-2 max-h-28 rounded-md border p-3">
                  <div className="flex flex-wrap gap-1">
                    {installReview.declared_capabilities.length === 0 ? (
                      <span className="text-sm text-muted-foreground">No capabilities requested</span>
                    ) : (
                      installReview.declared_capabilities.map((capability, index) => (
                        <Badge key={index} variant="outline" className="text-xs">
                          {capability}
                        </Badge>
                      ))
                    )}
                  </div>
                </ScrollArea>
              </div>

              <div className="flex gap-2">
                <Button
                  variant="outline"
                  onClick={() => setInstallReview(null)}
                  disabled={isInstalling}
                  className="flex-1"
                >
                  Back
                </Button>
                <Button
                  onClick={() => handleInstallPlugin(true)}
                  disabled={isInstalling}
                  className="flex-1"
                >
                  {isInstalling ? (
                    <>
                      <Activity className="w-4 h-4 mr-2 animate-spin" />
                      Installing...
                    </>
                  ) : (
                    <>
                      <Upload className="w-4 h-4 mr-2" />
                      Install Reviewed Package
                    </>
                  )}
                </Button>
              </div>
              <Button variant="ghost" onClick={() => handleInstallDialogOpenChange(false)} className="w-full">
                Cancel
              </Button>
            </div>
          ) : (
            <div className="space-y-4">
              <div>
                <Label htmlFor="zip-file">Plugin ZIP Package</Label>
                <Input
                  id="zip-file"
                  type="file"
                  accept=".zip"
                  onChange={(e) => {
                    setInstallForm({ zipFile: e.target.files?.[0] || null });
                    setInstallReview(null);
                    setInstallResult(null);
                  }}
                />
                <p className="text-sm text-muted-foreground mt-1">
                  Trust and capabilities are applied from the package manifest during install.
                </p>
              </div>

              <div className="flex gap-2">
                <Button 
                  onClick={() => handleInstallPlugin()} 
                  disabled={!installForm.zipFile || isInstalling}
                  className="flex-1"
                >
                  {isInstalling ? (
                    <>
                      <Activity className="w-4 h-4 mr-2 animate-spin" />
                      Installing...
                    </>
                  ) : (
                    <>
                      <Upload className="w-4 h-4 mr-2" />
                      Install
                    </>
                  )}
                </Button>
              </div>
              <Button variant="ghost" onClick={() => handleInstallDialogOpenChange(false)} className="w-full">
                Cancel
              </Button>
            </div>
          )}
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
                          {formatCapability(cap)}
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
                          <span className="font-mono text-sm">{formatCapability(capability)}</span>
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
    </PageLayout>
  );
};

export default Plugins;
