import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { ArrowLeft, ChevronDown, ChevronRight, Database, Shield, Settings } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Checkbox } from '@/components/ui/checkbox';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible';
import { Separator } from '@/components/ui/separator';
import { Badge } from '@/components/ui/badge';
import { Alert, AlertDescription } from '@/components/ui/alert';
import PageLayout from '@/components/PageLayout';
import { apiService } from '../services/api';
import type { CollectionSchema, FieldDefinition, CollectionType, CreateCollectionRequest } from '../types/api';
import { useFieldManagement, FieldsList } from '@/components/collection-form';
import { parseFieldDefaultValue } from '../utils/fieldDefaults';
import type { FieldFormData } from '@/components/collection-form/types';

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

const CreateCollection: React.FC = () => {
  const navigate = useNavigate();
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [collections, setCollections] = useState<CollectionSchema[]>([]);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [currentStep, setCurrentStep] = useState<'basic' | 'fields' | 'review'>('basic');

  // Schema form state
  const [collectionName, setCollectionName] = useState('');
  const [collectionType, setCollectionType] = useState<CollectionType>('base');
  
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
  
  // Use the field management hook
  const fieldManagement = useFieldManagement();

  // Keep track of previous collection type to detect transitions
  const prevCollectionTypeRef = React.useRef<CollectionType>(collectionType);

  // Load collections for relationship field options
  React.useEffect(() => {
    const loadCollections = async () => {
      try {
        const collections = await apiService.getCollections();
        setCollections(collections);
      } catch (err) {
        console.error('Failed to load collections:', err);
      }
    };
    loadCollections();
  }, []);

  // Set up default auth fields when switching into auth collection type
  React.useEffect(() => {
    if (collectionType === 'auth' && prevCollectionTypeRef.current !== 'auth') {
      // Define exactly four default fields for auth collections
      const defaultAuthFields: FieldFormData[] = [
        {
          name: authConfig.identifierField,
          field_type: 'email',
          required: true,
          unique: true,
          default: '',
          validation: {
            regex: '^[\\w\\.-]+@[\\w\\.-]+\\.[a-zA-Z]{2,}$',
            message: 'Please enter a valid email address',
            allow_empty: false,
            min: null,
            max: null,
          },
        },
        {
          name: authConfig.credentialField,
          field_type: 'password',
          required: true,
          unique: false,
          default: '',
          validation: {
            min: 8,
            message: 'Password must be at least 8 characters long',
            allow_empty: false,
            regex: null,
            max: null,
          },
        },
        {
          name: 'role',
          field_type: {
            select: {
              options: ['user', 'admin'],
              multiple: false,
              allow_empty: false,
            },
          },
          required: true,
          unique: false,
          default: 'user',
          validation: null,
          selectConfig: {
            options: ['user', 'admin'],
            multiple: false,
            allow_empty: false,
          },
        },
        {
          name: 'email_verified',
          field_type: 'boolean',
          required: true,
          unique: false,
          default: 'false',
          validation: null,
        },
      ];

      fieldManagement.setFieldsFromSchema(defaultAuthFields);
    } else if (collectionType === 'base' && prevCollectionTypeRef.current === 'auth') {
      // Switching from auth back to base: clear fields
      fieldManagement.setFieldsFromSchema([]);
    }

    // Update previous collection type reference
    prevCollectionTypeRef.current = collectionType;
  }, [collectionType, authConfig.identifierField, authConfig.credentialField, fieldManagement]);

  // Auto-enable refresh tokens for superuser collections
  React.useEffect(() => {
    if (collectionType === 'auth' && collectionName.toLowerCase().includes('superuser')) {
      setAuthConfig(prev => ({
        ...prev,
        refreshTokensEnabled: true,
        refreshTokensRequired: true
      }));
    }
  }, [collectionType, collectionName]);

  const handleCreateCollection = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!collectionName.trim()) return;

    // Validate collection name
    if (collectionName.trim().startsWith('_')) {
      setError('Collection names cannot start with underscore (reserved for system collections)');
      return;
    }

    try {
      setCreating(true);
      
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

      const schema: CreateCollectionRequest = {
        name: collectionName.trim(),
        collection_type: collectionType,
        fields: fieldsMap,
        indexes: [], // Start with no custom indexes
        // Include auth config for auth collections
        ...(collectionType === 'auth' ? { auth_config: authConfig } : {})
      };

      await apiService.createCollection(schema);
      navigate('/collections');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create collection');
    } finally {
      setCreating(false);
    }
  };

  const getCollectionTypeIcon = (type: CollectionType) => {
    return type === 'auth' ? <Shield className="h-4 w-4" /> : <Database className="h-4 w-4" />;
  };

  const getCollectionTypeDescription = (type: CollectionType) => {
    return type === 'auth' 
      ? 'Stores user authentication data with built-in security features'
      : 'Stores general application data like posts, products, or any custom entities';
  };

  const canProceedToFields = () => {
    return collectionName.trim().length > 0;
  };

  const canProceedToReview = () => {
    return fieldManagement.fields.length > 0 && fieldManagement.fields.every(f => f.name.trim());
  };

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
      title="Create New Collection" 
      description="Set up a new data collection with custom schema"
      leftActions={leftActions}
    >
      <div className="max-w-5xl mx-auto space-y-6">
        
        {/* Progress Steps */}
        <Card className="border-primary/20 bg-primary/5">
          <CardContent className="p-4">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-4">
                <div className={`flex items-center space-x-2 ${currentStep === 'basic' ? 'text-primary font-medium' : 'text-muted-foreground'}`}>
                  <div className={`w-8 h-8 rounded-full flex items-center justify-center text-sm ${
                    currentStep === 'basic' ? 'bg-primary text-primary-foreground' : 'bg-muted text-muted-foreground'
                  }`}>1</div>
                  <span className="hidden sm:inline">Basic Info</span>
                </div>
                <ChevronRight className="h-4 w-4 text-muted-foreground" />
                <div className={`flex items-center space-x-2 ${currentStep === 'fields' ? 'text-primary font-medium' : 'text-muted-foreground'}`}>
                  <div className={`w-8 h-8 rounded-full flex items-center justify-center text-sm ${
                    currentStep === 'fields' ? 'bg-primary text-primary-foreground' : 'bg-muted text-muted-foreground'
                  }`}>2</div>
                  <span className="hidden sm:inline">Schema Fields</span>
                </div>
                <ChevronRight className="h-4 w-4 text-muted-foreground" />
                <div className={`flex items-center space-x-2 ${currentStep === 'review' ? 'text-primary font-medium' : 'text-muted-foreground'}`}>
                  <div className={`w-8 h-8 rounded-full flex items-center justify-center text-sm ${
                    currentStep === 'review' ? 'bg-primary text-primary-foreground' : 'bg-muted text-muted-foreground'
                  }`}>3</div>
                  <span className="hidden sm:inline">Review & Create</span>
                </div>
              </div>
            </div>
          </CardContent>
        </Card>

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

        <form onSubmit={handleCreateCollection} className="space-y-6">
          
          {/* Step 1: Basic Information */}
          {currentStep === 'basic' && (
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center space-x-2">
                  <Database className="h-5 w-5" />
                  <span>Basic Information</span>
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-6">
                <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                  <div className="space-y-4">
                    <div className="space-y-2">
                      <Label htmlFor="collectionName" className="text-base font-medium">
                        Collection Name*
                      </Label>
                      <Input
                        id="collectionName"
                        value={collectionName}
                        onChange={(e) => setCollectionName(e.target.value)}
                        placeholder="e.g., users, posts, products"
                        autoFocus
                        required
                        className="text-base"
                        aria-describedby="collection-name-help"
                      />
                      <p id="collection-name-help" className="text-sm text-muted-foreground">
                        Choose a descriptive name for your collection. Use lowercase letters, numbers, and underscores.
                      </p>
                    </div>
                  </div>

                  <div className="space-y-4">
                    <div className="space-y-3">
                      <Label className="text-base font-medium">Collection Type*</Label>
                      <div className="space-y-3">
                        <div 
                          className={`p-4 border-2 rounded-lg cursor-pointer transition-colors ${
                            collectionType === 'base' 
                              ? 'border-primary bg-primary/5' 
                              : 'border-border hover:border-primary/50'
                          }`}
                          onClick={() => setCollectionType('base')}
                          role="button"
                          tabIndex={0}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter' || e.key === ' ') {
                              e.preventDefault();
                              setCollectionType('base');
                            }
                          }}
                          aria-label="Select base collection type"
                        >
                          <div className="flex items-start space-x-3">
                            <input
                              type="radio"
                              checked={collectionType === 'base'}
                              onChange={() => setCollectionType('base')}
                              className="mt-0.5"
                              aria-hidden="true"
                              tabIndex={-1}
                            />
                            <div className="flex-1">
                              <div className="flex items-center space-x-2 mb-1">
                                <Database className="h-4 w-4" />
                                <span className="font-medium">Base Collection</span>
                              </div>
                              <p className="text-sm text-muted-foreground">
                                {getCollectionTypeDescription('base')}
                              </p>
                            </div>
                          </div>
                        </div>

                        <div 
                          className={`p-4 border-2 rounded-lg cursor-pointer transition-colors ${
                            collectionType === 'auth' 
                              ? 'border-primary bg-primary/5' 
                              : 'border-border hover:border-primary/50'
                          }`}
                          onClick={() => setCollectionType('auth')}
                          role="button"
                          tabIndex={0}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter' || e.key === ' ') {
                              e.preventDefault();
                              setCollectionType('auth');
                            }
                          }}
                          aria-label="Select auth collection type"
                        >
                          <div className="flex items-start space-x-3">
                            <input
                              type="radio"
                              checked={collectionType === 'auth'}
                              onChange={() => setCollectionType('auth')}
                              className="mt-0.5"
                              aria-hidden="true"
                              tabIndex={-1}
                            />
                            <div className="flex-1">
                              <div className="flex items-center space-x-2 mb-1">
                                <Shield className="h-4 w-4" />
                                <span className="font-medium">Auth Collection</span>
                                <Badge variant="secondary" className="text-xs">Security</Badge>
                              </div>
                              <p className="text-sm text-muted-foreground">
                                {getCollectionTypeDescription('auth')}
                              </p>
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>

                {collectionType === 'auth' && (
                  <div className="mt-6">
                    <Alert>
                      <Shield className="h-4 w-4" />
                      <AlertDescription>
                        Auth collections come with default security fields (email, password, role, etc.). 
                        You can customize these in the next step.
                      </AlertDescription>
                    </Alert>
                  </div>
                )}

                <div className="flex justify-end">
                  <Button
                    type="button"
                    onClick={() => {
                      if (canProceedToFields()) {
                        setCurrentStep('fields');
                      }
                    }}
                    disabled={!canProceedToFields()}
                    className="min-w-32"
                  >
                    Next: Schema Fields
                  </Button>
                </div>
              </CardContent>
            </Card>
          )}

          {/* Step 2: Schema Fields */}
          {currentStep === 'fields' && (
            <div className="space-y-6">
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center justify-between">
                    <div className="flex items-center space-x-2">
                      <Settings className="h-5 w-5" />
                      <span>Schema Fields</span>
                    </div>
                    <Badge variant="outline">
                      {fieldManagement.fields.length} field{fieldManagement.fields.length !== 1 ? 's' : ''}
                    </Badge>
                  </CardTitle>
                </CardHeader>
                <CardContent>
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
                </CardContent>
              </Card>

              {/* Advanced Auth Configuration */}
              {collectionType === 'auth' && (
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

              <div className="flex justify-between">
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => setCurrentStep('basic')}
                >
                  ← Previous
                </Button>
                <Button
                  type="button"
                  onClick={() => {
                    if (canProceedToReview()) {
                      setCurrentStep('review');
                    }
                  }}
                  disabled={!canProceedToReview()}
                  className="min-w-32"
                >
                  Next: Review
                </Button>
              </div>
            </div>
          )}

          {/* Step 3: Review & Create */}
          {currentStep === 'review' && (
            <div className="space-y-6">
              <Card>
                <CardHeader>
                  <CardTitle>Review & Create Collection</CardTitle>
                </CardHeader>
                <CardContent className="space-y-6">
                  
                  {/* Collection Summary */}
                  <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                    <div className="space-y-4">
                      <div>
                        <Label className="text-sm font-medium text-muted-foreground">Collection Name</Label>
                        <div className="text-lg font-medium">{collectionName}</div>
                      </div>
                      <div>
                        <Label className="text-sm font-medium text-muted-foreground">Type</Label>
                        <div className="flex items-center space-x-2">
                          {getCollectionTypeIcon(collectionType)}
                          <span className="font-medium capitalize">{collectionType}</span>
                          {collectionType === 'auth' && (
                            <Badge variant="secondary" className="text-xs">Security</Badge>
                          )}
                        </div>
                      </div>
                    </div>
                    
                    <div className="space-y-4">
                      <div>
                        <Label className="text-sm font-medium text-muted-foreground">Total Fields</Label>
                        <div className="text-lg font-medium">{fieldManagement.fields.length}</div>
                      </div>
                      <div>
                        <Label className="text-sm font-medium text-muted-foreground">Required Fields</Label>
                        <div className="text-lg font-medium">
                          {fieldManagement.fields.filter(f => f.required).length}
                        </div>
                      </div>
                    </div>
                  </div>

                  <Separator />

                  {/* Fields Summary */}
                  <div>
                    <Label className="text-base font-medium mb-3 block">Fields Overview</Label>
                    <div className="space-y-2 max-h-64 overflow-y-auto">
                      {fieldManagement.fields.map((field, index) => (
                        <div key={index} className="flex items-center justify-between p-3 bg-muted/50 rounded-md">
                          <div className="flex items-center space-x-3">
                            <span className="font-medium">{field.name}</span>
                            <Badge variant="outline" className="text-xs">
                              {typeof field.field_type === 'string' ? field.field_type : Object.keys(field.field_type)[0]}
                            </Badge>
                            {field.required && (
                              <Badge variant="destructive" className="text-xs">Required</Badge>
                            )}
                            {field.unique && (
                              <Badge variant="secondary" className="text-xs">Unique</Badge>
                            )}
                          </div>
                          {field.default && (
                            <span className="text-xs text-muted-foreground">
                              Default: {field.default}
                            </span>
                          )}
                        </div>
                      ))}
                    </div>
                  </div>

                  <div className="flex justify-between">
                    <Button
                      type="button"
                      variant="outline"
                      onClick={() => setCurrentStep('fields')}
                    >
                      ← Previous
                    </Button>
                    <div className="flex space-x-2">
                      <Button
                        type="button"
                        variant="outline"
                        onClick={() => navigate('/collections')}
                        disabled={creating}
                      >
                        Cancel
                      </Button>
                      <Button
                        type="submit"
                        disabled={creating}
                        className="min-w-32"
                      >
                        {creating ? 'Creating...' : 'Create Collection'}
                      </Button>
                    </div>
                  </div>
                </CardContent>
              </Card>
            </div>
          )}

        </form>
      </div>
    </PageLayout>
  );
};

export default CreateCollection; 