//! TypeScript Demo Plugin for OxideDB
//!
//! This plugin demonstrates advanced plugin capabilities including:
//! 1. Data validation and transformation
//! 2. Third-party API integration (simulated)
//! 3. Custom business logic
//! 4. Error handling and logging
//! 5. Metadata processing

// Plugin response structure matching the host contract
interface PluginResponse {
  allow: boolean;
  modified_data?: string;
  error_message?: string;
  metadata: any;
}

// Event payload structure matching the host contract
interface EventPayload {
  event_type: string;
  collection: string;
  data: string;
  metadata: any;
}

// Import host functions that the plugin can call
declare function get_event_payload(): number;
declare function log_info(ptr: number, len: number): void;
declare function log_error(ptr: number, len: number): void;
declare function set_error(ptr: number, len: number): void;
declare function get_result_ptr(): number;
declare function get_result_len(): number;

// Global memory management
let memory: WebAssembly.Memory = new WebAssembly.Memory({ initial: 1 });
let responseBuffer: Uint8Array;

// Helper functions for host communication
function hostLogInfo(message: string): void {
  const encoder = new TextEncoder();
  const bytes = encoder.encode(message);
  const ptr = allocateMemory(bytes.length);
  const view = new Uint8Array(memory.buffer, ptr, bytes.length);
  view.set(bytes);
  log_info(ptr, bytes.length);
}

function hostLogError(message: string): void {
  const encoder = new TextEncoder();
  const bytes = encoder.encode(message);
  const ptr = allocateMemory(bytes.length);
  const view = new Uint8Array(memory.buffer, ptr, bytes.length);
  view.set(bytes);
  log_error(ptr, bytes.length);
}

function hostSetError(message: string): void {
  const encoder = new TextEncoder();
  const bytes = encoder.encode(message);
  const ptr = allocateMemory(bytes.length);
  const view = new Uint8Array(memory.buffer, ptr, bytes.length);
  view.set(bytes);
  set_error(ptr, bytes.length);
}

function hostGetEventPayload(): EventPayload | null {
  try {
    const result = get_event_payload();
    if (result < 0) {
      hostLogError("Failed to get event payload");
      return null;
    }

    const ptr = get_result_ptr();
    const len = get_result_len();

    if (ptr === 0 || len === 0) {
      hostLogError("Invalid payload pointer or length");
      return null;
    }

    const view = new Uint8Array(memory.buffer, ptr, len);
    const decoder = new TextDecoder();
    const jsonStr = decoder.decode(view);

    return JSON.parse(jsonStr) as EventPayload;
  } catch (error) {
    hostLogError(`Failed to parse event payload: ${error}`);
    return null;
  }
}

// Memory allocation function (required for WASM)
export function alloc(_size: number): number {
  // This would typically allocate memory in the WASM linear memory
  // For demo purposes, we'll simulate this
  return 0; // Placeholder
}

// Memory deallocation function
export function dealloc(_ptr: number, _size: number): void {
  // Deallocate memory - implementation depends on allocator
}

// Simulated memory allocation for demo
function allocateMemory(_size: number): number {
  // In a real implementation, this would manage WASM linear memory
  return 0; // Placeholder
}

// Business logic functions
class DataValidator {
  static validateUserData(data: any): { valid: boolean; errors: string[] } {
    const errors: string[] = [];

    // Email validation
    if (data.email && !this.isValidEmail(data.email)) {
      errors.push("Invalid email format");
    }

    // Required fields
    const requiredFields = ['name', 'email'];
    for (const field of requiredFields) {
      if (!data[field] || data[field].trim() === '') {
        errors.push(`Missing required field: ${field}`);
      }
    }

    // Age validation
    if (data.age !== undefined) {
      const age = parseInt(data.age);
      if (isNaN(age) || age < 0 || age > 150) {
        errors.push("Age must be a valid number between 0 and 150");
      }
    }

    return {
      valid: errors.length === 0,
      errors
    };
  }

  static isValidEmail(email: string): boolean {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    return emailRegex.test(email);
  }
}

class DataTransformer {
  static normalizeUserData(data: any): any {
    const normalized = { ...data };

    // Normalize email to lowercase
    if (normalized.email) {
      normalized.email = normalized.email.toLowerCase().trim();
    }

    // Normalize name to title case
    if (normalized.name) {
      normalized.name = this.toTitleCase(normalized.name.trim());
    }

    // Add metadata
    normalized.processed_at = new Date().toISOString();
    normalized.processed_by = 'typescript-demo-plugin';

    return normalized;
  }

  static toTitleCase(str: string): string {
    return str.replace(/\w\S*/g, (txt) => 
      txt.charAt(0).toUpperCase() + txt.substr(1).toLowerCase()
    );
  }
}

