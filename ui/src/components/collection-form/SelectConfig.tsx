import React from 'react';
import { Label } from '@/components/ui/label';
import type { SelectConfig as SelectConfigType } from './types';

interface SelectConfigProps {
  index: number;
  config: SelectConfigType | undefined;
  onUpdate: (index: number, config: Partial<SelectConfigType>) => void;
}

const SelectConfig: React.FC<SelectConfigProps> = ({
  index,
  config,
  onUpdate,
}) => {
  return (
    <div className="col-span-full space-y-4 p-4 border rounded-md bg-muted/50">
      <h4 className="font-medium text-sm">Select Configuration</h4>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="space-y-2">
          <Label htmlFor={`field-options-${index}`}>Options (one per line)</Label>
          <textarea
            id={`field-options-${index}`}
            value={config?.options?.join('\n') || ''}
            onChange={(e) => {
              const options = e.target.value.split('\n').map(opt => opt.trim()).filter(opt => opt);
              onUpdate(index, { options });
            }}
            className="flex min-h-[100px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
            placeholder="Option 1&#10;Option 2&#10;Option 3"
          />
        </div>

        <div className="space-y-2">
          <Label>Select Options</Label>
          <div className="space-y-2">
            <div className="flex items-center space-x-2">
              <input
                type="checkbox"
                id={`field-multiple-select-${index}`}
                checked={config?.multiple || false}
                onChange={(e) => onUpdate(index, { multiple: e.target.checked })}
                className="h-4 w-4 rounded border border-input bg-background text-primary focus:ring-2 focus:ring-ring focus:ring-offset-2"
              />
              <Label htmlFor={`field-multiple-select-${index}`}>Multiple Selection</Label>
            </div>
            <div className="flex items-center space-x-2">
              <input
                type="checkbox"
                id={`field-allow-empty-${index}`}
                checked={config?.allow_empty !== false}
                onChange={(e) => onUpdate(index, { allow_empty: e.target.checked })}
                className="h-4 w-4 rounded border border-input bg-background text-primary focus:ring-2 focus:ring-ring focus:ring-offset-2"
              />
              <Label htmlFor={`field-allow-empty-${index}`}>Allow Empty Values</Label>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default SelectConfig; 