import React from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import type { ValidationConfig } from './types';

interface ValidationConfigProps {
  index: number;
  fieldType: string;
  config?: ValidationConfig;
  onUpdate: (index: number, config: Partial<ValidationConfig>) => void;
}

const ValidationConfigComponent: React.FC<ValidationConfigProps> = ({
  index,
  fieldType,
  config = {},
  onUpdate,
}) => {
  const handleUpdate = (field: keyof ValidationConfig, value: any) => {
    onUpdate(index, { ...config, [field]: value });
  };

  const showRegex = ['text', 'email', 'url', 'password'].includes(fieldType);
  const showMinMax = ['text', 'number'].includes(fieldType);
  const showAllowEmpty = true; // Can be used for any field type

  return (
    <div className="col-span-full space-y-4 p-4 border rounded-md bg-muted/50">
      <h4 className="font-medium text-sm">Validation Rules</h4>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        
        {/* Regex Pattern */}
        {showRegex && (
          <div className="space-y-2">
            <Label htmlFor={`field-regex-${index}`}>
              Regex Pattern
              <span className="text-xs text-muted-foreground ml-1">(optional)</span>
            </Label>
            <Input
              id={`field-regex-${index}`}
              value={config.regex || ''}
              onChange={(e) => handleUpdate('regex', e.target.value || undefined)}
              placeholder={
                fieldType === 'email' ? '^[^@]+@[^@]+\\.[^@]+$' :
                fieldType === 'url' ? '^https?://.+' :
                fieldType === 'password' ? '^(?=.*[A-Za-z])(?=.*\\d).{8,}$' :
                '^\\w+$'
              }
              className="font-mono text-sm"
            />
            <p className="text-xs text-muted-foreground">
              {fieldType === 'email' && 'Pattern for email validation'}
              {fieldType === 'url' && 'Pattern for URL validation'}
              {fieldType === 'password' && 'Pattern for password strength'}
              {fieldType === 'text' && 'Custom pattern validation'}
            </p>
          </div>
        )}

        {/* Min/Max Values */}
        {showMinMax && (
          <>
            <div className="space-y-2">
              <Label htmlFor={`field-min-${index}`}>
                {fieldType === 'text' ? 'Min Length' : 'Min Value'}
                <span className="text-xs text-muted-foreground ml-1">(optional)</span>
              </Label>
              <Input
                id={`field-min-${index}`}
                type="number"
                step={fieldType === 'number' ? 'any' : '1'}
                value={config.min || ''}
                onChange={(e) => handleUpdate('min', e.target.value ? Number(e.target.value) : undefined)}
                placeholder={fieldType === 'text' ? '3' : '0'}
              />
              <p className="text-xs text-muted-foreground">
                {fieldType === 'text' ? 'Minimum character length' : 'Minimum numeric value'}
              </p>
            </div>

            <div className="space-y-2">
              <Label htmlFor={`field-max-${index}`}>
                {fieldType === 'text' ? 'Max Length' : 'Max Value'}
                <span className="text-xs text-muted-foreground ml-1">(optional)</span>
              </Label>
              <Input
                id={`field-max-${index}`}
                type="number"
                step={fieldType === 'number' ? 'any' : '1'}
                value={config.max || ''}
                onChange={(e) => handleUpdate('max', e.target.value ? Number(e.target.value) : undefined)}
                placeholder={fieldType === 'text' ? '255' : '100'}
              />
              <p className="text-xs text-muted-foreground">
                {fieldType === 'text' ? 'Maximum character length' : 'Maximum numeric value'}
              </p>
            </div>
          </>
        )}

        {/* Allow Empty */}
        {showAllowEmpty && (
          <div className="space-y-2">
            <Label>Empty Value Handling</Label>
            <div className="space-y-2">
              <div className="flex items-center space-x-2">
                <input
                  type="checkbox"
                  id={`field-allow-empty-${index}`}
                  checked={config.allow_empty !== false} // Default to true if undefined
                  onChange={(e) => handleUpdate('allow_empty', e.target.checked)}
                  className="h-4 w-4 rounded border border-input bg-background text-primary focus:ring-2 focus:ring-ring focus:ring-offset-2"
                />
                <Label htmlFor={`field-allow-empty-${index}`} className="text-sm">
                  Allow Empty Values
                </Label>
              </div>
              <p className="text-xs text-muted-foreground">
                When unchecked, empty values will be rejected even if field is not required
              </p>
            </div>
          </div>
        )}

        {/* Custom Error Message */}
        <div className="space-y-2 md:col-span-2 lg:col-span-3">
          <Label htmlFor={`field-message-${index}`}>
            Custom Error Message
            <span className="text-xs text-muted-foreground ml-1">(optional)</span>
          </Label>
          <Textarea
            id={`field-message-${index}`}
            value={config.message || ''}
            onChange={(e) => handleUpdate('message', e.target.value || undefined)}
            placeholder="Enter a custom error message for validation failures. Use {field} to reference the field name."
            className="h-20 resize-none"
          />
          <p className="text-xs text-muted-foreground">
            Custom message shown when validation fails. Use <code className="text-xs bg-muted px-1 rounded">{'{field}'}</code> to insert the field name.
          </p>
        </div>
      </div>

      {/* Validation Preview */}
      {(config.regex || config.min !== undefined || config.max !== undefined || config.message) && (
        <div className="mt-4 p-3 bg-background border rounded-md">
          <p className="text-sm font-medium mb-2">Validation Summary:</p>
          <ul className="text-xs text-muted-foreground space-y-1">
            {config.regex && (
              <li>• Pattern: <code className="bg-muted px-1 rounded">{config.regex}</code></li>
            )}
            {config.min !== undefined && (
              <li>• Minimum {fieldType === 'text' ? 'length' : 'value'}: {config.min}</li>
            )}
            {config.max !== undefined && (
              <li>• Maximum {fieldType === 'text' ? 'length' : 'value'}: {config.max}</li>
            )}
            {config.allow_empty === false && (
              <li>• Empty values not allowed</li>
            )}
            {config.message && (
              <li>• Custom message: "{config.message}"</li>
            )}
          </ul>
        </div>
      )}
    </div>
  );
};

export default ValidationConfigComponent; 