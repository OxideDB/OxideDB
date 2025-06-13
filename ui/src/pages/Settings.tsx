import React from 'react';
import { Settings as SettingsIcon } from 'lucide-react';

const Settings: React.FC = () => {
  return (
    <div>
      <div className="flex justify-between items-center mb-8">
        <div>
          <h1 className="text-2xl font-bold text-gray-900">Settings</h1>
          <p className="text-gray-600 mt-1">Configure OxideDB settings and preferences</p>
        </div>
      </div>

      <div className="space-y-6">
        {/* Coming Soon */}
        <div className="card text-center py-12">
          <SettingsIcon className="mx-auto h-12 w-12 text-gray-400 mb-4" />
          <h3 className="text-lg font-medium text-gray-900 mb-2">Settings Coming Soon</h3>
          <p className="text-gray-600">
            Advanced configuration options and system settings will be available in a future update.
          </p>
        </div>

        {/* API Information */}
        <div className="card">
          <h3 className="text-lg font-medium text-gray-900 mb-4">API Information</h3>
          <div className="space-y-3 text-sm">
            <div className="flex justify-between">
              <span className="text-gray-600">API Base URL:</span>
              <span className="font-medium text-gray-900">http://localhost:8080</span>
            </div>
            <div className="flex justify-between">
              <span className="text-gray-600">Frontend Version:</span>
              <span className="font-medium text-gray-900">1.0.0</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default Settings; 