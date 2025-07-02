/**
 * Utility functions for handling field default values
 * Ensures proper type parsing based on field type
 */

import type { FieldType } from '../types/api';

/**
 * Parse a default value string based on the field type
 * @param defaultValueString - The string representation of the default value
 * @param fieldType - The field type definition
 * @returns The properly typed default value or null if parsing fails
 */
export function parseFieldDefaultValue(
  defaultValueString: string | undefined,
  fieldType: FieldType
): any {
  if (!defaultValueString || !defaultValueString.trim()) {
    return null;
  }

  const fieldTypeString = typeof fieldType === 'string' ? fieldType : Object.keys(fieldType)[0];
  const value = defaultValueString.trim();

  try {
    switch (fieldTypeString) {
      case 'text':
      case 'email':
      case 'url':
      case 'password':
        // For text-based fields, use the value as-is (string)
        return value;

      case 'number':
        // For number fields, parse as number
        const numValue = parseFloat(value);
        return isNaN(numValue) ? null : numValue;

      case 'boolean':
        // For boolean fields, parse as boolean
        const boolValue = value.toLowerCase();
        return boolValue === 'true' || boolValue === '1' || boolValue === 'yes' || boolValue === 'on';

      case 'date':
        // For date fields, validate and use as string (ISO format expected)
        return value;

      case 'json':
        // For JSON fields, parse as JSON
        return JSON.parse(value);

      case 'select':
        // For select fields, use as string or try JSON for arrays
        if (value.startsWith('[') && value.endsWith(']')) {
          return JSON.parse(value);
        }
        return value;

      case 'relationship':
        // For relationship fields, could be ID or array of IDs
        if (value.startsWith('[') && value.endsWith(']')) {
          return JSON.parse(value);
        }
        // Try to parse as number (ID), fallback to string
        const idValue = parseFloat(value);
        return isNaN(idValue) ? value : idValue;

      case 'file':
        // For file fields, typically null or file metadata
        if (value === 'null' || value === '') {
          return null;
        }
        try {
          return JSON.parse(value);
        } catch {
          return null;
        }

      default:
        // For unknown fields, try JSON parse, fallback to string
        try {
          return JSON.parse(value);
        } catch {
          return value;
        }
    }
  } catch (error) {
    console.warn(`Failed to parse default value for field type ${fieldTypeString}:`, error);
    return null;
  }
}

/**
 * Format a default value for display in the UI
 * @param value - The actual default value
 * @param fieldType - The field type definition
 * @returns A string representation suitable for input fields
 */
export function formatFieldDefaultValue(
  value: any,
  fieldType: FieldType
): string {
  if (value === null || value === undefined) {
    return '';
  }

  const fieldTypeString = typeof fieldType === 'string' ? fieldType : Object.keys(fieldType)[0];

  switch (fieldTypeString) {
    case 'text':
    case 'email':
    case 'url':
    case 'password':
    case 'date':
      return String(value);

    case 'number':
      return String(value);

    case 'boolean':
      return String(value);

    case 'json':
    case 'select':
    case 'relationship':
    case 'file':
      return typeof value === 'string' ? value : JSON.stringify(value);

    default:
      return typeof value === 'string' ? value : JSON.stringify(value);
  }
}