// Simulated third-party API integration
class ThirdPartyIntegration {
  static enrichUserData(data: any): any {
    // Simulate API call delay and processing
    hostLogInfo(`Enriching user data for: ${data.email}`);
    
    // Simulate external API response
    const enrichedData = { ...data };
    
    // Add simulated enrichment data
    enrichedData.account_status = 'active';
    enrichedData.risk_score = Math.floor(Math.random() * 100);
    enrichedData.external_id = `ext_${Date.now()}`;
    
    // Simulate some business rules
    if (enrichedData.risk_score > 80) {
      enrichedData.requires_review = true;
      hostLogInfo(`High risk score detected for user: ${data.email}`);
    }
    
    return enrichedData;
  }

  static validateWithExternalService(data: any): boolean {
    // Simulate external validation
    hostLogInfo(`Validating user with external service: ${data.email}`);
    
    // Simulate some validation logic
    const isValid = !data.email.includes('spam') && !data.email.includes('test');
    
    if (!isValid) {
      hostLogError(`External validation failed for: ${data.email}`);
    }
    
    return isValid;
  }
}

// Plugin export functions

/**
 * Called before a record is created
 * Demonstrates comprehensive data validation, transformation, and enrichment
 */
export function on_before_create(): number {
  try {
    hostLogInfo("TypeScript Demo Plugin: Processing before_create event");
    
    const payload = hostGetEventPayload();
    if (!payload) {
      hostSetError("Failed to get event payload");
      return 0;
    }

    hostLogInfo(`Processing ${payload.event_type} for collection: ${payload.collection}`);
    
    // Parse the data
    let data;
    try {
      data = JSON.parse(payload.data);
    } catch (error) {
      hostSetError(`Invalid JSON data: ${error}`);
      return 0;
    }

    // Collection-specific logic
    if (payload.collection === 'users') {
      return handleUserCreation(data, payload);
    } else if (payload.collection === 'orders') {
      return handleOrderCreation(data, payload);
    } else if (payload.collection === 'sensitive_data') {
      return handleSensitiveDataCreation(data, payload);
    }

    // Default handling for other collections
    return handleGenericCreation(data, payload);
    
  } catch (error) {
    hostLogError(`Plugin error in on_before_create: ${error}`);
    hostSetError(`Internal plugin error: ${error}`);
    return 0;
  }
}

function handleUserCreation(data: any, payload: EventPayload): number {
  hostLogInfo(`Processing user creation for ${payload.collection}`);
  
  // Validate user data
  const validation = DataValidator.validateUserData(data);
  if (!validation.valid) {
    const errorMsg = `Validation failed: ${validation.errors.join(', ')}`;
    hostSetError(errorMsg);
    return 0;
  }

  // Transform and normalize data
  const normalizedData = DataTransformer.normalizeUserData(data);
  
  const externalValidation = ThirdPartyIntegration.validateWithExternalService(normalizedData)
    && !normalizedData.email.includes('blocked');
  if (!externalValidation) {
    hostSetError("User blocked by external validation service");
    return 0;
  }

  // Enrich data with external information
  const enrichedData = {
    ...ThirdPartyIntegration.enrichUserData(normalizedData),
    account_status: 'pending_verification',
    created_by_plugin: 'typescript-demo-plugin',
    validation_passed: true
  };

  // Create response
  const response: PluginResponse = {
    allow: true,
    modified_data: JSON.stringify(enrichedData),
    metadata: {
      processed_by: 'typescript-demo-plugin',
      validation_results: validation,
      transformations_applied: ['email_normalization', 'name_title_case', 'data_enrichment']
    }
  };

  // Store response for host to retrieve
  storeResponse(response);
  
  hostLogInfo(`User creation processed successfully for: ${enrichedData.email}`);
  return 1; // Success
}

function handleOrderCreation(data: any, payload: EventPayload): number {
  hostLogInfo(`Processing order creation for ${payload.collection}`);
  
  // Validate required order fields
  const requiredFields = ['customer_id', 'items', 'total'];
  for (const field of requiredFields) {
    if (!data[field]) {
      hostSetError(`Missing required field: ${field}`);
      return 0;
    }
  }

  // Validate order total
  if (typeof data.total !== 'number' || data.total <= 0) {
    hostSetError("Order total must be a positive number");
    return 0;
  }

  // Add order processing metadata
  const processedOrder = {
    ...data,
    order_id: `ord_${Date.now()}`,
    status: 'pending',
    created_at: new Date().toISOString(),
    processed_by: 'typescript-demo-plugin'
  };

  // Simulate fraud detection
  const fraudScore = Math.random() * 100;
  if (fraudScore > 85) {
    hostLogError(`High fraud score detected: ${fraudScore}`);
    hostSetError("Order flagged for manual review due to high fraud score");
    return 0;
  }

  processedOrder.fraud_score = fraudScore;
  processedOrder.fraud_check_passed = true;

  const response: PluginResponse = {
    allow: true,
    modified_data: JSON.stringify(processedOrder),
    metadata: {
      fraud_score: fraudScore,
      risk_level: fraudScore > 50 ? 'medium' : 'low'
    }
  };

  storeResponse(response);
  hostLogInfo(`Order creation processed successfully: ${processedOrder.order_id}`);
  return 1;
}

