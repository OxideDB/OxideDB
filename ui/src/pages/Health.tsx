import React, { useState, useEffect } from 'react';
import { CheckCircle, XCircle, RefreshCw, Database, Server, Activity, AlertTriangle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Alert, AlertDescription } from '@/components/ui/alert';
import PageLayout from '@/components/PageLayout';
import { LoadingState } from '@/components/admin/AdminState';
import { MetricCard } from '@/components/admin/MetricCard';
import { StatusIndicator } from '@/components/admin/StatusIndicator';
import { getStatusTone } from '@/components/admin/statusUtils';
import { apiService } from '../services/api';
import type { ApiHealthStatus } from '../types/api';

const Health: React.FC = () => {
  const [health, setHealth] = useState<ApiHealthStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchHealth();
  }, []);

  const fetchHealth = async () => {
    try {
      setLoading(true);
      setError(null);
      const healthData = await apiService.getHealth();
      setHealth(healthData);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch health status');
    } finally {
      setLoading(false);
    }
  };

  const isHealthy = health?.status === 'healthy' && health?.database === 'healthy';

  const healthMetrics = [
    {
      title: "API Server",
      status: health?.status || 'unknown',
      description: "REST API endpoint status",
      icon: Server,
      isHealthy: health?.status === 'healthy',
      tone: health?.status === 'healthy' ? 'success' as const : 'warning' as const,
    },
    {
      title: "Database",
      status: health?.database || 'unknown', 
      description: "Database connectivity",
      icon: Database,
      isHealthy: health?.database === 'healthy',
      tone: health?.database === 'healthy' ? 'success' as const : 'warning' as const,
    },
    {
      title: "Overall Status",
      status: isHealthy ? 'healthy' : 'unhealthy',
      description: "System-wide health check",
      icon: Activity,
      isHealthy: isHealthy,
      tone: isHealthy ? 'success' as const : 'warning' as const,
    }
  ];

  const headerActions = (
    <Button
      onClick={fetchHealth}
      disabled={loading}
      variant="secondary"
      className="flex items-center"
    >
      <RefreshCw className={`h-4 w-4 mr-2 ${loading ? 'animate-spin' : ''}`} />
      Refresh
    </Button>
  );

  return (
    <PageLayout 
      title="System Health" 
      description="Monitor the status of OxideDB components"
      headerActions={headerActions}
    >
      {error && (
        <Alert variant="destructive">
          <AlertTriangle className="h-4 w-4" />
          <AlertDescription>
            {error}
          <Button
            onClick={() => setError(null)}
            variant="ghost"
            size="sm"
            className="text-destructive hover:text-destructive mt-2 h-auto p-0"
          >
            Dismiss
          </Button>
          </AlertDescription>
        </Alert>
      )}

      {loading && !health ? (
        <LoadingState label="Loading health status" />
      ) : health ? (
        <div className="space-y-6">
          {/* Health Metrics Grid */}
          <div className="grid gap-3 md:gap-4 grid-cols-1 sm:grid-cols-2 lg:grid-cols-3">
            {healthMetrics.map((metric) => (
              <MetricCard
                key={metric.title}
                title={metric.title}
                value={
                  <StatusIndicator
                    label={metric.status}
                    tone={getStatusTone(metric.status)}
                  />
                }
                description={
                  health.version && metric.title === 'API Server'
                    ? `${metric.description} · Version ${health.version}`
                    : metric.description
                }
                icon={metric.icon}
                tone={metric.tone}
              />
            ))}
          </div>

          <div className="grid gap-4 md:gap-6 grid-cols-1 lg:grid-cols-2">
            {/* Overall System Status */}
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  {isHealthy ? (
                    <CheckCircle className="h-5 w-5 text-success" />
                  ) : (
                    <XCircle className="h-5 w-5 text-destructive" />
                  )}
                  System Status
                </CardTitle>
                <CardDescription>
                  Current operational status of all components
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="flex items-center justify-between">
                  <span className="text-sm text-muted-foreground">Database Connection</span>
                  <StatusIndicator
                    label={health.database === 'healthy' ? 'Connected' : 'Disconnected'}
                    tone={health.database === 'healthy' ? 'success' : 'danger'}
                  />
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-sm text-muted-foreground">API Endpoints</span>
                  <StatusIndicator
                    label={health.status === 'healthy' ? 'Online' : 'Offline'}
                    tone={health.status === 'healthy' ? 'success' : 'danger'}
                  />
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-sm text-muted-foreground">Overall Status</span>
                  <StatusIndicator
                    label={isHealthy ? 'Healthy' : 'Unhealthy'}
                    tone={isHealthy ? 'success' : 'danger'}
                  />
                </div>
              </CardContent>
            </Card>

            {/* Health Details */}
            <Card>
              <CardHeader>
                <CardTitle>System Details</CardTitle>
                <CardDescription>
                  Raw health check response data
                </CardDescription>
              </CardHeader>
              <CardContent>
                <div className="rounded-lg border bg-muted/40 p-4">
                  <pre className="overflow-x-auto whitespace-pre-wrap text-sm text-foreground">
                    {JSON.stringify(health, null, 2)}
                  </pre>
                </div>
              </CardContent>
            </Card>
          </div>
        </div>
      ) : null}
    </PageLayout>
  );
};

export default Health;
