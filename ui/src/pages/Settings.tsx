import React, { useState, useEffect } from 'react';
import { useForm } from 'react-hook-form';
import { Settings as SettingsIcon, Save, RotateCcw, AlertTriangle, CheckCircle, TestTube } from 'lucide-react';
import PageLayout from '@/components/PageLayout';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import { Switch } from '@/components/ui/switch';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import PageTabs, { TabsContent, TabsTrigger } from '@/components/PageTabs';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Form, FormControl, FormDescription, FormField, FormItem, FormLabel, FormMessage } from '@/components/ui/form';
import { Separator } from '@/components/ui/separator';
import { Badge } from '@/components/ui/badge';
import { apiService } from '@/services/api';
import type { SiteSettings, UpdateSiteSettingsRequest, SettingsHealthStatus } from '@/types/api';

const Settings: React.FC = () => {
  const [settings, setSettings] = useState<SiteSettings | null>(null);
  const [health, setHealth] = useState<SettingsHealthStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState('branding');

  const form = useForm<UpdateSiteSettingsRequest>({
    defaultValues: {}
  });

  // Load settings on component mount
  useEffect(() => {
    loadSettings();
  }, []);

  // Update form when settings are loaded
  useEffect(() => {
    if (settings) {
      form.reset({
        branding: settings.branding,
        email: settings.email,
        general: settings.general,
        security: settings.security,
        system_info: {
          oxidedb_edition: settings.system_info.oxidedb_edition,
          environment: settings.system_info.environment,
          instance_name: settings.system_info.instance_name,
          license_key: settings.system_info.license_key,
        }
      });
    }
  }, [settings, form]);

  const loadSettings = async () => {
    try {
      setLoading(true);
      setError(null);
      const [settingsData, healthData] = await Promise.all([
        apiService.getSiteSettings(true),
        apiService.getSettingsHealth().catch(() => null) // Health might not be available
      ]);
      setSettings(settingsData);
      setHealth(healthData);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load settings');
    } finally {
      setLoading(false);
    }
  };

  const onSubmit = async (data: UpdateSiteSettingsRequest) => {
    try {
      setSaving(true);
      setError(null);
      await apiService.updateSiteSettings(data);
      setSuccess('Settings updated successfully');
      await loadSettings(); // Reload to get updated data
      setTimeout(() => setSuccess(null), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update settings');
    } finally {
      setSaving(false);
    }
  };

  const resetSettings = async () => {
    if (!confirm('Are you sure you want to reset all settings to defaults? This action cannot be undone.')) {
      return;
    }

    try {
      setSaving(true);
      setError(null);
      await apiService.resetSiteSettings();
      setSuccess('Settings reset to defaults');
      await loadSettings();
      setTimeout(() => setSuccess(null), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to reset settings');
    } finally {
      setSaving(false);
    }
  };

  const testEmailConfig = async () => {
    try {
      setTesting(true);
      setError(null);
      const result = await apiService.testEmailConfiguration();
      if (result) {
        setSuccess('Email test successful');
      } else {
        setError('Email test failed - check your SMTP configuration');
      }
      setTimeout(() => setSuccess(null), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Email test failed');
    } finally {
      setTesting(false);
    }
  };

  if (loading) {
    return (
      <PageLayout title="Settings" description="Configure OxideDB settings and preferences">
        <div className="flex items-center justify-center py-12">
          <div className="text-center">
            <SettingsIcon className="mx-auto h-8 w-8 animate-spin text-muted-foreground" />
            <p className="mt-2 text-sm text-muted-foreground">Loading settings...</p>
          </div>
        </div>
      </PageLayout>
    );
  }

  return (
    <PageLayout title="Settings" description="Configure OxideDB settings and preferences">
      <div className="space-y-6">
        {/* Health Status */}
        {health && !health.healthy && (
          <Alert>
            <AlertTriangle className="h-4 w-4" />
            <AlertDescription>
              Settings validation found {health.warnings.length} warning(s). 
              Some features may not work correctly.
            </AlertDescription>
          </Alert>
        )}

        {/* Status Messages */}
        {error && (
          <Alert variant="destructive">
            <AlertTriangle className="h-4 w-4" />
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        {success && (
          <Alert>
            <CheckCircle className="h-4 w-4" />
            <AlertDescription>{success}</AlertDescription>
          </Alert>
        )}

        {/* Settings Form */}
        <Form {...form}>
          <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-6">
            <PageTabs 
              value={activeTab} 
              onValueChange={setActiveTab}
              tabTriggers={
                <>
                  <TabsTrigger value="branding" className="h-12 px-4 sm:px-6 rounded-none border-b-2 border-transparent data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-primary data-[state=active]:shadow-none bg-transparent transition-all duration-200 text-sm font-medium hover:text-primary/80">Branding</TabsTrigger>
                  <TabsTrigger value="email" className="h-12 px-4 sm:px-6 rounded-none border-b-2 border-transparent data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-primary data-[state=active]:shadow-none bg-transparent transition-all duration-200 text-sm font-medium hover:text-primary/80">Email</TabsTrigger>
                  <TabsTrigger value="system" className="h-12 px-4 sm:px-6 rounded-none border-b-2 border-transparent data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-primary data-[state=active]:shadow-none bg-transparent transition-all duration-200 text-sm font-medium hover:text-primary/80">System</TabsTrigger>
                  <TabsTrigger value="general" className="h-12 px-4 sm:px-6 rounded-none border-b-2 border-transparent data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-primary data-[state=active]:shadow-none bg-transparent transition-all duration-200 text-sm font-medium hover:text-primary/80">General</TabsTrigger>
                  <TabsTrigger value="security" className="h-12 px-4 sm:px-6 rounded-none border-b-2 border-transparent data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-primary data-[state=active]:shadow-none bg-transparent transition-all duration-200 text-sm font-medium hover:text-primary/80">Security</TabsTrigger>
                </>
              }
            >
              {/* Branding Settings */}
              <TabsContent value="branding" className="space-y-4">
                <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                  <FormField
                    control={form.control}
                    name="branding.site_title"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Site Title</FormLabel>
                        <FormControl>
                          <Input placeholder="OxideDB" {...field} />
                        </FormControl>
                        <FormDescription>
                          The title displayed in the browser and UI.
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="branding.site_description"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Site Description</FormLabel>
                        <FormControl>
                          <Input placeholder="A hook-first database with plugin architecture" {...field} />
                        </FormControl>
                        <FormDescription>
                          A brief description or tagline for your site.
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="branding.primary_color"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Primary Color</FormLabel>
                        <FormControl>
                          <Input type="color" {...field} />
                        </FormControl>
                        <FormDescription>
                          Primary brand color (hex format).
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="branding.secondary_color"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Secondary Color</FormLabel>
                        <FormControl>
                          <Input type="color" {...field} />
                        </FormControl>
                        <FormDescription>
                          Secondary brand color (hex format).
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />
                </div>

                <FormField
                  control={form.control}
                  name="branding.footer_text"
                  render={({ field }) => (
                    <FormItem>
                      <FormLabel>Footer Text</FormLabel>
                      <FormControl>
                        <Textarea placeholder="Powered by OxideDB" {...field} />
                      </FormControl>
                      <FormDescription>
                        Text or HTML displayed in the footer.
                      </FormDescription>
                      <FormMessage />
                    </FormItem>
                  )}
                />
              </TabsContent>

              {/* Email Settings */}
              <TabsContent value="email" className="space-y-4">
                <FormField
                  control={form.control}
                  name="email.enabled"
                  render={({ field }) => (
                    <FormItem className="flex flex-row items-center justify-between rounded-lg border p-4">
                      <div className="space-y-0.5">
                        <FormLabel className="text-base">Enable Email</FormLabel>
                        <FormDescription>
                          Enable email functionality for notifications and user communications.
                        </FormDescription>
                      </div>
                      <FormControl>
                        <Switch
                          checked={field.value}
                          onCheckedChange={field.onChange}
                        />
                      </FormControl>
                    </FormItem>
                  )}
                />

                <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                  <FormField
                    control={form.control}
                    name="email.smtp_host"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>SMTP Host</FormLabel>
                        <FormControl>
                          <Input placeholder="smtp.gmail.com" {...field} />
                        </FormControl>
                        <FormDescription>
                          Your SMTP server hostname.
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="email.smtp_port"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>SMTP Port</FormLabel>
                        <FormControl>
                          <Input 
                            type="number" 
                            placeholder="587" 
                            {...field}
                            onChange={(e) => field.onChange(parseInt(e.target.value) || 587)}
                          />
                        </FormControl>
                        <FormDescription>
                          SMTP port (usually 587 for TLS or 465 for SSL).
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="email.from_email"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>From Email</FormLabel>
                        <FormControl>
                          <Input type="email" placeholder="noreply@example.com" {...field} />
                        </FormControl>
                        <FormDescription>
                          Email address used for sending system emails.
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="email.from_name"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>From Name</FormLabel>
                        <FormControl>
                          <Input placeholder="OxideDB" {...field} />
                        </FormControl>
                        <FormDescription>
                          Display name for system emails.
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />
                </div>

                <div className="flex items-center gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    onClick={testEmailConfig}
                    disabled={testing || !form.watch('email.enabled')}
                  >
                    <TestTube className="mr-2 h-4 w-4" />
                    {testing ? 'Testing...' : 'Test Email Configuration'}
                  </Button>
                  {health && health.email_config_valid && (
                    <Badge variant="default">
                      <CheckCircle className="mr-1 h-3 w-3" />
                      Valid
                    </Badge>
                  )}
                </div>
              </TabsContent>

              {/* System Settings */}
              <TabsContent value="system" className="space-y-4">
                <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                  <FormField
                    control={form.control}
                    name="system_info.oxidedb_edition"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>OxideDB Edition</FormLabel>
                        <Select onValueChange={field.onChange} defaultValue={field.value}>
                          <FormControl>
                            <SelectTrigger>
                              <SelectValue placeholder="Select edition" />
                            </SelectTrigger>
                          </FormControl>
                          <SelectContent>
                            <SelectItem value="Community">Community</SelectItem>
                            <SelectItem value="Professional">Professional</SelectItem>
                            <SelectItem value="Enterprise">Enterprise</SelectItem>
                          </SelectContent>
                        </Select>
                        <FormDescription>
                          The OxideDB edition you are using.
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="system_info.environment"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Environment</FormLabel>
                        <Select onValueChange={field.onChange} defaultValue={field.value}>
                          <FormControl>
                            <SelectTrigger>
                              <SelectValue placeholder="Select environment" />
                            </SelectTrigger>
                          </FormControl>
                          <SelectContent>
                            <SelectItem value="Development">Development</SelectItem>
                            <SelectItem value="Staging">Staging</SelectItem>
                            <SelectItem value="Production">Production</SelectItem>
                          </SelectContent>
                        </Select>
                        <FormDescription>
                          The deployment environment.
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="system_info.instance_name"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Instance Name</FormLabel>
                        <FormControl>
                          <Input placeholder="my-oxidedb-instance" {...field} />
                        </FormControl>
                        <FormDescription>
                          Custom name for this OxideDB instance.
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />
                </div>

                {settings && (
                  <div className="rounded-lg border p-4 space-y-2">
                    <h3 className="font-medium">System Information</h3>
                    <div className="grid grid-cols-1 gap-2 text-sm">
                      <div className="flex justify-between">
                        <span className="text-muted-foreground">Version:</span>
                        <span>{settings.system_info.oxidedb_version}</span>
                      </div>
                      <div className="flex justify-between">
                        <span className="text-muted-foreground">Installation ID:</span>
                        <span className="font-mono text-xs">{settings.system_info.installation_id}</span>
                      </div>
                    </div>
                  </div>
                )}
              </TabsContent>

              {/* General Settings */}
              <TabsContent value="general" className="space-y-4">
                <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                  <FormField
                    control={form.control}
                    name="general.default_timezone"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Default Timezone</FormLabel>
                        <FormControl>
                          <Input placeholder="UTC" {...field} />
                        </FormControl>
                        <FormDescription>
                          Default timezone for the application.
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="general.default_locale"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Default Locale</FormLabel>
                        <FormControl>
                          <Input placeholder="en-US" {...field} />
                        </FormControl>
                        <FormDescription>
                          Default language/locale for the application.
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="general.max_upload_size"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Max Upload Size (bytes)</FormLabel>
                        <FormControl>
                          <Input 
                            type="number" 
                            placeholder="10485760" 
                            {...field}
                            onChange={(e) => field.onChange(parseInt(e.target.value) || 10485760)}
                          />
                        </FormControl>
                        <FormDescription>
                          Maximum file upload size in bytes (10MB = 10485760).
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="general.api_rate_limit"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>API Rate Limit</FormLabel>
                        <FormControl>
                          <Input 
                            type="number" 
                            placeholder="1000" 
                            {...field}
                            onChange={(e) => field.onChange(parseInt(e.target.value) || 1000)}
                          />
                        </FormControl>
                        <FormDescription>
                          Maximum API requests per minute per user.
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />
                </div>

                <Separator />

                <div className="space-y-4">
                  <h3 className="text-lg font-medium">Access Control</h3>
                  
                  <FormField
                    control={form.control}
                    name="general.allow_user_registration"
                    render={({ field }) => (
                      <FormItem className="flex flex-row items-center justify-between rounded-lg border p-4">
                        <div className="space-y-0.5">
                          <FormLabel className="text-base">Allow User Registration</FormLabel>
                          <FormDescription>
                            Allow new users to register accounts.
                          </FormDescription>
                        </div>
                        <FormControl>
                          <Switch
                            checked={field.value}
                            onCheckedChange={field.onChange}
                          />
                        </FormControl>
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="general.allow_public_api"
                    render={({ field }) => (
                      <FormItem className="flex flex-row items-center justify-between rounded-lg border p-4">
                        <div className="space-y-0.5">
                          <FormLabel className="text-base">Allow Public API Access</FormLabel>
                          <FormDescription>
                            Allow unauthenticated access to public API endpoints.
                          </FormDescription>
                        </div>
                        <FormControl>
                          <Switch
                            checked={field.value}
                            onCheckedChange={field.onChange}
                          />
                        </FormControl>
                      </FormItem>
                    )}
                  />
                </div>
              </TabsContent>

              {/* Security Settings */}
              <TabsContent value="security" className="space-y-4">
                <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                  <FormField
                    control={form.control}
                    name="security.password_min_length"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Minimum Password Length</FormLabel>
                        <FormControl>
                          <Input 
                            type="number" 
                            min="4" 
                            max="128" 
                            {...field}
                            onChange={(e) => field.onChange(parseInt(e.target.value) || 8)}
                          />
                        </FormControl>
                        <FormDescription>
                          Minimum required password length (4-128 characters).
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="security.session_timeout_minutes"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Session Timeout (minutes)</FormLabel>
                        <FormControl>
                          <Input 
                            type="number" 
                            min="5" 
                            {...field}
                            onChange={(e) => field.onChange(parseInt(e.target.value) || 480)}
                          />
                        </FormControl>
                        <FormDescription>
                          How long user sessions remain active (in minutes).
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="security.max_login_attempts"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Max Login Attempts</FormLabel>
                        <FormControl>
                          <Input 
                            type="number" 
                            min="1" 
                            {...field}
                            onChange={(e) => field.onChange(parseInt(e.target.value) || 5)}
                          />
                        </FormControl>
                        <FormDescription>
                          Maximum failed login attempts before account lockout.
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="security.lockout_duration_minutes"
                    render={({ field }) => (
                      <FormItem>
                        <FormLabel>Lockout Duration (minutes)</FormLabel>
                        <FormControl>
                          <Input 
                            type="number" 
                            min="1" 
                            {...field}
                            onChange={(e) => field.onChange(parseInt(e.target.value) || 15)}
                          />
                        </FormControl>
                        <FormDescription>
                          How long accounts remain locked after max attempts.
                        </FormDescription>
                        <FormMessage />
                      </FormItem>
                    )}
                  />
                </div>

                <Separator />

                <div className="space-y-4">
                  <h3 className="text-lg font-medium">Security Features</h3>
                  
                  <FormField
                    control={form.control}
                    name="security.require_email_verification"
                    render={({ field }) => (
                      <FormItem className="flex flex-row items-center justify-between rounded-lg border p-4">
                        <div className="space-y-0.5">
                          <FormLabel className="text-base">Require Email Verification</FormLabel>
                          <FormDescription>
                            Require users to verify their email addresses.
                          </FormDescription>
                        </div>
                        <FormControl>
                          <Switch
                            checked={field.value}
                            onCheckedChange={field.onChange}
                          />
                        </FormControl>
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="security.password_require_complexity"
                    render={({ field }) => (
                      <FormItem className="flex flex-row items-center justify-between rounded-lg border p-4">
                        <div className="space-y-0.5">
                          <FormLabel className="text-base">Require Password Complexity</FormLabel>
                          <FormDescription>
                            Require passwords to include uppercase, lowercase, numbers, and symbols.
                          </FormDescription>
                        </div>
                        <FormControl>
                          <Switch
                            checked={field.value}
                            onCheckedChange={field.onChange}
                          />
                        </FormControl>
                      </FormItem>
                    )}
                  />

                  <FormField
                    control={form.control}
                    name="security.enable_audit_logging"
                    render={({ field }) => (
                      <FormItem className="flex flex-row items-center justify-between rounded-lg border p-4">
                        <div className="space-y-0.5">
                          <FormLabel className="text-base">Enable Audit Logging</FormLabel>
                          <FormDescription>
                            Log security-relevant events for compliance and monitoring.
                          </FormDescription>
                        </div>
                        <FormControl>
                          <Switch
                            checked={field.value}
                            onCheckedChange={field.onChange}
                          />
                        </FormControl>
                      </FormItem>
                    )}
                  />
                </div>
              </TabsContent>
            </PageTabs>

            {/* Action Buttons */}
            <div className="flex items-center gap-4 pt-6">
              <Button type="submit" disabled={saving}>
                <Save className="mr-2 h-4 w-4" />
                {saving ? 'Saving...' : 'Save Settings'}
              </Button>
              
              <Button 
                type="button" 
                variant="outline" 
                onClick={resetSettings}
                disabled={saving}
              >
                <RotateCcw className="mr-2 h-4 w-4" />
                Reset to Defaults
              </Button>
            </div>
          </form>
        </Form>
      </div>
    </PageLayout>
  );
};

export default Settings; 