function handleSensitiveDataCreation(data: any, payload: EventPayload): number {
  hostLogInfo(`Processing sensitive data creation for ${payload.collection} - applying strict validation`);
  
  // For sensitive data, we apply stricter rules
  // This demonstrates how plugins can implement collection-specific security
  
  // Check for PII patterns
  const dataStr = JSON.stringify(data).toLowerCase();
  const piiPatterns = [
    /\b\d{3}-\d{2}-\d{4}\b/, // SSN pattern
    /\b\d{4}[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}\b/, // Credit card pattern
  ];

  for (const pattern of piiPatterns) {
    if (pattern.test(dataStr)) {
      hostLogError("Potential PII detected in sensitive data");
      hostSetError("Sensitive data contains patterns that require additional encryption");
      return 0;
    }
  }

  // Add encryption metadata (in real scenario, data would be encrypted)
  const secureData = {
    ...data,
    encryption_required: true,
    security_level: 'high',
    audit_required: true,
    processed_by: 'typescript-demo-plugin'
  };

  const response: PluginResponse = {
    allow: true,
    modified_data: JSON.stringify(secureData),
    metadata: {
      security_processed: true,
      requires_audit: true,
      encryption_applied: false // Would be true in real implementation
    }
  };

  storeResponse(response);
  hostLogInfo("Sensitive data creation processed with security enhancements");
  return 1;
}

function handleGenericCreation(data: any, payload: EventPayload): number {
  hostLogInfo(`Processing generic creation for collection: ${payload.collection}`);
  
  // Basic validation and processing
  const processedData = {
    ...data,
    created_at: new Date().toISOString(),
    processed_by: 'typescript-demo-plugin'
  };

  const response: PluginResponse = {
    allow: true,
    modified_data: JSON.stringify(processedData),
    metadata: {
      generic_processing: true
    }
  };

  storeResponse(response);
  return 1;
}

/**
 * Called after a record is created
 * Demonstrates post-processing and external integrations
 */
export function on_after_create(): number {
  try {
    hostLogInfo("TypeScript Demo Plugin: Processing after_create event");
    
    const payload = hostGetEventPayload();
    if (!payload) {
      hostLogError("Failed to get event payload in after_create");
      return 0;
    }

    // Parse the created data
    let data;
    try {
      data = JSON.parse(payload.data);
    } catch (error) {
      hostLogError(`Invalid JSON data in after_create: ${error}`);
      return 0;
    }

    // Simulate post-creation tasks
    if (payload.collection === 'users') {
      // Simulate sending welcome email
      hostLogInfo(`Sending welcome email to: ${data.email}`);
      
      // Simulate external system notification
      hostLogInfo(`Notifying CRM system of new user: ${data.email}`);
    } else if (payload.collection === 'orders') {
      // Simulate inventory update
      hostLogInfo(`Updating inventory for order: ${data.order_id}`);
      
      // Simulate payment processing notification
      hostLogInfo(`Notifying payment processor for order: ${data.order_id}`);
    }

    const response: PluginResponse = {
      allow: true,
      metadata: {
        post_processing_completed: true,
        notifications_sent: true
      }
    };

    storeResponse(response);
    return 1;
    
  } catch (error) {
    hostLogError(`Plugin error in on_after_create: ${error}`);
    return 0;
  }
}

/**
 * Plugin initialization function
 */
export function plugin_init(): number {
  hostLogInfo("TypeScript Demo Plugin initialized successfully");
  hostLogInfo("Capabilities: Data validation, transformation, external integration, security processing");
  
  // Initialize plugin state if needed
  const response: PluginResponse = {
    allow: true,
    metadata: {
      plugin_name: 'typescript-demo-plugin',
      version: '1.0.0',
      capabilities: [
        'data_validation',
        'data_transformation', 
        'external_integration',
        'security_processing',
        'fraud_detection'
      ]
    }
  };

  storeResponse(response);
  return 1;
}

/**
 * Plugin cleanup function
 */
export function plugin_cleanup(): number {
  hostLogInfo("TypeScript Demo Plugin cleanup completed");
  
  const response: PluginResponse = {
    allow: true,
    metadata: {
      cleanup_completed: true
    }
  };

  storeResponse(response);
  return 1;
}

// Helper function to store response for host retrieval
function storeResponse(response: PluginResponse): void {
  try {
    const responseJson = JSON.stringify(response);
    const encoder = new TextEncoder();
    responseBuffer = encoder.encode(responseJson);
    
    // In a real implementation, this would store the response in a way
    // that the host can retrieve it
    hostLogInfo(`Response prepared: ${responseBuffer.length} bytes`);
  } catch (error) {
    hostLogError(`Failed to store response: ${error}`);
  }
}

// Export memory for host access
export { memory };
