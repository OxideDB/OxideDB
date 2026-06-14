import React, { useState, useEffect, useCallback } from 'react';
import { Alert, AlertDescription } from '../components/ui/alert';
import { Button } from '../components/ui/button';
import PageTabs, { TabsTrigger, TabsContent } from '@/components/PageTabs';
import { TooltipProvider, Tooltip, TooltipContent, TooltipTrigger } from '../components/ui/tooltip';
import { 
  AlertTriangle, RefreshCw, Download
} from 'lucide-react';
import PageLayout from '@/components/PageLayout';
import { apiService } from '../services/api';
import {
  SearchTab,
  RetentionTab,
  LogsTable,
  AuditTable,
  DashboardTab
} from '../components/LogsComponents';
import type {
  LogEntry, SecurityAuditEvent, DashboardMetrics, LogQueryParams, AuditQueryParams,
  RetentionStats, LoggingHealthResponse
} from '../types/api';

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
  
  // State for filtering
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

  const loadDashboardData = useCallback(async () => {
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
  }, []);

  const loadRecentLogs = useCallback(async () => {
    try {
      const recent = await apiService.getRecentLogs(10);
      setRecentLogs(recent);
    } catch (err) {
      console.error('Failed to load recent logs:', err);
    }
  }, []);

  const loadRetentionStats = useCallback(async () => {
    try {
      setLoading(true);
      const stats = await apiService.getRetentionStats();
      setRetentionStats(stats);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load retention stats');
    } finally {
      setLoading(false);
    }
  }, []);

  const loadLogs = useCallback(async () => {
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
  }, [logFilters, logsPagination.limit, logsPagination.offset, searchTerm]);

  const loadAuditEvents = useCallback(async () => {
    try {
      setLoading(true);
      const filters = {
        ...auditFilters,
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
  }, [auditFilters, auditPagination.limit, auditPagination.offset]);

  // Load initial data
  useEffect(() => {
    loadDashboardData();
    loadRecentLogs();
  }, [loadDashboardData, loadRecentLogs]);

  // Load data when tab changes
  useEffect(() => {
    setError(null);
    
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
    }
  }, [
    activeTab,
    auditEvents.length,
    loadAuditEvents,
    loadDashboardData,
    loadLogs,
    loadRecentLogs,
    loadRetentionStats,
    logs.length,
    retentionStats,
  ]);

  const searchByCorrelation = async () => {
    if (!selectedCorrelationId.trim()) return;
    
    try {
      setLoading(true);
      setError(null);
      
      const response = await apiService.getLogsByCorrelation(selectedCorrelationId);
      
      setLogFilters({
        level: '',
        limit: response.pagination.limit,
        sort: 'desc'
      });
      setSearchTerm('');
      setLogs(response.data);
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
    setLogs([]);
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
  };

  const handleLogsLimitChange = (newLimit: number) => {
    setLogsPagination(prev => ({ ...prev, limit: newLimit, offset: 0 }));
    setLogFilters(prev => ({ ...prev, limit: newLimit }));
  };

  // Pagination handlers for audit events
  const handleAuditPageChange = (newOffset: number) => {
    setAuditPagination(prev => ({ ...prev, offset: newOffset }));
  };

  const handleAuditLimitChange = (newLimit: number) => {
    setAuditPagination(prev => ({ ...prev, limit: newLimit, offset: 0 }));
    setAuditFilters(prev => ({ ...prev, limit: newLimit }));
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

  // Quick search handlers
  const handleQuickSearch = (type: 'errors' | 'security' | 'auth') => {
    setSelectedCorrelationId('');
    
    switch (type) {
      case 'errors':
        handleLogFiltersChange({ level: 'ERROR', limit: 50, sort: 'desc' });
        setActiveTab('logs');
        setTimeout(() => loadLogs(), 100);
        break;
      case 'security':
        handleAuditFiltersChange({ event_type: 'SecurityViolation', limit: 50, sort: 'desc' });
        setActiveTab('audit');
        setTimeout(() => loadAuditEvents(), 100);
        break;
      case 'auth':
        handleAuditFiltersChange({ event_type: 'Authentication', limit: 50, sort: 'desc' });
        setActiveTab('audit');
        setTimeout(() => loadAuditEvents(), 100);
        break;
    }
  };

  // Watch for pagination changes and reload data
  useEffect(() => {
    if (activeTab === 'logs' && !selectedCorrelationId) {
      loadLogs();
    }
  }, [activeTab, loadLogs, logsPagination.offset, logsPagination.limit, selectedCorrelationId]);

  useEffect(() => {
    if (activeTab === 'audit') {
      loadAuditEvents();
    }
  }, [activeTab, auditPagination.offset, auditPagination.limit, loadAuditEvents]);

  if (error && activeTab === 'dashboard') {
    return (
      <PageLayout title="Logs & Monitoring" description="System logs, audit events, and monitoring dashboard">
        <Alert>
          <AlertTriangle className="h-4 w-4" />
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      </PageLayout>
    );
  }

  const headerActions = (
    <TooltipProvider>
      <div className="flex gap-2">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button onClick={loadDashboardData} disabled={loading}>
              <RefreshCw className="h-4 w-4 mr-2" />
              Refresh
            </Button>
          </TooltipTrigger>
          <TooltipContent>Refresh dashboard data</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button onClick={flushLogs} variant="outline">
              <Download className="h-4 w-4 mr-2" />
              Flush Logs
            </Button>
          </TooltipTrigger>
          <TooltipContent>Export and flush log data</TooltipContent>
        </Tooltip>
      </div>
    </TooltipProvider>
  );

  return (
    <PageLayout 
      title="Logs & Monitoring" 
      description="System logs, audit events, and monitoring dashboard"
      headerActions={headerActions}
    >
      <TooltipProvider>

        {error && (
          <Alert>
            <AlertTriangle className="h-4 w-4" />
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        <PageTabs
          value={activeTab}
          onValueChange={setActiveTab}
          tabTriggers={
            <>
              <TabsTrigger value="dashboard">
                Dashboard
              </TabsTrigger>
              <TabsTrigger value="logs">
                Logs
              </TabsTrigger>
              <TabsTrigger value="audit">
                Audit Events
              </TabsTrigger>
              <TabsTrigger value="retention">
                Retention
              </TabsTrigger>
              <TabsTrigger value="search">
                Search
              </TabsTrigger>
            </>
          }
        >
          <TabsContent value="dashboard">
            <DashboardTab
              dashboardMetrics={dashboardMetrics}
              loggingHealth={loggingHealth}
              recentLogs={recentLogs}
              onCorrelationSearch={(correlationId) => {
                setSelectedCorrelationId(correlationId);
                searchByCorrelation();
              }}
            />
          </TabsContent>

          <TabsContent value="logs">
            <LogsTable
              logs={logs}
              searchTerm={searchTerm}
              filters={logFilters}
              selectedCorrelationId={selectedCorrelationId}
              loading={loading}
              pagination={logsPagination}
              onSearchTermChange={setSearchTerm}
              onFiltersChange={handleLogFiltersChange}
              onSearch={() => {
                clearCorrelationSearch();
                loadLogs();
              }}
              onClearSearch={clearCorrelationSearch}
              onCorrelationSearch={(correlationId) => {
                setSelectedCorrelationId(correlationId);
                searchByCorrelation();
              }}
              onPageChange={handleLogsPageChange}
              onLimitChange={handleLogsLimitChange}
            />
          </TabsContent>

          <TabsContent value="audit">
            <AuditTable
              auditEvents={auditEvents}
              filters={auditFilters}
              loading={loading}
              pagination={auditPagination}
              onFiltersChange={handleAuditFiltersChange}
              onSearch={() => {
                setSelectedCorrelationId('');
                loadAuditEvents();
              }}
              onCorrelationSearch={(correlationId) => {
                setSelectedCorrelationId(correlationId);
                searchByCorrelation();
              }}
              onPageChange={handleAuditPageChange}
              onLimitChange={handleAuditLimitChange}
            />
          </TabsContent>

          <TabsContent value="retention">
            <RetentionTab
              retentionStats={retentionStats}
              loading={loading}
            />
          </TabsContent>

          <TabsContent value="search">
            <SearchTab
              selectedCorrelationId={selectedCorrelationId}
              loading={loading}
              onCorrelationIdChange={setSelectedCorrelationId}
              onCorrelationSearch={searchByCorrelation}
              onQuickSearch={handleQuickSearch}
            />
          </TabsContent>
        </PageTabs>
      </TooltipProvider>
    </PageLayout>
  );
};

export default Logs; 
