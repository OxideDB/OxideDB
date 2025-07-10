import React, { useEffect, useState } from "react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Database,
  Users,
  Activity,
  TrendingUp,
  Server,
  BarChart2,
} from "lucide-react";
import PageLayout from "@/components/PageLayout";
import { dashboardApi, type DashboardStats } from "@/types/api";

const Dashboard: React.FC = () => {
  const [dashboardData, setDashboardData] = useState<DashboardStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchDashboardData = async () => {
      try {
        setLoading(true);
        const data = await dashboardApi.getDashboardStats();
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
      <PageLayout title="Dashboard">
        <div className="flex items-center justify-center min-h-[400px]">
          <div className="text-center">
            <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-gray-900 mx-auto mb-4"></div>
            <p className="text-gray-600">Loading dashboard data...</p>
          </div>
        </div>
      </PageLayout>
    );
  }

  // Show error state
  if (error) {
    return (
      <PageLayout title="Dashboard">
        <div className="flex items-center justify-center min-h-[400px]">
          <div className="text-center">
            <div className="text-red-500 mb-4">
              <Server className="h-12 w-12 mx-auto mb-2" />
            </div>
            <h3 className="text-lg font-semibold text-gray-900 mb-2">Error Loading Dashboard</h3>
            <p className="text-gray-600">{error}</p>
          </div>
        </div>
      </PageLayout>
    );
  }

  // Show dashboard with real data
  if (!dashboardData) return null;

  const stats = [
    {
      title: "Total Collections",
      value: dashboardData.system_stats.total_collections.toString(),
      description: "Active database collections",
      icon: Database,
      trend: `+${dashboardData.system_stats.trends.collections_this_month} this month`,
    },
    {
      title: "Total Records",
      value: dashboardData.system_stats.total_records.toLocaleString(),
      description: "Records across all collections",
      icon: Activity,
      trend: `+${dashboardData.system_stats.trends.records_growth_percent.toFixed(1)}% from last month`,
    },
    {
      title: "Active Users",
      value: dashboardData.system_stats.active_users.toString(),
      description: "Users with database access",
      icon: Users,
      trend: `+${dashboardData.system_stats.trends.new_users_count} new users`,
    },
    {
      title: "API Requests",
      value: dashboardData.system_stats.api_requests_24h.toLocaleString(),
      description: "Requests in the last 24h",
      icon: TrendingUp,
      trend: `+${dashboardData.system_stats.trends.api_growth_percent.toFixed(1)}% from yesterday`,
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

  const getHealthStatusColor = (status: string): string => {
    switch (status) {
      case 'Healthy': return 'bg-green-500';
      case 'Warning': return 'bg-yellow-500';
      case 'Degraded': return 'bg-orange-500';
      case 'Unhealthy': return 'bg-red-500';
      default: return 'bg-gray-500';
    }
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

  const getHealthStatusTextColor = (status: string): string => {
    switch (status) {
      case 'Healthy': return 'text-green-600';
      case 'Warning': return 'text-yellow-600';
      case 'Degraded': return 'text-orange-600';
      case 'Unhealthy': return 'text-red-600';
      default: return 'text-gray-600';
    }
  };

  const formatStorageSize = (bytes: string | number | bigint): string => {
    try {
      // Convert to BigInt regardless of input type
      const bigIntBytes = typeof bytes === 'bigint' ? bytes : BigInt(bytes.toString());
      const gbInBytes = 1073741824n; // 1024^3 as BigInt literal
      const gb = bigIntBytes / gbInBytes;
      return gb.toString();
    } catch (error) {
      console.warn('Error formatting storage size:', error);
      return '0';
    }
  };

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
    <PageLayout title="Dashboard">
      {/* Stats Grid */}
      <div className="grid gap-3 md:gap-4 grid-cols-1 sm:grid-cols-2 lg:grid-cols-4">
        {stats.map((stat) => (
          <Card key={stat.title}>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">
                {stat.title}
              </CardTitle>
              <stat.icon className="h-4 w-4 text-muted-foreground" />
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{stat.value}</div>
              <p className="text-xs text-muted-foreground">
                {stat.description}
              </p>
              <p className="text-xs text-green-600 mt-1">{stat.trend}</p>
            </CardContent>
          </Card>
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
            <div className="flex items-center justify-between">
              <span className="text-sm">Database Connection</span>
              <div className="flex items-center gap-2">
                <div className={`h-2 w-2 rounded-full ${getHealthStatusColor(dashboardData.system_health.database_status)}`}></div>
                <span className={`text-sm ${getHealthStatusTextColor(dashboardData.system_health.database_status)}`}>
                  {getHealthStatusText(dashboardData.system_health.database_status)}
                </span>
              </div>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">API Endpoints</span>
              <div className="flex items-center gap-2">
                <div className={`h-2 w-2 rounded-full ${getHealthStatusColor(dashboardData.system_health.api_status)}`}></div>
                <span className={`text-sm ${getHealthStatusTextColor(dashboardData.system_health.api_status)}`}>
                  {getHealthStatusText(dashboardData.system_health.api_status)}
                </span>
              </div>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">Authentication Service</span>
              <div className="flex items-center gap-2">
                <div className={`h-2 w-2 rounded-full ${getHealthStatusColor(dashboardData.system_health.auth_status)}`}></div>
                <span className={`text-sm ${getHealthStatusTextColor(dashboardData.system_health.auth_status)}`}>
                  {getHealthStatusText(dashboardData.system_health.auth_status)}
                </span>
              </div>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">Storage Usage</span>
              <span className="text-sm">
                {formatStorageSize(dashboardData.system_health.storage_usage.used_bytes)} GB / {formatStorageSize(dashboardData.system_health.storage_usage.total_bytes)} GB
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">Plugin Runtime</span>
              <div className="flex items-center gap-2">
                <div className={`h-2 w-2 rounded-full ${getHealthStatusColor(dashboardData.system_health.plugin_status)}`}></div>
                <span className={`text-sm ${getHealthStatusTextColor(dashboardData.system_health.plugin_status)}`}>
                  {getHealthStatusText(dashboardData.system_health.plugin_status)}
                </span>
              </div>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">Virtual File System</span>
              <div className="flex items-center gap-2">
                <div className={`h-2 w-2 rounded-full ${getHealthStatusColor(dashboardData.system_health.vfs_status)}`}></div>
                <span className={`text-sm ${getHealthStatusTextColor(dashboardData.system_health.vfs_status)}`}>
                  {getHealthStatusText(dashboardData.system_health.vfs_status)}
                </span>
              </div>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">Uptime</span>
              <span className="text-sm">{formatDuration(Number(dashboardData.system_health.uptime_seconds))}</span>
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
          <CardContent className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-muted-foreground text-xs text-left">
                  <th className="pb-1 pr-2">Name</th>
                  <th className="pb-1 pr-2">Records</th>
                  <th className="pb-1 pr-2 hidden md:table-cell">Size (KB)</th>
                </tr>
              </thead>
              <tbody>
                {dashboardData.collection_stats.slice(0, 6).map((col) => (
                  <tr key={col.name} className="border-t last:border-b-0">
                    <td className="py-1 pr-2 font-medium break-all">{col.name}</td>
                    <td className="py-1 pr-2">{col.record_count.toLocaleString()}</td>
                    <td className="py-1 pr-2 hidden md:table-cell">{Math.round(Number(col.size_bytes) / 1024).toLocaleString()}</td>
                  </tr>
                ))}
              </tbody>
            </table>
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
                <div key={idx} className="flex items-center justify-between text-sm">
                  <span className="truncate max-w-[140px]" title={u.username}>{u.username}</span>
                  <span className="text-muted-foreground">{u.action_count.toLocaleString()}</span>
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
            <div className="grid grid-cols-2 gap-2 text-sm">
              <span className="text-muted-foreground">Requests (24h)</span>
              <span>{dashboardData.api_stats.requests_24h.toLocaleString()}</span>
              <span className="text-muted-foreground">Requests (7d)</span>
              <span>{dashboardData.api_stats.requests_7d.toLocaleString()}</span>
              <span className="text-muted-foreground">Avg Response (ms)</span>
              <span>{dashboardData.api_stats.avg_response_time_ms.toFixed(1)}</span>
              <span className="text-muted-foreground">Error Rate</span>
              <span>{dashboardData.api_stats.error_rate_percent.toFixed(2)}%</span>
            </div>
            <hr className="my-2" />
            <div>
              <p className="text-xs text-muted-foreground mb-1">Top Endpoints</p>
              <ul className="space-y-1 text-xs">
                {dashboardData.api_stats.top_endpoints.slice(0, 5).map((ep, idx) => (
                  <li key={idx} className="flex items-center justify-between">
                    <span className="truncate max-w-[120px]" title={`${ep.method} ${ep.path}`}>{ep.method} {ep.path}</span>
                    <span>{ep.request_count}</span>
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
                <div key={index} className="flex items-start space-x-3">
                  <div className="h-2 w-2 bg-blue-500 rounded-full mt-2"></div>
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
