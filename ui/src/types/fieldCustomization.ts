/**
 * Field customization types for the EditRecord form
 * Allows users to control field visibility, ordering, and sizing
 */

import type { FieldDefinition } from './api';

export type FieldSize = 'full' | 'half' | 'third' | 'quarter';

export interface FieldCustomization {
  /** Whether the field is visible in the form */
  visible: boolean;
  /** Custom order index for the field (lower values appear first) */
  order: number;
  /** Width/size of the field in the form layout */
  size: FieldSize;
}

export interface FieldCustomizationSettings {
  /** Mapping of field names to their customization settings */
  [fieldName: string]: FieldCustomization;
}

export interface CollectionViewSettings {
  /** Whether customization mode is enabled */
  customizationMode: boolean;
  /** Field customization settings for this collection */
  fieldCustomizations: FieldCustomizationSettings;
  /** Timestamp when settings were last updated */
  lastUpdated: number;
}

export interface FieldRenderProps {
  /** Field name */
  fieldName: string;
  /** Field definition from schema */
  fieldDefinition: FieldDefinition;
  /** Customization settings for this field */
  customization: FieldCustomization;
  /** Whether we're in customization mode */
  isCustomizationMode: boolean;
  /** Callback to update field customization */
  onCustomizationChange: (fieldName: string, customization: Partial<FieldCustomization>) => void;
  /** Callback for drag and drop reordering */
  onDragStart?: (fieldName: string) => void;
  onDragOver?: (e: React.DragEvent) => void;
  onDrop?: (e: React.DragEvent, targetFieldName: string) => void;
} 