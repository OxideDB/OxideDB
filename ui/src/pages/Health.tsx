import React, { useState, useEffect } from 'react';
import { CheckCircle, XCircle, RefreshCw } from 'lucide-react';
import { apiService } from '../services/api';
import type { HealthStatus } from '../types/api';

const Health: React.FC = () => {
  const [health, setHealth] = useState<HealthStatus | null>(null);
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

  const isHealthy = health?.status === 'healthy' && health?.database.includes('healthy');

  return (
    <div>
      <div className="flex justify-between items-center mb-8">
        <div>
          <h1 className="text-2xl font-bold text-gray-900">System Health</h1>
          <p className="text-gray-600 mt-1">Monitor the status of OxideDB components</p>
        </div>
        <button
          onClick={fetchHealth}
          disabled={loading}
          className="btn-secondary flex items-center"
        >
          <RefreshCw className={`h-4 w-4 mr-2 ${loading ? 'animate-spin' : ''}`} />
          Refresh
        </button>
      </div>

      {error && (
        <div className="bg-red-50 border border-red-200 rounded-lg p-4 mb-6">
          <div className="flex items-center">
            <XCircle className="h-5 w-5 text-red-500 mr-2" />
            <div className="text-red-800">{error}</div>
          </div>
          <button
            onClick={() => setError(null)}
            className="text-red-600 text-sm mt-2 hover:text-red-800"
          >
            Dismiss
          </button>
        </div>
      )}

      {loading && !health ? (
        <div className="flex items-center justify-center h-64">
          <div className="text-gray-500">Loading health status...</div>
        </div>
      ) : health ? (
        <div className="space-y-6">
          {/* Overall Status */}
          <div className="card">
            <div className="flex items-center justify-between">
              <div>
                <h3 className="text-lg font-medium text-gray-900">Overall Status</h3>
                <p className="text-sm text-gray-600 mt-1">System-wide health check</p>
              </div>
              <div className="flex items-center">
                {isHealthy ? (
                  <CheckCircle className="h-8 w-8 text-green-500" />
                ) : (
                  <XCircle className="h-8 w-8 text-red-500" />
                )}
                <span className={`ml-2 text-lg font-medium ${
                  isHealthy ? 'text-green-700' : 'text-red-700'
                }`}>
                  {isHealthy ? 'Healthy' : 'Unhealthy'}
                </span>
              </div>
            </div>
          </div>

          {/* Component Status */}
          <div className="grid gap-6 md:grid-cols-2">
            {/* API Status */}
            <div className="card">
              <div className="flex items-center justify-between mb-4">
                <h3 className="text-lg font-medium text-gray-900">API Server</h3>
                {health.status === 'healthy' ? (
                  <CheckCircle className="h-6 w-6 text-green-500" />
                ) : (
                  <XCircle className="h-6 w-6 text-red-500" />
                )}
              </div>
              <div className="space-y-2 text-sm">
                <div className="flex justify-between">
                  <span className="text-gray-600">Status:</span>
                  <span className={`font-medium ${
                    health.status === 'healthy' ? 'text-green-700' : 'text-red-700'
                  }`}>
                    {health.status}
                  </span>
                </div>

              </div>
            </div>

            {/* Database Status */}
            <div className="card">
              <div className="flex items-center justify-between mb-4">
                <h3 className="text-lg font-medium text-gray-900">Database</h3>
                {health.database.includes('healthy') ? (
                  <CheckCircle className="h-6 w-6 text-green-500" />
                ) : (
                  <XCircle className="h-6 w-6 text-red-500" />
                )}
              </div>
              <div className="space-y-2 text-sm">
                <div className="flex justify-between">
                  <span className="text-gray-600">Status:</span>
                  <span className={`font-medium ${
                    health.database.includes('healthy') ? 'text-green-700' : 'text-red-700'
                  }`}>
                    {health.database}
                  </span>
                </div>
              </div>
            </div>
          </div>

          {/* Health Details */}
          <div className="card">
            <h3 className="text-lg font-medium text-gray-900 mb-4">Health Details</h3>
            <div className="bg-gray-50 rounded-lg p-4">
              <pre className="text-sm text-gray-800 whitespace-pre-wrap">
                {JSON.stringify(health, null, 2)}
              </pre>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
};

export default Health; 