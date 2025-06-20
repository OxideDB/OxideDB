import React, { useState, useEffect, useCallback } from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../components/ui/card';
import { Badge, type BadgeProps } from '../components/ui/badge';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../components/ui/select';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../components/ui/tabs';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../components/ui/table';
import { Alert, AlertDescription } from '../components/ui/alert';
import { Progress } from '../components/ui/progress';
import { Separator } from '../components/ui/separator';
import { ScrollArea } from '../components/ui/scroll-area';
import { 
  Activity, AlertTriangle, Info, Settings, Search, Calendar, 
  User, Database, Clock, TrendingUp, Filter, RefreshCw,
  Download, Trash, Eye, Shield, Bug, ChevronLeft, ChevronRight,
  ChevronsLeft, ChevronsRight, Copy
} from 'lucide-react';
import { apiService } from '../services/api';
import type {
  LogEntry, SecurityAuditEvent, DashboardMetrics, LogQueryParams, AuditQueryParams,
  LogLevel, AuditEventType, RetentionStats, LoggingHealthResponse
} from '../types/api';

const Logs: React.FC = () => {
  // State for dashboard metrics
  const [dashboardMetrics, setDashboardMetrics] = useState<DashboardMetrics | null>(null);
  const [retentionStats, setRetentionStats] = useState<RetentionStats | null>(null);
  const [loggingHealth, setLoggingHealth] = useState<LoggingHealthResponse | null>(null);
  
  // State for logs and audit events
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [auditEvents, setAuditEvents] = useState<SecurityAuditEvent[]>([]);
  const [recentLogs, setRecentLogs] = useState<LogEntry[]>([]);
  
  // State for pagination
  const [logsPagination, setLogsPagination] = useState({
    offset: 0,
    limit: 50,
    total: 0,
    hasMore: false
  });
  const [auditPagination, setAuditPagination] = useState({
    offset: 0,
    limit: 50,
    total: 0,
    hasMore: false
  });
  
  // State for filtering and pagination
  const [logFilters, setLogFilters] = useState<LogQueryParams>({
    level: '',
    limit: 50,
    sort: 'desc'
  });
  const [auditFilters, setAuditFilters] = useState<AuditQueryParams>({
    severity: '',
    limit: 50,
    sort: 'desc'
  });
  
  // UI state
  const [activeTab, setActiveTab] = useState('dashboard');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searchTerm, setSearchTerm] = useState('');
  const [selectedCorrelationId, setSelectedCorrelationId] = useState<string>('');

  // Map frontend audit event types to backend format
  const mapAuditEventType = (frontendType: string): string => {
    const mapping: Record<string, string> = {
      'Authentication': 'authentication',
      'Authorization': 'authorization',
      'DataAccess': 'data_access',
      'DataModification': 'data_modification',
      'ConfigurationChange': 'configuration_change',
      'SecurityViolation': 'security_violation',
      'PluginEvent': 'plugin_event',
      'SystemEvent': 'system_event'
    };
    return mapping[frontendType] || frontendType;
  };

  // Load initial data
  useEffect(() => {
    loadDashboardData();
    loadRecentLogs();
  }, []);

  // Load data when tab changes
  useEffect(() => {
    setError(null); // Clear any previous errors
    
    switch (activeTab) {
      case 'dashboard':
        loadDashboardData();
        loadRecentLogs();
        break;
      case 'logs':
        if (logs.length === 0) {
          loadLogs();
        }
        break;
      case 'audit':
        if (auditEvents.length === 0) {
          loadAuditEvents();
        }
        break;
      case 'retention':
        if (!retentionStats) {
          loadRetentionStats();
        }
        break;
      // Search tab doesn't need initial data loading
    }
  }, [activeTab]);

  const loadDashboardData = async () => {
    try {
      setLoading(true);
      const [metrics, retention, health] = await Promise.all([
        apiService.getDashboardMetrics(),
        apiService.getRetentionStats(),
        apiService.getLoggingHealth()
      ]);
      setDashboardMetrics(metrics);
      setRetentionStats(retention);
      setLoggingHealth(health);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load dashboard data');
    } finally {
      setLoading(false);
    }
  };

  const loadRecentLogs = async () => {
    try {
      const recent = await apiService.getRecentLogs(10);
      setRecentLogs(recent);
    } catch (err) {
      console.error('Failed to load recent logs:', err);
    }
  };

  const loadRetentionStats = async () => {
    try {
      setLoading(true);
      const stats = await apiService.getRetentionStats();
      setRetentionStats(stats);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load retention stats');
    } finally {
      setLoading(false);
    }
  };

  const loadLogs = async () => {
    try {
      setLoading(true);
      const filters = {
        ...logFilters,
        search: searchTerm || undefined,
        offset: logsPagination.offset,
        limit: logsPagination.limit
      };
      const response = await apiService.getLogs(filters);
      setLogs(response.data);
      setLogsPagination({
        offset: response.pagination.offset,
        limit: response.pagination.limit,
        total: response.pagination.total || 0,
        hasMore: response.pagination.has_more
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load logs');
    } finally {
      setLoading(false);
    }
  };

  const loadAuditEvents = async () => {
    try {
      setLoading(true);
      const filters = {
        ...auditFilters,
        // Map frontend event type to backend format
        event_type: auditFilters.event_type ? mapAuditEventType(auditFilters.event_type) : undefined,
        offset: auditPagination.offset,
        limit: auditPagination.limit
      };
      const response = await apiService.getAuditEvents(filters);
      setAuditEvents(response.data);
      setAuditPagination({
        offset: response.pagination.offset,
        limit: response.pagination.limit,
        total: response.pagination.total || 0,
        hasMore: response.pagination.has_more
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load audit events');
    } finally {
      setLoading(false);
    }
  };

  const searchByCorrelation = async () => {
    if (!selectedCorrelationId.trim()) return;
    
    try {
      setLoading(true);
      setError(null);
      
      const response = await apiService.getLogsByCorrelation(selectedCorrelationId);
      
      // Clear filters and search term
      setLogFilters({
        level: '',
        limit: response.pagination.limit,
        sort: 'desc'
      });
      setSearchTerm('');
      
      // Update logs and pagination without triggering useEffect
      setLogs(response.data);
      // Don't update pagination to avoid triggering the effect
      
      setActiveTab('logs');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to search by correlation ID');
    } finally {
      setLoading(false);
    }
  };

  const clearCorrelationSearch = () => {
    setSelectedCorrelationId('');
    setLogFilters({ level: '', limit: 50, sort: 'desc' });
    setSearchTerm('');
    setLogsPagination(prev => ({ ...prev, offset: 0 }));
    setLogs([]); // Clear existing logs
    // The useEffect will trigger loadLogs() automatically
  };

  const flushLogs = async () => {
    try {
      await apiService.flushLogs();
      loadDashboardData();
      loadRecentLogs();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to flush logs');
    }
  };

  // Pagination handlers for logs
  const handleLogsPageChange = (newOffset: number) => {
    setLogsPagination(prev => ({ ...prev, offset: newOffset }));
    // Don't call loadLogs() here, it will be called after state update
  };

  const handleLogsLimitChange = (newLimit: number) => {
    setLogsPagination(prev => ({ ...prev, limit: newLimit, offset: 0 }));
    setLogFilters(prev => ({ ...prev, limit: newLimit }));
    // Don't call loadLogs() here, it will be called after state update
  };

  // Pagination handlers for audit events
  const handleAuditPageChange = (newOffset: number) => {
    setAuditPagination(prev => ({ ...prev, offset: newOffset }));
    // Don't call loadAuditEvents() here, it will be called after state update
  };

  const handleAuditLimitChange = (newLimit: number) => {
    setAuditPagination(prev => ({ ...prev, limit: newLimit, offset: 0 }));
    setAuditFilters(prev => ({ ...prev, limit: newLimit }));
    // Don't call loadAuditEvents() here, it will be called after state update
  };

  // Reset pagination when filters change
  const handleLogFiltersChange = (newFilters: LogQueryParams) => {
    setLogFilters(newFilters);
    setLogsPagination(prev => ({ ...prev, offset: 0 }));
  };

  const handleAuditFiltersChange = (newFilters: AuditQueryParams) => {
    setAuditFilters(newFilters);
    setAuditPagination(prev => ({ ...prev, offset: 0 }));
  };

  // Watch for pagination changes and reload data
  useEffect(() => {
    if (activeTab === 'logs' && !selectedCorrelationId) {
      loadLogs();
    }
  }, [logsPagination.offset, logsPagination.limit]);

  useEffect(() => {
    if (activeTab === 'audit') {
      loadAuditEvents();
    }
  }, [auditPagination.offset, auditPagination.limit]);

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const formatDate = (dateString: string): string => {
    return new Date(dateString).toLocaleString();
  };

  const getLogLevelColor = (level: LogLevel): string => {
    switch (level) {
      case 'ERROR': return 'destructive';
      case 'WARN': return 'secondary';
      case 'INFO': return 'default';
      case 'DEBUG': return 'outline';
      case 'TRACE': return 'outline';
      default: return 'default';
    }
  };

  const getLogLevelIcon = (level: LogLevel) => {
    switch (level) {
      case 'ERROR': return <AlertTriangle className="h-4 w-4" />;
      case 'WARN': return <AlertTriangle className="h-4 w-4" />;
      case 'INFO': return <Info className="h-4 w-4" />;
      case 'DEBUG': return <Bug className="h-4 w-4" />;
      case 'TRACE': return <Eye className="h-4 w-4" />;
      default: return <Info className="h-4 w-4" />;
    }
  };

  const getAuditEventIcon = (eventType: AuditEventType) => {
    switch (eventType) {
      case 'Authentication': return <User className="h-4 w-4" />;
      case 'Authorization': return <Shield className="h-4 w-4" />;
      case 'DataAccess': return <Database className="h-4 w-4" />;
      case 'DataModification': return <Database className="h-4 w-4" />;
      case 'ConfigurationChange': return <Settings className="h-4 w-4" />;
      case 'SecurityViolation': return <AlertTriangle className="h-4 w-4" />;
      case 'PluginEvent': return <Activity className="h-4 w-4" />;
      case 'SystemEvent': return <Settings className="h-4 w-4" />;
      default: return <Info className="h-4 w-4" />;
    }
  };

  // Pagination component
  const PaginationControls: React.FC<{
    offset: number;
    limit: number;
    total: number;
    hasMore: boolean;
    onPageChange: (newOffset: number) => void;
    onLimitChange: (newLimit: number) => void;
  }> = ({ offset, limit, total, hasMore, onPageChange, onLimitChange }) => {
    const currentPage = Math.floor(offset / limit) + 1;
    const totalPages = total > 0 ? Math.ceil(total / limit) : 1;
    const hasNext = hasMore;
    const hasPrev = offset > 0;

    return (
      <div className="flex items-center justify-between">
        <div className="flex items-center space-x-2">
          <Label>Show</Label>
          <Select value={limit.toString()} onValueChange={(value) => onLimitChange(parseInt(value))}>
            <SelectTrigger className="w-[80px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="25">25</SelectItem>
              <SelectItem value="50">50</SelectItem>
              <SelectItem value="100">100</SelectItem>
              <SelectItem value="200">200</SelectItem>
            </SelectContent>
          </Select>
          <span className="text-sm text-muted-foreground">entries per page</span>
        </div>

        <div className="flex items-center space-x-2">
          <span className="text-sm text-muted-foreground">
            {total > 0 ? `${offset + 1}-${Math.min(offset + limit, total)} of ${total}` : `${offset + 1}-${offset + limit}`}
          </span>
          
          <div className="flex items-center space-x-1">
            <Button
              variant="outline"
              size="sm"
              onClick={() => onPageChange(0)}
              disabled={!hasPrev}
            >
              <ChevronsLeft className="h-4 w-4" />
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => onPageChange(Math.max(0, offset - limit))}
              disabled={!hasPrev}
            >
              <ChevronLeft className="h-4 w-4" />
            </Button>
            
            <span className="text-sm px-2">
              Page {currentPage} {total > 0 ? `of ${totalPages}` : ''}
            </span>
            
            <Button
              variant="outline"
              size="sm"
              onClick={() => onPageChange(offset + limit)}
              disabled={!hasNext}
            >
              <ChevronRight className="h-4 w-4" />
            </Button>
            {total > 0 && (
              <Button
                variant="outline"
                size="sm"
                onClick={() => onPageChange((totalPages - 1) * limit)}
                disabled={!hasNext}
              >
                <ChevronsRight className="h-4 w-4" />
              </Button>
            )}
          </div>
        </div>
      </div>
    );
  };

  if (error && activeTab === 'dashboard') {
    return (
      <div className="container mx-auto p-6">
        <Alert>
          <AlertTriangle className="h-4 w-4" />
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      </div>
    );
  }

  return (
    <div className="container mx-auto p-6 space-y-6">
      <div className="flex justify-between items-center">
        <div>
          <h1 className="text-3xl font-bold">Logs & Monitoring</h1>
          <p className="text-gray-600">System logs, audit events, and monitoring dashboard</p>
        </div>
        <div className="flex gap-2">
          <Button onClick={loadDashboardData} disabled={loading}>
            <RefreshCw className="h-4 w-4 mr-2" />
            Refresh
          </Button>
          <Button onClick={flushLogs} variant="outline">
            <Download className="h-4 w-4 mr-2" />
            Flush Logs
          </Button>
        </div>
      </div>

      {error && (
        <Alert>
          <AlertTriangle className="h-4 w-4" />
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <TabsList className="grid w-full grid-cols-5">
          <TabsTrigger value="dashboard">Dashboard</TabsTrigger>
          <TabsTrigger value="logs">Logs</TabsTrigger>
          <TabsTrigger value="audit">Audit Events</TabsTrigger>
          <TabsTrigger value="retention">Retention</TabsTrigger>
          <TabsTrigger value="search">Search</TabsTrigger>
        </TabsList>

        <TabsContent value="dashboard">
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
            {/* Overview Cards */}
            {dashboardMetrics && (
              <>
                <Card>
                  <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                    <CardTitle className="text-sm font-medium">Total Logs</CardTitle>
                    <Activity className="h-4 w-4 text-muted-foreground" />
                  </CardHeader>
                  <CardContent>
                    <div className="text-2xl font-bold">
                      {dashboardMetrics.log_metrics.total_entries.toLocaleString()}
                    </div>
                    <p className="text-xs text-muted-foreground">
                      Avg {dashboardMetrics.log_metrics.avg_entries_per_day.toFixed(1)}/day
                    </p>
                  </CardContent>
                </Card>

                <Card>
                  <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                    <CardTitle className="text-sm font-medium">Error Rate</CardTitle>
                    <AlertTriangle className="h-4 w-4 text-muted-foreground" />
                  </CardHeader>
                  <CardContent>
                    <div className="text-2xl font-bold">
                      {dashboardMetrics.log_metrics.error_rate_24h.toFixed(2)}%
                    </div>
                    <p className="text-xs text-muted-foreground">Last 24 hours</p>
                  </CardContent>
                </Card>

                <Card>
                  <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                    <CardTitle className="text-sm font-medium">Storage</CardTitle>
                    <Database className="h-4 w-4 text-muted-foreground" />
                  </CardHeader>
                  <CardContent>
                    <div className="text-2xl font-bold">
                      {formatBytes(dashboardMetrics.log_metrics.storage_size_bytes)}
                    </div>
                    <p className="text-xs text-muted-foreground">
                      {dashboardMetrics.health_indicators.storage_utilization.toFixed(1)}% utilized
                    </p>
                  </CardContent>
                </Card>

                <Card>
                  <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                    <CardTitle className="text-sm font-medium">Active Users</CardTitle>
                    <User className="h-4 w-4 text-muted-foreground" />
                  </CardHeader>
                  <CardContent>
                    <div className="text-2xl font-bold">
                      {dashboardMetrics.health_indicators.active_users}
                    </div>
                    <p className="text-xs text-muted-foreground">Currently active</p>
                  </CardContent>
                </Card>
              </>
            )}
          </div>

          {/* Performance Metrics */}
          {loggingHealth && (
            <Card className="mt-6">
              <CardHeader>
                <CardTitle>System Health</CardTitle>
                <CardDescription>Current system performance indicators</CardDescription>
              </CardHeader>
              <CardContent>
                <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                  <div>
                    <Label>Status</Label>
                    <Badge variant={loggingHealth.status === 'healthy' ? 'default' : 'destructive'}>
                      {loggingHealth.status}
                    </Badge>
                  </div>
                  <div>
                    <Label>Storage Size</Label>
                    <p className="text-lg font-semibold">{loggingHealth.storage_size_mb} MB</p>
                  </div>
                  <div>
                    <Label>Error Rate (24h)</Label>
                    <p className="text-lg font-semibold">{loggingHealth.error_rate_24h.toFixed(2)}%</p>
                    <Progress value={loggingHealth.error_rate_24h} className="mt-2" />
                  </div>
                </div>
              </CardContent>
            </Card>
          )}

          {/* Recent Activity */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mt-6">
            <Card>
              <CardHeader>
                <CardTitle>Recent Logs</CardTitle>
                <CardDescription>Latest 10 log entries</CardDescription>
              </CardHeader>
              <CardContent>
                <ScrollArea className="h-[300px]">
                  <div className="space-y-2">
                    {recentLogs.map((log) => (
                      <div key={log.id} className="flex items-start space-x-2 p-2 border rounded">
                        {getLogLevelIcon(log.level)}
                        <div className="flex-1 min-w-0">
                          <p className="text-sm font-medium truncate">{log.message}</p>
                          <p className="text-xs text-muted-foreground">
                            {log.module} • {formatDate(log.timestamp)}
                          </p>
                          <div className="flex items-center space-x-2 mt-1">
                            <code className="text-xs bg-muted px-1 py-0.5 rounded" title={log.correlation_id}>
                              {log.correlation_id.slice(0, 8)}...
                            </code>
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => {
                                setSelectedCorrelationId(log.correlation_id);
                                searchByCorrelation();
                              }}
                              className="h-5 w-5 p-0"
                              title="Search by correlation ID"
                            >
                              <Search className="h-3 w-3" />
                            </Button>
                          </div>
                        </div>
                        <Badge variant={getLogLevelColor(log.level as LogLevel) as BadgeProps['variant']}>{log.level}</Badge>
                      </div>
                    ))}
                  </div>
                </ScrollArea>
              </CardContent>
            </Card>

            {/* Top Users */}
            {dashboardMetrics && (
              <Card>
                <CardHeader>
                  <CardTitle>Top Active Users</CardTitle>
                  <CardDescription>Most active users by log volume</CardDescription>
                </CardHeader>
                <CardContent>
                  <div className="space-y-2">
                    {dashboardMetrics.log_metrics.top_users.slice(0, 5).map(([userId, count]) => (
                      <div key={userId} className="flex justify-between items-center">
                        <span className="text-sm font-medium">{userId}</span>
                        <Badge variant="outline">{count} actions</Badge>
                      </div>
                    ))}
                  </div>
                </CardContent>
              </Card>
            )}
          </div>
        </TabsContent>

        <TabsContent value="logs">
          <Card>
            <CardHeader>
              <CardTitle>Log Entries</CardTitle>
              <CardDescription>
                {selectedCorrelationId ? 
                  `Showing logs for correlation ID: ${selectedCorrelationId}` : 
                  'Browse and filter system logs'
                }
              </CardDescription>
            </CardHeader>
            <CardContent>
              {/* Filters */}
              <div className="flex flex-wrap gap-4 mb-4">
                <div className="flex-1 min-w-[200px]">
                  <Label htmlFor="search">Search</Label>
                  <Input
                    id="search"
                    placeholder="Search log messages..."
                    value={searchTerm}
                    onChange={(e) => setSearchTerm(e.target.value)}
                    disabled={!!selectedCorrelationId}
                  />
                </div>
                <div>
                  <Label>Level</Label>
                  <Select 
                    value={logFilters.level || "all"} 
                    onValueChange={(value) => 
                      handleLogFiltersChange({ ...logFilters, level: value === "all" ? "" : value })
                    }
                    disabled={!!selectedCorrelationId}
                  >
                    <SelectTrigger className="w-[120px]">
                      <SelectValue placeholder="All" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="all">All</SelectItem>
                      <SelectItem value="ERROR">Error</SelectItem>
                      <SelectItem value="WARN">Warning</SelectItem>
                      <SelectItem value="INFO">Info</SelectItem>
                      <SelectItem value="DEBUG">Debug</SelectItem>
                      <SelectItem value="TRACE">Trace</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                {selectedCorrelationId ? (
                  <Button onClick={clearCorrelationSearch} variant="outline">
                    <RefreshCw className="h-4 w-4 mr-2" />
                    Clear Search
                  </Button>
                ) : (
                  <Button onClick={() => {
                    clearCorrelationSearch();
                    loadLogs();
                  }} disabled={loading}>
                    <Search className="h-4 w-4 mr-2" />
                    Search
                  </Button>
                )}
              </div>

              <Separator className="mb-4" />

              {loading && <p className="text-center text-muted-foreground">Loading logs...</p>}

              {/* Logs Table */}
              <div className="rounded-md border">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Level</TableHead>
                      <TableHead>Timestamp</TableHead>
                      <TableHead>Module</TableHead>
                      <TableHead>Message</TableHead>
                      <TableHead>User</TableHead>
                      <TableHead>Collection</TableHead>
                      <TableHead>Correlation ID</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {logs.length === 0 && !loading ? (
                      <TableRow>
                        <TableCell colSpan={7} className="text-center text-muted-foreground">
                          No logs found. Click "Search" to load logs.
                        </TableCell>
                      </TableRow>
                    ) : (
                      logs.map((log) => (
                        <TableRow key={log.id}>
                          <TableCell>
                            <div className="flex items-center space-x-2">
                              {getLogLevelIcon(log.level)}
                              <Badge variant={getLogLevelColor(log.level as LogLevel) as BadgeProps['variant']}>{log.level}</Badge>
                            </div>
                          </TableCell>
                          <TableCell className="text-sm font-mono">
                            {formatDate(log.timestamp)}
                          </TableCell>
                          <TableCell className="font-medium">{log.module}</TableCell>
                          <TableCell className="max-w-md truncate">{log.message}</TableCell>
                          <TableCell>{log.context.user_id || '-'}</TableCell>
                          <TableCell>{log.context.collection || '-'}</TableCell>
                          <TableCell>
                            <div className="flex items-center space-x-1">
                              <code className="text-xs bg-muted px-2 py-1 rounded max-w-[100px] truncate" 
                                    title={log.correlation_id}>
                                {log.correlation_id.slice(0, 8)}...
                              </code>
                              <Button
                                variant="ghost"
                                size="sm"
                                onClick={() => navigator.clipboard.writeText(log.correlation_id)}
                                className="h-6 w-6 p-0"
                                title="Copy correlation ID"
                              >
                                <Copy className="h-3 w-3" />
                              </Button>
                              <Button
                                variant="ghost"
                                size="sm"
                                onClick={() => {
                                  setSelectedCorrelationId(log.correlation_id);
                                  searchByCorrelation();
                                }}
                                className="h-6 w-6 p-0"
                                title="Search by correlation ID"
                              >
                                <Search className="h-3 w-3" />
                              </Button>
                            </div>
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </div>

              {/* Pagination */}
              {logs.length > 0 && (
                <div className="mt-4">
                  <PaginationControls
                    offset={logsPagination.offset}
                    limit={logsPagination.limit}
                    total={logsPagination.total}
                    hasMore={logsPagination.hasMore}
                    onPageChange={handleLogsPageChange}
                    onLimitChange={handleLogsLimitChange}
                  />
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="audit">
          <Card>
            <CardHeader>
              <CardTitle>Security Audit Events</CardTitle>
              <CardDescription>Monitor security-related events and activities</CardDescription>
            </CardHeader>
            <CardContent>
              {/* Audit Filters */}
              <div className="flex flex-wrap gap-4 mb-4">
                <div>
                  <Label>Severity</Label>
                  <Select value={auditFilters.severity || "all"} onValueChange={(value) => 
                    handleAuditFiltersChange({ ...auditFilters, severity: value === "all" ? "" : value })
                  }>
                    <SelectTrigger className="w-[120px]">
                      <SelectValue placeholder="All" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="all">All</SelectItem>
                      <SelectItem value="ERROR">Error</SelectItem>
                      <SelectItem value="WARN">Warning</SelectItem>
                      <SelectItem value="INFO">Info</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div>
                  <Label>Event Type</Label>
                  <Select value={auditFilters.event_type || "all"} onValueChange={(value) => 
                    handleAuditFiltersChange({ ...auditFilters, event_type: value === "all" ? "" : value })
                  }>
                    <SelectTrigger className="w-[150px]">
                      <SelectValue placeholder="All" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="all">All</SelectItem>
                      <SelectItem value="Authentication">Authentication</SelectItem>
                      <SelectItem value="Authorization">Authorization</SelectItem>
                      <SelectItem value="DataAccess">Data Access</SelectItem>
                      <SelectItem value="DataModification">Data Modification</SelectItem>
                      <SelectItem value="SecurityViolation">Security Violation</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <Button onClick={() => {
                  setSelectedCorrelationId(''); // Clear correlation search
                  loadAuditEvents();
                }} disabled={loading}>
                  <Search className="h-4 w-4 mr-2" />
                  Search
                </Button>
              </div>

              <Separator className="mb-4" />

              {loading && <p className="text-center text-muted-foreground">Loading audit events...</p>}

              {/* Audit Events Table */}
              <div className="rounded-md border">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Type</TableHead>
                      <TableHead>Timestamp</TableHead>
                      <TableHead>Actor</TableHead>
                      <TableHead>Action</TableHead>
                      <TableHead>Target</TableHead>
                      <TableHead>Result</TableHead>
                      <TableHead>Risk</TableHead>
                      <TableHead>Correlation ID</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {auditEvents.length === 0 && !loading ? (
                      <TableRow>
                        <TableCell colSpan={8} className="text-center text-muted-foreground">
                          No audit events found. Click "Search" to load events.
                        </TableCell>
                      </TableRow>
                    ) : (
                      auditEvents.map((event) => (
                        <TableRow key={event.id}>
                          <TableCell>
                            <div className="flex items-center space-x-2">
                              {getAuditEventIcon(event.event_type)}
                              <span className="text-sm">{event.event_type}</span>
                            </div>
                          </TableCell>
                          <TableCell className="text-sm font-mono">
                            {formatDate(event.timestamp)}
                          </TableCell>
                          <TableCell className="font-medium">{event.actor}</TableCell>
                          <TableCell>{event.action}</TableCell>
                          <TableCell>{event.target || '-'}</TableCell>
                          <TableCell>
                            <Badge variant={event.result === 'success' ? 'default' : 'destructive'}>
                              {event.result}
                            </Badge>
                          </TableCell>
                          <TableCell>
                            {event.risk_score ? (
                              <Badge variant={event.risk_score > 70 ? 'destructive' : 
                                             event.risk_score > 40 ? 'secondary' : 'default'}>
                                {event.risk_score}
                              </Badge>
                            ) : '-'}
                          </TableCell>
                          <TableCell>
                            <div className="flex items-center space-x-1">
                              <code className="text-xs bg-muted px-2 py-1 rounded max-w-[100px] truncate" 
                                    title={event.correlation_id}>
                                {event.correlation_id.slice(0, 8)}...
                              </code>
                              <Button
                                variant="ghost"
                                size="sm"
                                onClick={() => navigator.clipboard.writeText(event.correlation_id)}
                                className="h-6 w-6 p-0"
                                title="Copy correlation ID"
                              >
                                <Copy className="h-3 w-3" />
                              </Button>
                              <Button
                                variant="ghost"
                                size="sm"
                                onClick={() => {
                                  setSelectedCorrelationId(event.correlation_id);
                                  searchByCorrelation();
                                }}
                                className="h-6 w-6 p-0"
                                title="Search by correlation ID"
                              >
                                <Search className="h-3 w-3" />
                              </Button>
                            </div>
                          </TableCell>
                        </TableRow>
                      ))
                    )}
                  </TableBody>
                </Table>
              </div>

              {/* Pagination */}
              {auditEvents.length > 0 && (
                <div className="mt-4">
                  <PaginationControls
                    offset={auditPagination.offset}
                    limit={auditPagination.limit}
                    total={auditPagination.total}
                    hasMore={auditPagination.hasMore}
                    onPageChange={handleAuditPageChange}
                    onLimitChange={handleAuditLimitChange}
                  />
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="retention">
          <Card>
            <CardHeader>
              <CardTitle>Log Retention</CardTitle>
              <CardDescription>Manage log retention policies and storage</CardDescription>
            </CardHeader>
            <CardContent>
              {loading && <p className="text-center text-muted-foreground">Loading retention data...</p>}
              
              {retentionStats ? (
                <div className="space-y-6">
                  <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                    <div>
                      <Label>Total Log Entries</Label>
                      <p className="text-2xl font-bold">{retentionStats.total_log_entries.toLocaleString()}</p>
                    </div>
                    <div>
                      <Label>Storage Size</Label>
                      <p className="text-2xl font-bold">{formatBytes(retentionStats.storage_size_bytes)}</p>
                    </div>
                    <div>
                      <Label>Cleanup Candidates</Label>
                      <p className="text-2xl font-bold">{retentionStats.estimated_cleanup_candidates.toLocaleString()}</p>
                    </div>
                  </div>

                  <Separator />

                  <div>
                    <h3 className="text-lg font-semibold mb-4">Retention Policy</h3>
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                      <div>
                        <Label>Standard Logs</Label>
                        <p className="text-lg">{retentionStats.policy.standard_retention_days} days</p>
                      </div>
                      <div>
                        <Label>Audit Logs</Label>
                        <p className="text-lg">{retentionStats.policy.audit_retention_days} days</p>
                      </div>
                      <div>
                        <Label>Error Logs</Label>
                        <p className="text-lg">{retentionStats.policy.error_retention_days} days</p>
                      </div>
                      <div>
                        <Label>Compression</Label>
                        <Badge variant={retentionStats.policy.enable_compression ? 'default' : 'secondary'}>
                          {retentionStats.policy.enable_compression ? 'Enabled' : 'Disabled'}
                        </Badge>
                      </div>
                    </div>
                  </div>

                  {retentionStats.last_cleanup_time && (
                    <div>
                      <Label>Last Cleanup</Label>
                      <p className="text-lg">{formatDate(retentionStats.last_cleanup_time)}</p>
                    </div>
                  )}
                </div>
              ) : !loading ? (
                <p className="text-center text-muted-foreground">No retention information available</p>
              ) : null}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="search">
          <Card>
            <CardHeader>
              <CardTitle>Advanced Search</CardTitle>
              <CardDescription>Search logs by correlation ID and other advanced criteria</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div>
                <Label htmlFor="correlationId">Correlation ID</Label>
                <div className="flex space-x-2">
                  <Input
                    id="correlationId"
                    placeholder="Enter correlation ID..."
                    value={selectedCorrelationId}
                    onChange={(e) => setSelectedCorrelationId(e.target.value)}
                    className="flex-1"
                  />
                  <Button onClick={searchByCorrelation} disabled={loading || !selectedCorrelationId.trim()}>
                    <Search className="h-4 w-4 mr-2" />
                    Search
                  </Button>
                </div>
                <p className="text-sm text-muted-foreground mt-1">
                  Search for all logs related to a specific operation or request
                </p>
              </div>

              <Separator />

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <Card>
                  <CardHeader>
                    <CardTitle className="text-lg">Quick Searches</CardTitle>
                  </CardHeader>
                  <CardContent className="space-y-2">
                    <Button
                      variant="outline"
                      className="w-full justify-start"
                      onClick={() => {
                        setSelectedCorrelationId('');
                        handleLogFiltersChange({ level: 'ERROR', limit: 50, sort: 'desc' });
                        setActiveTab('logs');
                        setTimeout(() => loadLogs(), 100);
                      }}
                    >
                      <AlertTriangle className="h-4 w-4 mr-2" />
                      Recent Errors
                    </Button>
                    <Button
                      variant="outline"
                      className="w-full justify-start"
                      onClick={() => {
                        setSelectedCorrelationId('');
                        handleAuditFiltersChange({ event_type: 'SecurityViolation', limit: 50, sort: 'desc' });
                        setActiveTab('audit');
                        setTimeout(() => loadAuditEvents(), 100);
                      }}
                    >
                      <Shield className="h-4 w-4 mr-2" />
                      Security Violations
                    </Button>
                    <Button
                      variant="outline"
                      className="w-full justify-start"
                      onClick={() => {
                        setSelectedCorrelationId('');
                        handleAuditFiltersChange({ event_type: 'Authentication', limit: 50, sort: 'desc' });
                        setActiveTab('audit');
                        setTimeout(() => loadAuditEvents(), 100);
                      }}
                    >
                      <User className="h-4 w-4 mr-2" />
                      Authentication Events
                    </Button>
                  </CardContent>
                </Card>

                <Card>
                  <CardHeader>
                    <CardTitle className="text-lg">Search Tips</CardTitle>
                  </CardHeader>
                  <CardContent className="space-y-2 text-sm text-muted-foreground">
                    <p>• Use correlation IDs to trace requests across services</p>
                    <p>• Filter by log level to focus on specific severity</p>
                    <p>• Search audit events by event type for security monitoring</p>
                    <p>• Use time ranges to narrow down investigation periods</p>
                  </CardContent>
                </Card>
              </div>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
};

export default Logs; 