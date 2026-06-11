import React from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../ui/card';
import { Button } from '../ui/button';
import { Input } from '../ui/input';
import { Label } from '../ui/label';
import { Separator } from '../ui/separator';
import { Tooltip, TooltipContent, TooltipTrigger } from '../ui/tooltip';
import { 
  Search, TrendingUp, Info, AlertTriangle, Shield, User
} from 'lucide-react';

interface SearchTabProps {
  selectedCorrelationId: string;
  loading: boolean;
  onCorrelationIdChange: (id: string) => void;
  onCorrelationSearch: () => void;
  onQuickSearch: (type: 'errors' | 'security' | 'auth') => void;
}

export const SearchTab: React.FC<SearchTabProps> = ({
  selectedCorrelationId,
  loading,
  onCorrelationIdChange,
  onCorrelationSearch,
  onQuickSearch
}) => {
  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && selectedCorrelationId.trim()) {
      onCorrelationSearch();
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center space-x-2">
          <Search className="h-5 w-5" />
          <span>Advanced Search</span>
        </CardTitle>
        <CardDescription>
          Search logs by correlation ID and other advanced criteria
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-6">
        {/* Correlation ID Search */}
        <div className="space-y-3">
          <Label htmlFor="correlationId">Correlation ID</Label>
          <div className="flex items-end space-x-2">
            <div className="flex-1">
              <Input
                id="correlationId"
                placeholder="Enter correlation ID..."
                value={selectedCorrelationId}
                onChange={(e) => onCorrelationIdChange(e.target.value)}
                onKeyPress={handleKeyPress}
              />
            </div>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button 
                  onClick={onCorrelationSearch} 
                  disabled={loading || !selectedCorrelationId.trim()}
                  className="shrink-0"
                >
                  <Search className="h-4 w-4 mr-2" />
                  Search
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                Search for all logs related to this correlation ID
                {selectedCorrelationId.trim() && (
                  <div className="text-xs">Press Enter to search quickly</div>
                )}
              </TooltipContent>
            </Tooltip>
          </div>
          <p className="text-sm text-muted-foreground">
            Search for all logs related to a specific operation or request
          </p>
        </div>

        <Separator />

        {/* Quick Searches and Tips */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          <Card>
            <CardHeader>
              <CardTitle className="text-lg flex items-center space-x-2">
                <TrendingUp className="h-5 w-5" />
                <span>Quick Searches</span>
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="outline"
                    className="w-full justify-start hover:bg-red-50 dark:hover:bg-red-950"
                    onClick={() => onQuickSearch('errors')}
                  >
                    <AlertTriangle className="h-4 w-4 mr-2 text-red-500" />
                    Recent Errors
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  View recent error-level log entries for troubleshooting
                </TooltipContent>
              </Tooltip>

              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="outline"
                    className="w-full justify-start hover:bg-orange-50 dark:hover:bg-orange-950"
                    onClick={() => onQuickSearch('security')}
                  >
                    <Shield className="h-4 w-4 mr-2 text-orange-500" />
                    Security Violations
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  View security violation events for threat monitoring
                </TooltipContent>
              </Tooltip>

              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="outline"
                    className="w-full justify-start hover:bg-blue-50 dark:hover:bg-blue-950"
                    onClick={() => onQuickSearch('auth')}
                  >
                    <User className="h-4 w-4 mr-2 text-blue-500" />
                    Authentication Events
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  View authentication events and login activities
                </TooltipContent>
              </Tooltip>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="text-lg flex items-center space-x-2">
                <Info className="h-5 w-5" />
                <span>Search Tips</span>
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-3 text-sm text-muted-foreground">
              <div className="flex items-start space-x-2">
                <div className="w-1.5 h-1.5 rounded-full bg-blue-500 mt-2 flex-shrink-0"></div>
                <p>Use correlation IDs to trace requests across services and components</p>
              </div>
              <div className="flex items-start space-x-2">
                <div className="w-1.5 h-1.5 rounded-full bg-green-500 mt-2 flex-shrink-0"></div>
                <p>Filter by log level to focus on specific severity levels</p>
              </div>
              <div className="flex items-start space-x-2">
                <div className="w-1.5 h-1.5 rounded-full bg-purple-500 mt-2 flex-shrink-0"></div>
                <p>Search audit events by type for security monitoring</p>
              </div>
              <div className="flex items-start space-x-2">
                <div className="w-1.5 h-1.5 rounded-full bg-orange-500 mt-2 flex-shrink-0"></div>
                <p>Use time ranges to narrow down investigation periods</p>
              </div>
            </CardContent>
          </Card>
        </div>
      </CardContent>
    </Card>
  );
}; 
