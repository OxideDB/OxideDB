import React, { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Eye, EyeOff, Database, AlertCircle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { useAuth } from '../contexts/AuthContext';
// import { apiService } from '../services/api';

const Login: React.FC = () => {
  const navigate = useNavigate();
  const { login, authCollections } = useAuth();
  const [formData, setFormData] = useState({
    collection: '',
    identifier: '',
    credential: '',
  });
  const [showPassword, setShowPassword] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Auto-select the first available collection when collections are loaded
  useEffect(() => {
    if (authCollections.length > 0 && !formData.collection) {
      // Prefer 'users' collection if available, otherwise use the first one
      const defaultCollection = authCollections.find(c => c.name === 'users') || authCollections[0];
      if (defaultCollection) {
        setFormData(prev => ({ ...prev, collection: defaultCollection.name }));
      }
    }
  }, [authCollections, formData.collection]);

  const selectedCollection = authCollections.find(c => c.name === formData.collection);

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const { name, value } = e.target;
    setFormData(prev => ({
      ...prev,
      [name]: value,
    }));
    // Clear error when user starts typing
    if (error) setError(null);
  };

  const handleCollectionChange = (collection: string) => {
    setFormData(prev => ({
      ...prev,
      collection,
      identifier: '', // Clear identifier when changing collections
    }));
    if (error) setError(null);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    if (!formData.collection || !formData.identifier || !formData.credential) {
      setError('Please fill in all fields');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      await login(formData.collection, formData.identifier, formData.credential);
      navigate('/collections');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed. Please check your credentials.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-background px-4">
      <div className="w-full max-w-md space-y-8">
        {/* Header */}
        <div className="text-center">
          <div className="flex justify-center mb-4">
            <div className="p-3 bg-primary/10 rounded-full">
              <Database className="h-8 w-8 text-primary" />
            </div>
          </div>
          <h1 className="text-3xl font-bold text-foreground">OxideDB Admin</h1>
          <p className="text-muted-foreground mt-2">
            Sign in to access the database administration panel
          </p>
        </div>

        {/* Login Form */}
        <Card>
          <CardHeader>
            <CardTitle>Sign In</CardTitle>
            <CardDescription>
              Select a collection and enter your credentials
            </CardDescription>
          </CardHeader>
          <CardContent>
            <form onSubmit={handleSubmit} className="space-y-4">
              {/* Error Alert */}
              {error && (
                <Alert variant="destructive">
                  <AlertCircle className="h-4 w-4" />
                  <AlertDescription>{error}</AlertDescription>
                </Alert>
              )}

                                            {/* Collection Selector */}
               <div className="space-y-2">
                 <Label htmlFor="collection">Collection</Label>
                 <Select
                   value={formData.collection}
                   onValueChange={handleCollectionChange}
                 >
                   <SelectTrigger disabled={loading}>
                     <SelectValue placeholder="Select a collection" />
                   </SelectTrigger>
                   <SelectContent>
                     {authCollections.map((collection) => (
                       <SelectItem key={collection.name} value={collection.name}>
                         {collection.name}
                       </SelectItem>
                     ))}
                   </SelectContent>
                 </Select>
               </div>

              {/* Identifier Field */}
              <div className="space-y-2">
                <Label htmlFor="identifier">
                  {selectedCollection?.identifier_field === 'email' ? 'Email' : 'Identifier'}
                </Label>
                <Input
                  id="identifier"
                  name="identifier"
                  type={selectedCollection?.identifier_field === 'email' ? 'email' : 'text'}
                  placeholder={selectedCollection?.identifier_field === 'email' ? 'admin@example.com' : 'Enter your identifier'}
                  value={formData.identifier}
                  onChange={handleInputChange}
                  disabled={loading}
                  required
                  autoComplete="username"
                  className="w-full"
                />
              </div>

                             {/* Credential Field */}
               <div className="space-y-2">
                 <Label htmlFor="credential">Credential</Label>
                 <div className="relative">
                   <Input
                     id="credential"
                     name="credential"
                     type={showPassword ? 'text' : 'password'}
                     placeholder="Enter your credential"
                     value={formData.credential}
                     onChange={handleInputChange}
                     disabled={loading}
                     required
                     autoComplete="current-password"
                     className="w-full pr-10"
                   />
                   <Button
                     type="button"
                     variant="ghost"
                     size="sm"
                     className="absolute right-0 top-0 h-full px-3 py-2 hover:bg-transparent"
                     onClick={() => setShowPassword(!showPassword)}
                     disabled={loading}
                   >
                     {showPassword ? (
                       <EyeOff className="h-4 w-4 text-muted-foreground" />
                     ) : (
                       <Eye className="h-4 w-4 text-muted-foreground" />
                     )}
                   </Button>
                 </div>
               </div>

              {/* Submit Button */}
              <Button
                type="submit"
                className="w-full"
                disabled={loading}
              >
                {loading ? 'Signing in...' : 'Sign In'}
              </Button>
            </form>

            {/* Demo Credentials */}
            <div className="mt-6 p-4 bg-muted rounded-lg">
              <h4 className="text-sm font-medium text-muted-foreground mb-2">
                Demo Credentials
              </h4>
              <div className="text-xs text-muted-foreground space-y-1">
                <div>
                  <strong>Superusers Collection:</strong> admin@example.com / secure_password_123
                </div>
                <div>
                  <strong>Users Collection:</strong> user@example.com / user_password_456
                </div>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Footer */}
        <div className="text-center text-sm text-muted-foreground">
          <p>OxideDB v1.0.0 - Secure Database Administration</p>
        </div>
      </div>
    </div>
  );
};

export default Login;