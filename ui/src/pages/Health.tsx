import React, { useState, useEffect } from 'react';
import { CheckCircle, XCircle, RefreshCw, Database, Server, Activity, AlertTriangle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import PageLayout from '@/components/PageLayout';
import { apiService } from '../services/api';
import type { HealthStatus } from '../types/api';

type ExtendedHealthStatus = HealthStatus & { version?: string };

const Health: React.FC = () => {
  const [health, setHealth] = useState<ExtendedHealthStatus | null>(null);
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
      isHealthy: health?.status === 'healthy'
    },
    {
      title: "Database",
      status: health?.database || 'unknown', 
      description: "Database connectivity",
      icon: Database,
      isHealthy: health?.database === 'healthy'
    },
    {
      title: "Overall Status",
      status: isHealthy ? 'healthy' : 'unhealthy',
      description: "System-wide health check",
      icon: Activity,
      isHealthy: isHealthy
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
        <div className="bg-destructive/15 border border-destructive/20 rounded-lg p-4">
          <div className="flex items-center gap-2">
            <AlertTriangle className="h-4 w-4 text-destructive" />
            <div className="text-destructive text-sm font-medium">{error}</div>
          </div>
          <Button
            onClick={() => setError(null)}
            variant="ghost"
            size="sm"
            className="text-destructive hover:text-destructive mt-2 h-auto p-0"
          >
            Dismiss
          </Button>
        </div>
      )}

      {loading && !health ? (
        <div className="flex items-center justify-center h-64">
          <div className="text-muted-foreground">Loading health status...</div>
        </div>
      ) : health ? (
        <div className="space-y-6">
          {/* Health Metrics Grid */}
          <div className="grid gap-3 md:gap-4 grid-cols-1 sm:grid-cols-2 lg:grid-cols-3">
            {healthMetrics.map((metric) => (
              <Card key={metric.title}>
                <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                  <CardTitle className="text-sm font-medium">
                    {metric.title}
                  </CardTitle>
                  <metric.icon className="h-4 w-4 text-muted-foreground" />
                </CardHeader>
                <CardContent>
                  <div className="flex items-center gap-2 mb-1">
                    <div className={`h-2 w-2 rounded-full ${
                      metric.isHealthy ? 'bg-green-500' : 'bg-red-500'
                    }`}></div>
                    <Badge variant={metric.isHealthy ? 'default' : 'destructive'} className="text-xs">
                      {metric.status}
                    </Badge>
                  </div>
                  <p className="text-xs text-muted-foreground">
                    {metric.description}
                  </p>
                  {health.version && metric.title === 'API Server' && (
                    <p className="text-xs text-muted-foreground mt-1">
                      Version: {health.version}
                    </p>
                  )}
                </CardContent>
              </Card>
            ))}
          </div>

          <div className="grid gap-4 md:gap-6 grid-cols-1 lg:grid-cols-2">
            {/* Overall System Status */}
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  {isHealthy ? (
                    <CheckCircle className="h-5 w-5 text-green-500" />
                  ) : (
                    <XCircle className="h-5 w-5 text-red-500" />
                  )}
                  System Status
                </CardTitle>
                <CardDescription>
                  Current operational status of all components
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="flex items-center justify-between">
                  <span className="text-sm">Database Connection</span>
                  <div className="flex items-center gap-2">
                    <div className={`h-2 w-2 rounded-full ${
                      health.database === 'healthy' ? 'bg-green-500' : 'bg-red-500'
                    }`}></div>
                    <span className={`text-sm ${
                      health.database === 'healthy' ? 'text-green-600' : 'text-red-600'
                    }`}>
                      {health.database === 'healthy' ? 'Connected' : 'Disconnected'}
                    </span>
                  </div>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-sm">API Endpoints</span>
                  <div className="flex items-center gap-2">
                    <div className={`h-2 w-2 rounded-full ${
                      health.status === 'healthy' ? 'bg-green-500' : 'bg-red-500'
                    }`}></div>
                    <span className={`text-sm ${
                      health.status === 'healthy' ? 'text-green-600' : 'text-red-600'
                    }`}>
                      {health.status === 'healthy' ? 'Online' : 'Offline'}
                    </span>
                  </div>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-sm">Overall Status</span>
                  <div className="flex items-center gap-2">
                    <div className={`h-2 w-2 rounded-full ${
                      isHealthy ? 'bg-green-500' : 'bg-red-500'
                    }`}></div>
                    <span className={`text-sm font-medium ${
                      isHealthy ? 'text-green-600' : 'text-red-600'
                    }`}>
                      {isHealthy ? 'Healthy' : 'Unhealthy'}
                    </span>
                  </div>
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
                <div className="bg-muted rounded-lg p-4">
                  <pre className="text-sm text-foreground whitespace-pre-wrap overflow-x-auto">
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