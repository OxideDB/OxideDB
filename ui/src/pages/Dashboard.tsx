import React, { useEffect, useState } from "react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Database,
  Users,
  Activity,
  TrendingUp,
  Server,
  BarChart2,
} from "lucide-react";
import PageLayout from "@/components/PageLayout";
import { AdminState, ErrorState, LoadingState } from "@/components/admin/AdminState";
import { MetricCard } from "@/components/admin/MetricCard";
import { StatusDot, StatusIndicator } from "@/components/admin/StatusIndicator";
import { getStatusTone } from "@/components/admin/statusUtils";
import { apiService } from "@/services/api";
import type { DashboardStats } from "@/types/api";

const Dashboard: React.FC = () => {
  const [dashboardData, setDashboardData] = useState<DashboardStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchDashboardData = async () => {
      try {
        setLoading(true);
        setError(null);
        const data = await apiService.getDashboardStats();
        setDashboardData(data);
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load dashboard data');
        console.error('Failed to fetch dashboard data:', err);
      } finally {
        setLoading(false);
      }
    };

    fetchDashboardData();
  }, []);

  // Show loading state
  if (loading) {
    return (
      <PageLayout title="Dashboard" description="Operational overview for OxideDB">
        <LoadingState label="Loading dashboard data" />
      </PageLayout>
    );
  }

  // Show error state
  if (error) {
    return (
      <PageLayout title="Dashboard" description="Operational overview for OxideDB">
        <ErrorState
          title="Error loading dashboard"
          description={error}
          onRetry={() => window.location.reload()}
        />
      </PageLayout>
    );
  }

  // Show dashboard with real data
  if (!dashboardData) {
    return (
      <PageLayout title="Dashboard" description="Operational overview for OxideDB">
        <AdminState
          title="No dashboard data"
          description="The server returned an empty dashboard response."
          icon={Server}
        />
      </PageLayout>
    );
  }

  const stats = [
    {
      title: "Total Collections",
      value: dashboardData.system_stats.total_collections.toString(),
      description: "Active database collections",
      icon: Database,
      trend: `+${dashboardData.system_stats.trends.collections_this_month} this month`,
      tone: "info" as const,
    },
    {
      title: "Total Records",
      value: dashboardData.system_stats.total_records.toLocaleString(),
      description: "Records across all collections",
      icon: Activity,
      trend: `+${dashboardData.system_stats.trends.records_growth_percent.toFixed(1)}% from last month`,
      tone: "success" as const,
    },
    {
      title: "Active Users",
      value: dashboardData.system_stats.active_users.toString(),
      description: "Users with database access",
      icon: Users,
      trend: `+${dashboardData.system_stats.trends.new_users_count} new users`,
      tone: "warning" as const,
    },
    {
      title: "API Requests",
      value: dashboardData.system_stats.api_requests_24h.toLocaleString(),
      description: "Requests in the last 24h",
      icon: TrendingUp,
      trend: `+${dashboardData.system_stats.trends.api_growth_percent.toFixed(1)}% from yesterday`,
      tone: "info" as const,
    },
  ];

  const formatActivityTime = (timestamp: string): string => {
    const date = new Date(timestamp);
    const now = new Date();
    const diffInMinutes = Math.floor((now.getTime() - date.getTime()) / (1000 * 60));
    
    if (diffInMinutes < 1) return 'just now';
    if (diffInMinutes < 60) return `${diffInMinutes} minute${diffInMinutes === 1 ? '' : 's'} ago`;
    
    const diffInHours = Math.floor(diffInMinutes / 60);
    if (diffInHours < 24) return `${diffInHours} hour${diffInHours === 1 ? '' : 's'} ago`;
    
    const diffInDays = Math.floor(diffInHours / 24);
    return `${diffInDays} day${diffInDays === 1 ? '' : 's'} ago`;
  };

  const getHealthStatusText = (status: string): string => {
    switch (status) {
      case 'Healthy': return 'Healthy';
      case 'Warning': return 'Warning';
      case 'Degraded': return 'Degraded';
      case 'Unhealthy': return 'Unhealthy';
      default: return 'Unknown';
    }
  };

  const formatStorageSize = (bytes: string | number | bigint): string => {
    try {
      const value = Number(bytes);
      if (!Number.isFinite(value) || value <= 0) return "0 B";

      const units = ["B", "KB", "MB", "GB", "TB"];
      const exponent = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
      const normalized = value / Math.pow(1024, exponent);

      return `${normalized.toFixed(normalized >= 10 || exponent === 0 ? 0 : 1)} ${units[exponent]}`;
    } catch (error) {
      console.warn('Error formatting storage size:', error);
      return '0 B';
    }
  };

  const healthRows = [
    ["Database Connection", dashboardData.system_health.database_status],
    ["API Endpoints", dashboardData.system_health.api_status],
    ["Authentication Service", dashboardData.system_health.auth_status],
    ["Plugin Runtime", dashboardData.system_health.plugin_status],
    ["Virtual File System", dashboardData.system_health.vfs_status],
  ];

  const formatDuration = (seconds: number): string => {
    if (seconds < 60) return `${seconds}s`;
    const mins = Math.floor(seconds / 60);
    if (mins < 60) return `${mins}m ${seconds % 60}s`;
    const hrs = Math.floor(mins / 60);
    const remainingMins = mins % 60;
    if (hrs < 24) return `${hrs}h ${remainingMins}m`;
    const days = Math.floor(hrs / 24);
    return `${days}d ${hrs % 24}h`;
  };

  return (
    <PageLayout title="Dashboard" description="Operational overview for OxideDB">
      {/* Stats Grid */}
      <div className="grid gap-3 md:gap-4 grid-cols-1 sm:grid-cols-2 lg:grid-cols-4">
        {stats.map((stat) => (
          <MetricCard
            key={stat.title}
            title={stat.title}
            value={stat.value}
            description={stat.description}
            trend={stat.trend}
            icon={stat.icon}
            tone={stat.tone}
          />
        ))}
      </div>

      <div className="grid gap-4 md:gap-6 grid-cols-1 lg:grid-cols-2">
        {/* System Status */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Server className="h-5 w-5" />
              System Status
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            {healthRows.map(([label, status]) => (
              <div key={label} className="flex items-center justify-between gap-4">
                <span className="text-sm text-muted-foreground">{label}</span>
                <StatusIndicator
                  label={getHealthStatusText(status)}
                  tone={getStatusTone(status)}
                />
              </div>
            ))}
            <div className="flex items-center justify-between gap-4 border-t pt-4">
              <span className="text-sm text-muted-foreground">Storage Usage</span>
              <span className="text-sm font-medium tabular-nums">
                {formatStorageSize(dashboardData.system_health.storage_usage.used_bytes)} / {formatStorageSize(dashboardData.system_health.storage_usage.total_bytes)}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm text-muted-foreground">Uptime</span>
              <span className="text-sm font-medium tabular-nums">{formatDuration(Number(dashboardData.system_health.uptime_seconds))}</span>
            </div>
          </CardContent>
        </Card>

        {/* Collection Stats */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Database className="h-4 w-4" /> Collections
            </CardTitle>
            <CardDescription>Record distribution</CardDescription>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Records</TableHead>
                  <TableHead className="hidden md:table-cell">Size</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {dashboardData.collection_stats.slice(0, 6).map((col) => (
                  <TableRow key={col.name}>
                    <TableCell className="font-medium break-all">{col.name}</TableCell>
                    <TableCell className="tabular-nums">{col.record_count.toLocaleString()}</TableCell>
                    <TableCell className="hidden tabular-nums md:table-cell">
                      {formatStorageSize(col.size_bytes)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>

        {/* Top Users */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Users className="h-4 w-4" /> Top Active Users
            </CardTitle>
            <CardDescription>By action count</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              {dashboardData.user_stats.top_active_users.map((u, idx) => (
                <div key={idx} className="flex items-center justify-between gap-4 rounded-md border bg-muted/20 px-3 py-2 text-sm">
                  <span className="truncate max-w-[140px]" title={u.username}>{u.username}</span>
                  <span className="text-muted-foreground tabular-nums">{u.action_count.toLocaleString()}</span>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        {/* API Stats */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <BarChart2 className="h-4 w-4" /> API Stats
            </CardTitle>
            <CardDescription>Last 24 hours</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid grid-cols-2 gap-x-4 gap-y-3 text-sm">
              <span className="text-muted-foreground">Requests (24h)</span>
              <span className="text-right font-medium tabular-nums">{dashboardData.api_stats.requests_24h.toLocaleString()}</span>
              <span className="text-muted-foreground">Requests (7d)</span>
              <span className="text-right font-medium tabular-nums">{dashboardData.api_stats.requests_7d.toLocaleString()}</span>
              <span className="text-muted-foreground">Avg Response (ms)</span>
              <span className="text-right font-medium tabular-nums">{dashboardData.api_stats.avg_response_time_ms.toFixed(1)}</span>
              <span className="text-muted-foreground">Error Rate</span>
              <span className="text-right font-medium tabular-nums">{dashboardData.api_stats.error_rate_percent.toFixed(2)}%</span>
            </div>
            <hr className="my-2 border-border" />
            <div>
              <p className="text-xs text-muted-foreground mb-1">Top Endpoints</p>
              <ul className="space-y-1 text-xs">
                {dashboardData.api_stats.top_endpoints.slice(0, 5).map((ep, idx) => (
                  <li key={idx} className="flex items-center justify-between">
                    <span className="truncate max-w-[160px]" title={`${ep.method} ${ep.path}`}>{ep.method} {ep.path}</span>
                    <span className="font-medium tabular-nums">{ep.request_count}</span>
                  </li>
                ))}
              </ul>
            </div>
          </CardContent>
        </Card>

        {/* Recent Activity */}
        <Card className="lg:col-span-2">
          <CardHeader>
            <CardTitle>Recent Activity</CardTitle>
            <CardDescription>Latest database operations</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              {dashboardData.recent_activity.map((activity, index) => (
                <div key={index} className="flex items-start space-x-3 rounded-md border bg-muted/20 p-3">
                  <StatusDot tone="info" className="mt-2" />
                  <div className="flex-1 space-y-1 min-w-0">
                    <p className="text-sm break-words">{activity.description}</p>
                    <div className="flex flex-col sm:flex-row sm:items-center gap-1 sm:gap-2 text-xs text-muted-foreground">
                      <span>{activity.user}</span>
                      <span className="hidden sm:inline">•</span>
                      <span>{formatActivityTime(activity.timestamp)}</span>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>
    </PageLayout>
  );
};

export default Dashboard;
