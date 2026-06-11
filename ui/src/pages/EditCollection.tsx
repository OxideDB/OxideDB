import React, { useState, useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { ArrowLeft, Save, Database, Settings, Shield, ChevronDown, ChevronRight } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Separator } from '@/components/ui/separator';
import { Checkbox } from '@/components/ui/checkbox';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';
import PageLayout from '@/components/PageLayout';
import { apiService } from '../services/api';
import type { CollectionSchema, FieldDefinition } from '../types/api';
import { parseFieldDefaultValue } from '../utils/fieldDefaults';
import { useFieldManagement, FieldsList, convertSchemaFieldsToFormData } from '@/components/collection-form';

// Auth collection configuration interface
interface AuthCollectionConfig {
  identifierField: string;
  credentialField: string;
  registrationEnabled: boolean;
  emailVerificationRequired: boolean;
  refreshTokensEnabled: boolean;
  refreshTokensRequired: boolean;
  customClaimFields: string[];
}

type CollectionSchemaWithAuthConfig = CollectionSchema & {
  auth_config?: Partial<AuthCollectionConfig>;
};

const EditCollection: React.FC = () => {
  const { collection } = useParams<{ collection: string }>();
  const navigate = useNavigate();
  const [schema, setSchema] = useState<CollectionSchema | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [collections, setCollections] = useState<CollectionSchema[]>([]);
  const [showAdvanced, setShowAdvanced] = useState(false);
  
  // Use the field management hook
  const fieldManagement = useFieldManagement();
  
  // Auth collection configuration
  const [authConfig, setAuthConfig] = useState<AuthCollectionConfig>({
    identifierField: 'email',
    credentialField: 'password',
    registrationEnabled: true,
    emailVerificationRequired: false,
    refreshTokensEnabled: false,
    refreshTokensRequired: false,
    customClaimFields: [],
  });
  const [newClaimField, setNewClaimField] = useState('');

  useEffect(() => {
    if (collection) {
      fetchSchema();
    }
    loadCollections();
  }, [collection]);

  // Auto-expand auth configuration for auth collections
  useEffect(() => {
    if (schema?.collection_type === 'auth' && !showAdvanced) {
      setShowAdvanced(true);
    }
  }, [schema?.collection_type]);

  const loadCollections = async () => {
    try {
      const collections = await apiService.getCollections();
      setCollections(collections);
    } catch (err) {
      console.error('Failed to load collections:', err);
    }
  };

  const fetchSchema = async () => {
    if (!collection) return;

    try {
      setLoading(true);
      const schemaData = await apiService.getCollectionSchema(collection);
      setSchema(schemaData);
      
      // Convert schema fields to form data using the utility
      const formFields = convertSchemaFieldsToFormData(schemaData.fields);
      
      // Initialize field management with existing fields
      fieldManagement.setFieldsFromSchema(formFields);
      
      // Extract auth config if this is an auth collection
      const schemaWithAuthConfig = schemaData as CollectionSchemaWithAuthConfig;
      if (schemaData.collection_type === 'auth' && schemaWithAuthConfig.auth_config) {
        const authConfigData = schemaWithAuthConfig.auth_config;
        setAuthConfig({
          identifierField: authConfigData.identifierField || 'email',
          credentialField: authConfigData.credentialField || 'password',
          registrationEnabled: authConfigData.registrationEnabled !== false,
          emailVerificationRequired: authConfigData.emailVerificationRequired || false,
          refreshTokensEnabled: authConfigData.refreshTokensEnabled || false,
          refreshTokensRequired: authConfigData.refreshTokensRequired || false,
          customClaimFields: authConfigData.customClaimFields || [],
        });
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch collection schema');
    } finally {
      setLoading(false);
    }
  };

  // Auth config helper functions
  const addCustomClaimField = () => {
    if (newClaimField.trim() && !authConfig.customClaimFields.includes(newClaimField.trim())) {
      setAuthConfig(prev => ({
        ...prev,
        customClaimFields: [...prev.customClaimFields, newClaimField.trim()]
      }));
      setNewClaimField('');
    }
  };

  const removeCustomClaimField = (field: string) => {
    setAuthConfig(prev => ({
      ...prev,
      customClaimFields: prev.customClaimFields.filter(f => f !== field)
    }));
  };

  const handleUpdateSchema = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!schema || !collection) return;

    try {
      setSaving(true);
      
      // Convert fields to the required format
      const fieldsMap: Record<string, FieldDefinition> = {};
      fieldManagement.fields.forEach(field => {
        if (field.name.trim()) {
          // Parse default value based on field type using utility function
          const defaultValue = parseFieldDefaultValue(field.default, field.field_type);

          fieldsMap[field.name.trim()] = {
            field_type: field.field_type,
            required: field.required,
            unique: field.unique,
            index: false,
            default: defaultValue,
            validation: field.validation ? { 
              regex: field.validation.regex ?? null,
              min: field.validation.min ?? null,
              max: field.validation.max ?? null,
              message: field.validation.message ?? null,
              allow_empty: field.validation.allow_empty !== undefined ? field.validation.allow_empty : null
            } : null
          };
        }
      });

      const updatedSchema: CollectionSchema = {
        ...schema,
        fields: fieldsMap,
        version: (schema.version || 1) + 1, // Increment version for schema update (default to 1 if undefined)
        updated_at: BigInt(Math.floor(Date.now() / 1000)),
        // Include auth config for auth collections
        ...(schema.collection_type === 'auth' ? { auth_config: authConfig } : {})
      };

      await apiService.updateCollectionSchema(collection, updatedSchema);
      navigate(`/collections/${encodeURIComponent(collection)}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update collection schema');
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <PageLayout title="Loading..." description="Loading collection schema...">
        <div className="flex items-center justify-center h-64">
          <div className="text-muted-foreground">Loading collection schema...</div>
        </div>
      </PageLayout>
    );
  }

  if (!schema) {
    const leftActions = (
      <Button
        onClick={() => navigate('/collections')}
        variant="ghost"
        size="sm"
      >
        <ArrowLeft className="h-4 w-4 mr-2" />
        Back to Collections
      </Button>
    );

    return (
      <PageLayout 
        title="Collection Not Found" 
        description="The requested collection could not be found"
        leftActions={leftActions}
      >
        <div className="max-w-4xl mx-auto">
          {/* Content if needed */}
        </div>
      </PageLayout>
    );
  }

  const leftActions = (
    <Button
      onClick={() => navigate(`/collections/${encodeURIComponent(collection!)}`)}
      variant="ghost"
      size="sm"
    >
      <ArrowLeft className="h-4 w-4 mr-2" />
      Back to Records
    </Button>
  );

  return (
    <PageLayout 
      title="Edit Collection Schema" 
      description={`Update schema for "${collection}"`}
      leftActions={leftActions}
    >
      <div className="max-w-5xl mx-auto space-y-6">

        {/* Error Display */}
        {error && (
          <Alert variant="destructive">
            <AlertDescription className="flex items-center justify-between">
              <span>{error}</span>
              <Button
                onClick={() => setError(null)}
                variant="ghost"
                size="sm"
                className="text-destructive hover:text-destructive/80 h-auto p-1"
              >
                ×
              </Button>
            </AlertDescription>
          </Alert>
        )}

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <Settings className="h-5 w-5" />
                <span>Edit Collection Schema</span>
              </div>
              <div className="flex items-center space-x-2">
                <Badge variant="outline" className="capitalize">
                  {schema?.collection_type}
                </Badge>
                <Badge variant="secondary">
                  {fieldManagement.fields.length} field{fieldManagement.fields.length !== 1 ? 's' : ''}
                </Badge>
              </div>
            </CardTitle>
          </CardHeader>
        <CardContent>
          <form onSubmit={handleUpdateSchema} className="space-y-6">
            {/* Collection Info */}
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
              <div className="space-y-4">
                <div className="space-y-2">
                  <Label className="text-base font-medium">Collection Name</Label>
                  <Input
                    value={schema.name}
                    disabled
                    className="bg-muted text-base"
                  />
                  <p className="text-sm text-muted-foreground">
                    Collection name cannot be changed after creation
                  </p>
                </div>
              </div>
              
              <div className="space-y-4">
                <div className="space-y-2">
                  <Label className="text-base font-medium">Collection Type</Label>
                  <div className="flex items-center space-x-2 p-3 bg-muted rounded-md">
                    {schema.collection_type === 'auth' ? (
                      <>
                        <Shield className="h-4 w-4" />
                        <span className="font-medium">Auth Collection</span>
                        <Badge variant="secondary" className="text-xs">Security</Badge>
                      </>
                    ) : (
                      <>
                        <Database className="h-4 w-4" />
                        <span className="font-medium">Base Collection</span>
                      </>
                    )}
                  </div>
                  <p className="text-sm text-muted-foreground">
                    Collection type cannot be changed after creation
                  </p>
                </div>
              </div>
            </div>

            <Separator />

            {/* Schema fields */}
            <div className="space-y-4">
              <div className="flex justify-between items-center">
                <Label className="text-base font-medium">Schema Fields</Label>
                <Badge variant="outline">
                  {fieldManagement.fields.length} field{fieldManagement.fields.length !== 1 ? 's' : ''}
                </Badge>
              </div>

              <FieldsList
                fields={fieldManagement.fields}
                collections={collections}
                onAddField={fieldManagement.addField}
                onUpdateField={fieldManagement.updateField}
                onUpdateFieldType={fieldManagement.updateFieldType}
                onUpdateRelationshipConfig={fieldManagement.updateRelationshipConfig}
                onUpdateFileConfig={fieldManagement.updateFileConfig}
                onUpdateSelectConfig={fieldManagement.updateSelectConfig}
                onUpdateValidationConfig={fieldManagement.updateValidationConfig}
                onRemoveField={fieldManagement.removeField}
              />
            </div>

            {/* Auth Collection Info */}
            {schema.collection_type === 'auth' && (
              <Alert>
                <Shield className="h-4 w-4" />
                <AlertDescription>
                  This is an authentication collection with built-in security features. 
                  You can customize authentication settings and security options below.
                </AlertDescription>
              </Alert>
            )}

            {/* Advanced Auth Configuration */}
            {schema.collection_type === 'auth' && (
              <Collapsible open={showAdvanced} onOpenChange={setShowAdvanced}>
                <Card>
                  <CollapsibleTrigger asChild>
                    <CardHeader className="cursor-pointer hover:bg-muted/50 transition-colors">
                      <CardTitle className="flex items-center justify-between">
                        <div className="flex items-center space-x-2">
                          <Shield className="h-5 w-5" />
                          <span>Advanced Authentication Settings</span>
                        </div>
                        <div className="flex items-center space-x-2">
                          <Badge variant="secondary">Optional</Badge>
                          {showAdvanced ? (
                            <ChevronDown className="h-4 w-4" />
                          ) : (
                            <ChevronRight className="h-4 w-4" />
                          )}
                        </div>
                      </CardTitle>
                    </CardHeader>
                  </CollapsibleTrigger>
                  
                  <CollapsibleContent>
                    <CardContent className="space-y-6">
                      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                        <div className="space-y-4">
                          <div className="space-y-2">
                            <Label htmlFor="identifierField" className="font-medium">
                              Identifier Field
                            </Label>
                            <Input
                              id="identifierField"
                              value={authConfig.identifierField}
                              onChange={(e) => setAuthConfig(prev => ({ ...prev, identifierField: e.target.value }))}
                              placeholder="email, username, etc."
                            />
                            <p className="text-xs text-muted-foreground">
                              Field used for login (usually email or username)
                            </p>
                          </div>

                          <div className="space-y-2">
                            <Label htmlFor="credentialField" className="font-medium">
                              Credential Field
                            </Label>
                            <Input
                              id="credentialField"
                              value={authConfig.credentialField}
                              onChange={(e) => setAuthConfig(prev => ({ ...prev, credentialField: e.target.value }))}
                              placeholder="password"
                            />
                            <p className="text-xs text-muted-foreground">
                              Field used for password storage
                            </p>
                          </div>
                        </div>

                        <div className="space-y-4">
                          <Label className="font-medium">Security Options</Label>
                          <div className="space-y-3">
                            <div className="flex items-center space-x-2">
                              <Checkbox
                                id="registrationEnabled"
                                checked={authConfig.registrationEnabled}
                                onCheckedChange={(checked) => setAuthConfig(prev => ({ ...prev, registrationEnabled: !!checked }))}
                              />
                              <Label htmlFor="registrationEnabled" className="font-normal">
                                Enable user registration
                              </Label>
                            </div>

                            <div className="flex items-center space-x-2">
                              <Checkbox
                                id="emailVerificationRequired"
                                checked={authConfig.emailVerificationRequired}
                                onCheckedChange={(checked) => setAuthConfig(prev => ({ ...prev, emailVerificationRequired: !!checked }))}
                              />
                              <Label htmlFor="emailVerificationRequired" className="font-normal">
                                Require email verification
                              </Label>
                            </div>

                            <div className="flex items-center space-x-2">
                              <Checkbox
                                id="refreshTokensEnabled"
                                checked={authConfig.refreshTokensEnabled}
                                onCheckedChange={(checked) => setAuthConfig(prev => ({ ...prev, refreshTokensEnabled: !!checked }))}
                              />
                              <Label htmlFor="refreshTokensEnabled" className="font-normal">
                                Enable refresh tokens
                              </Label>
                            </div>

                            {authConfig.refreshTokensEnabled && (
                              <div className="flex items-center space-x-2 ml-6">
                                <Checkbox
                                  id="refreshTokensRequired"
                                  checked={authConfig.refreshTokensRequired}
                                  onCheckedChange={(checked) => setAuthConfig(prev => ({ ...prev, refreshTokensRequired: !!checked }))}
                                />
                                <Label htmlFor="refreshTokensRequired" className="font-normal">
                                  Require refresh tokens
                                </Label>
                              </div>
                            )}
                          </div>
                        </div>
                      </div>

                      <Separator />

                      <div className="space-y-4">
                        <Label className="font-medium">Custom JWT Claim Fields</Label>
                        <div className="space-y-3">
                          {authConfig.customClaimFields.map((field, index) => (
                            <div key={index} className="flex items-center space-x-2">
                              <Input value={field} readOnly className="flex-1" />
                              <Button
                                type="button"
                                variant="outline"
                                size="sm"
                                onClick={() => removeCustomClaimField(field)}
                              >
                                Remove
                              </Button>
                            </div>
                          ))}
                          <div className="flex items-center space-x-2">
                            <Input
                              value={newClaimField}
                              onChange={(e) => setNewClaimField(e.target.value)}
                              placeholder="Field name to include in JWT"
                              className="flex-1"
                              onKeyPress={(e) => {
                                if (e.key === 'Enter') {
                                  e.preventDefault();
                                  addCustomClaimField();
                                }
                              }}
                            />
                            <Button
                              type="button"
                              variant="outline"
                              size="sm"
                              onClick={addCustomClaimField}
                              disabled={!newClaimField.trim()}
                            >
                              Add
                            </Button>
                          </div>
                        </div>
                        <p className="text-xs text-muted-foreground">
                          Additional fields to include in JWT tokens for this collection
                        </p>
                      </div>
                    </CardContent>
                  </CollapsibleContent>
                </Card>
              </Collapsible>
            )}

            <div className="flex justify-end space-x-2">
              <Button
                type="button"
                variant="outline"
                onClick={() => navigate(`/collections/${encodeURIComponent(collection!)}`)}
                disabled={saving}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                disabled={saving}
              >
                {saving ? 'Saving...' : (
                  <>
                    <Save className="h-4 w-4 mr-2" />
                    Save Schema
                  </>
                )}
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
      </div>
    </PageLayout>
  );
};

export default EditCollection;
