import React from "react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Database, Users, Activity, TrendingUp, Server } from "lucide-react";
import PageLayout from "@/components/PageLayout";

const Dashboard: React.FC = () => {
  const stats = [
    {
      title: "Total Collections",
      value: "12",
      description: "Active database collections",
      icon: Database,
      trend: "+2 this month",
    },
    {
      title: "Total Records",
      value: "1,247",
      description: "Records across all collections",
      icon: Activity,
      trend: "+18% from last month",
    },
    {
      title: "Active Users",
      value: "23",
      description: "Users with database access",
      icon: Users,
      trend: "+3 new users",
    },
    {
      title: "API Requests",
      value: "8,429",
      description: "Requests in the last 24h",
      icon: TrendingUp,
      trend: "+12% from yesterday",
    },
  ];

  const recentActivity = [
    {
      action: "Created collection 'user_profiles'",
      user: "admin",
      time: "2 minutes ago",
    },
    {
      action: "Added 15 records to 'products'",
      user: "john.doe",
      time: "5 minutes ago",
    },
    {
      action: "Updated schema for 'orders'",
      user: "jane.smith",
      time: "10 minutes ago",
    },
    {
      action: "Created new user 'mike.wilson'",
      user: "admin",
      time: "15 minutes ago",
    },
  ];

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
                <div className="h-2 w-2 bg-green-500 rounded-full"></div>
                <span className="text-sm text-green-600">Healthy</span>
              </div>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">API Endpoints</span>
              <div className="flex items-center gap-2">
                <div className="h-2 w-2 bg-green-500 rounded-full"></div>
                <span className="text-sm text-green-600">Online</span>
              </div>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">Authentication Service</span>
              <div className="flex items-center gap-2">
                <div className="h-2 w-2 bg-green-500 rounded-full"></div>
                <span className="text-sm text-green-600">Active</span>
              </div>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-sm">Storage Usage</span>
              <span className="text-sm">2.4 GB / 10 GB</span>
            </div>
          </CardContent>
        </Card>

        {/* Recent Activity */}
        <Card>
          <CardHeader>
            <CardTitle>Recent Activity</CardTitle>
            <CardDescription>Latest database operations</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              {recentActivity.map((activity, index) => (
                <div key={index} className="flex items-start space-x-3">
                  <div className="h-2 w-2 bg-blue-500 rounded-full mt-2"></div>
                  <div className="flex-1 space-y-1 min-w-0">
                    <p className="text-sm break-words">{activity.action}</p>
                    <div className="flex flex-col sm:flex-row sm:items-center gap-1 sm:gap-2 text-xs text-muted-foreground">
                      <span>{activity.user}</span>
                      <span className="hidden sm:inline">•</span>
                      <span>{activity.time}</span>
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
