import { useState, useCallback, useEffect } from 'react';
import type { 
  CollectionViewSettings, 
  FieldCustomization, 
  FieldCustomizationSettings,
  FieldSize 
} from '../types/fieldCustomization';
import type { CollectionSchema } from '../types/api';
import { apiService } from '../services/api';

/**
 * Custom hook for managing field customization settings
 * Handles visibility, ordering, sizing, and persistence
 */
export const useFieldCustomization = (collection: string, schema: CollectionSchema | null) => {
  const [settings, setSettings] = useState<CollectionViewSettings>({
    customizationMode: false,
    fieldCustomizations: {},
    lastUpdated: Date.now()
  });
  
  const [draggedField, setDraggedField] = useState<string | null>(null);

  // Load settings from database on mount and when collection changes
  useEffect(() => {
    if (!collection || !schema) return;

    const loadSettingsFromApi = async () => {
      try {
        const stored = await apiService.getFieldCustomization(collection);
        
        let loadedSettings: CollectionViewSettings;
        
        if (stored) {
          try {
            loadedSettings = stored as CollectionViewSettings;
          } catch {
            loadedSettings = createDefaultSettings(schema);
          }
        } else {
          loadedSettings = createDefaultSettings(schema);
        }

        // Ensure all schema fields have customization settings
        const updatedSettings = ensureAllFieldsHaveSettings(loadedSettings, schema);
        setSettings(updatedSettings);
        
        // Save back if we had to add missing fields
        if (Object.keys(updatedSettings.fieldCustomizations).length !== Object.keys(loadedSettings.fieldCustomizations).length) {
          try {
            await apiService.storeFieldCustomization(collection, updatedSettings);
          } catch (saveError) {
            console.error('Failed to save updated field customization settings:', saveError);
          }
        }
      } catch (error) {
        console.error('Failed to load field customization settings:', error);
        // Fall back to default settings on error
        const defaultSettings = createDefaultSettings(schema);
        setSettings(defaultSettings);
      }
    };

    loadSettingsFromApi();
  }, [collection, schema]);

  const createDefaultSettings = (schema: CollectionSchema): CollectionViewSettings => {
    const fieldCustomizations: FieldCustomizationSettings = {};
    
    // Get all field names and sort them for consistent ordering
    const fieldNames = Object.keys(schema.fields).sort();
    
    fieldNames.forEach((fieldName, index) => {
      fieldCustomizations[fieldName] = {
        visible: true,
        order: index, // Each field gets a unique sequential order
        size: 'full' as FieldSize
      };
    });

    return {
      customizationMode: false,
      fieldCustomizations,
      lastUpdated: Date.now()
    };
  };

  const ensureAllFieldsHaveSettings = (
    settings: CollectionViewSettings, 
    schema: CollectionSchema
  ): CollectionViewSettings => {
    const currentFields = Object.keys(schema.fields).sort(); // Sort for consistent ordering
    const existingCustomizations = { ...settings.fieldCustomizations };
    
    // Get the maximum order value from existing customizations
    const maxOrder = Object.values(existingCustomizations).reduce(
      (max, customization) => Math.max(max, customization.order), 
      -1
    );
    
    // Add missing fields with proper sequential order values
    currentFields.forEach((fieldName, index) => {
      if (!existingCustomizations[fieldName]) {
        existingCustomizations[fieldName] = {
          visible: true,
          order: maxOrder + 1 + index, // Continue from the highest existing order
          size: 'full' as FieldSize
        };
      }
    });

    // Remove customizations for fields that no longer exist
    Object.keys(existingCustomizations).forEach(fieldName => {
      if (!currentFields.includes(fieldName)) {
        delete existingCustomizations[fieldName];
      }
    });

    return {
      ...settings,
      fieldCustomizations: existingCustomizations,
      lastUpdated: Date.now()
    };
  };

  const saveSettings = useCallback(async (newSettings: CollectionViewSettings) => {
    if (!collection) return;
    
    try {
      await apiService.storeFieldCustomization(collection, newSettings);
    } catch (error) {
      console.error('Failed to save field customization settings:', error);
    }
  }, [collection]);

  const updateSettings = useCallback(async (updater: (prev: CollectionViewSettings) => CollectionViewSettings) => {
    const newSettings = updater(settings);
    setSettings(newSettings);
    await saveSettings(newSettings);
  }, [saveSettings, settings]);

  // Toggle customization mode
  const toggleCustomizationMode = useCallback(async () => {
    await updateSettings(prev => {
      const newSettings = {
        ...prev,
        customizationMode: !prev.customizationMode,
        lastUpdated: Date.now()
      };
      
      // If entering customization mode and we have schema, ensure all fields have proper settings
      if (!prev.customizationMode && schema) {
        const currentFields = Object.keys(schema.fields).sort();
        const existingCustomizations = { ...prev.fieldCustomizations };
        
        // Check if any fields are missing customization
        const missingFields = currentFields.filter(fieldName => !existingCustomizations[fieldName]);
        
        if (missingFields.length > 0) {
          // Get the maximum order value from existing customizations
          const maxOrder = Object.values(existingCustomizations).reduce(
            (max, customization) => Math.max(max, customization.order), 
            -1
          );
          
          // Add missing fields with proper order values
          missingFields.forEach((fieldName, index) => {
            existingCustomizations[fieldName] = {
              visible: true,
              order: maxOrder + 1 + index,
              size: 'full' as FieldSize
            };
          });
          
          newSettings.fieldCustomizations = existingCustomizations;
        }
      }
      
      return newSettings;
    });
  }, [updateSettings, schema]);

  // Update field customization
  const updateFieldCustomization = useCallback(async (
    fieldName: string, 
    customization: Partial<FieldCustomization>
  ) => {
    await updateSettings(prev => ({
      ...prev,
      fieldCustomizations: {
        ...prev.fieldCustomizations,
        [fieldName]: {
          ...prev.fieldCustomizations[fieldName],
          ...customization
        }
      },
      lastUpdated: Date.now()
    }));
  }, [updateSettings]);

  // Toggle field visibility
  const toggleFieldVisibility = useCallback((fieldName: string) => {
    const currentVisibility = settings.fieldCustomizations[fieldName]?.visible ?? true;
    updateFieldCustomization(fieldName, { visible: !currentVisibility });
  }, [settings.fieldCustomizations, updateFieldCustomization]);

  // Change field size
  const setFieldSize = useCallback((fieldName: string, size: FieldSize) => {
    updateFieldCustomization(fieldName, { size });
  }, [updateFieldCustomization]);

  // Get ordered and filtered fields
  const getOrderedVisibleFields = useCallback(() => {
    if (!schema) return [];
    
    return Object.entries(schema.fields)
      .map(([fieldName, fieldDef]) => ({
        fieldName,
        fieldDef,
        customization: settings.fieldCustomizations[fieldName] || {
          visible: true,
          order: 0,
          size: 'full' as FieldSize
        }
      }))
      .filter(field => field.customization.visible)
      .sort((a, b) => a.customization.order - b.customization.order);
  }, [schema, settings.fieldCustomizations]);

  // Get all fields (including hidden ones) for customization UI
  const getAllFieldsWithCustomization = useCallback(() => {
    if (!schema) return [];
    
    return Object.entries(schema.fields)
      .map(([fieldName, fieldDef]) => {
        const customization = settings.fieldCustomizations[fieldName];
        return {
          fieldName,
          fieldDef,
          customization: customization || {
            visible: true,
            order: 0,
            size: 'full' as FieldSize
          }
        };
      })
      .sort((a, b) => a.customization.order - b.customization.order);
  }, [schema, settings.fieldCustomizations]);

  // Drag and drop functionality
  const handleDragStart = useCallback((fieldName: string) => {
    setDraggedField(fieldName);
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
  }, []);

  const handleDrop = useCallback(async (e: React.DragEvent, targetFieldName: string) => {
    e.preventDefault();
    
    if (!draggedField || draggedField === targetFieldName) {
      setDraggedField(null);
      return;
    }

    const allFields = getAllFieldsWithCustomization();
    const draggedFieldData = allFields.find(f => f.fieldName === draggedField);
    const targetFieldData = allFields.find(f => f.fieldName === targetFieldName);
    
    if (!draggedFieldData || !targetFieldData) {
      setDraggedField(null);
      return;
    }

    // Swap the order values
    const draggedOrder = draggedFieldData.customization.order;
    const targetOrder = targetFieldData.customization.order;
    
    try {
      // Update both fields atomically
      await updateSettings(prev => {
        const newCustomizations = { ...prev.fieldCustomizations };
        
        // Update dragged field
        newCustomizations[draggedField] = {
          ...newCustomizations[draggedField],
          order: targetOrder
        };
        
        // Update target field
        newCustomizations[targetFieldName] = {
          ...newCustomizations[targetFieldName],
          order: draggedOrder
        };
        
        return {
          ...prev,
          fieldCustomizations: newCustomizations,
          lastUpdated: Date.now()
        };
      });
    } catch (error) {
      console.error('Failed to update field orders:', error);
    }
    
    setDraggedField(null);
  }, [draggedField, getAllFieldsWithCustomization, updateSettings]);

  // Reset all customizations to default
  const resetToDefault = useCallback(async () => {
    if (!schema) return;
    
    const defaultSettings = createDefaultSettings(schema);
    setSettings(defaultSettings);
    await saveSettings(defaultSettings);
  }, [schema, saveSettings]);

  // Bulk operations
  const showAllFields = useCallback(async () => {
    if (!schema) return;
    
    const updates: FieldCustomizationSettings = {};
    Object.keys(schema.fields).forEach(fieldName => {
      updates[fieldName] = {
        ...settings.fieldCustomizations[fieldName],
        visible: true
      };
    });
    
    await updateSettings(prev => ({
      ...prev,
      fieldCustomizations: { ...prev.fieldCustomizations, ...updates },
      lastUpdated: Date.now()
    }));
  }, [schema, settings.fieldCustomizations, updateSettings]);

  const hideAllFields = useCallback(async () => {
    if (!schema) return;
    
    const updates: FieldCustomizationSettings = {};
    Object.keys(schema.fields).forEach(fieldName => {
      updates[fieldName] = {
        ...settings.fieldCustomizations[fieldName],
        visible: false
      };
    });
    
    await updateSettings(prev => ({
      ...prev,
      fieldCustomizations: { ...prev.fieldCustomizations, ...updates },
      lastUpdated: Date.now()
    }));
  }, [schema, settings.fieldCustomizations, updateSettings]);

  // Fix field customizations with duplicate order values
  const fixDuplicateOrders = useCallback(async () => {
    if (!schema) return;
    
    const currentFields = Object.keys(schema.fields).sort();
    const existingCustomizations = { ...settings.fieldCustomizations };
    
    // Check if there are duplicate order values
    const orders = Object.values(existingCustomizations).map(c => c.order);
    const uniqueOrders = new Set(orders);
    
    if (orders.length !== uniqueOrders.size) {
      // Reassign all fields with sequential order values
      currentFields.forEach((fieldName, index) => {
        if (existingCustomizations[fieldName]) {
          existingCustomizations[fieldName] = {
            ...existingCustomizations[fieldName],
            order: index
          };
        }
      });
      
      await updateSettings(prev => ({
        ...prev,
        fieldCustomizations: existingCustomizations,
        lastUpdated: Date.now()
      }));
    }
  }, [schema, settings.fieldCustomizations, updateSettings]);

  return {
    settings,
    customizationMode: settings.customizationMode,
    toggleCustomizationMode,
    updateFieldCustomization,
    toggleFieldVisibility,
    setFieldSize,
    getOrderedVisibleFields,
    getAllFieldsWithCustomization,
    handleDragStart,
    handleDragOver,
    handleDrop,
    draggedField,
    resetToDefault,
    showAllFields,
    hideAllFields,
    fixDuplicateOrders
  };
}; 