import React, { useState } from 'react';
import { Plus, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Checkbox } from '@/components/ui/checkbox';
import type { SelectConfig as SelectConfigType } from './types';

interface SelectConfigProps {
  index: number;
  config?: SelectConfigType;
  onUpdate: (index: number, config: Partial<SelectConfigType>) => void;
}

const SelectConfig: React.FC<SelectConfigProps> = ({
  index,
  config,
  onUpdate,
}) => {
  const [newOption, setNewOption] = useState('');

  const addOption = () => {
    if (newOption.trim()) {
      const options = config?.options || [];
      onUpdate(index, { options: [...options, newOption.trim()] });
      setNewOption('');
    }
  };

  const removeOption = (optionIndex: number) => {
    const options = config?.options || [];
    onUpdate(index, { options: options.filter((_, i) => i !== optionIndex) });
  };

  const updateOption = (optionIndex: number, value: string) => {
    const options = config?.options || [];
    const updatedOptions = [...options];
    updatedOptions[optionIndex] = value;
    onUpdate(index, { options: updatedOptions });
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      addOption();
    }
  };

  return (
    <div className="space-y-4">
      <div className="space-y-3">
        <div className="space-y-2">
          <Label className="text-sm font-medium">Options*</Label>
          <div className="space-y-2">
            {/* Existing Options */}
            {(config?.options || []).map((option, optionIndex) => (
              <div key={optionIndex} className="flex items-center space-x-2">
                <Input
                  value={option}
                  onChange={(e) => updateOption(optionIndex, e.target.value)}
                  placeholder="Option value"
                  className="flex-1 text-sm"
                />
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => removeOption(optionIndex)}
                  className="flex-shrink-0"
                  aria-label="Remove option"
                >
                  <X className="h-4 w-4" />
                </Button>
              </div>
            ))}
            
            {/* Add New Option */}
            <div className="flex items-center space-x-2">
              <Input
                value={newOption}
                onChange={(e) => setNewOption(e.target.value)}
                onKeyPress={handleKeyPress}
                placeholder="Add new option..."
                className="flex-1 text-sm"
              />
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={addOption}
                disabled={!newOption.trim()}
                className="flex-shrink-0"
              >
                <Plus className="h-4 w-4" />
              </Button>
            </div>

            {/* Quick add for empty state */}
            {(!config?.options || config.options.length === 0) && (
              <div className="text-center py-4 border-2 border-dashed border-muted-foreground/25 rounded-md">
                <p className="text-sm text-muted-foreground mb-2">No options defined yet</p>
                <div className="flex flex-wrap gap-1 justify-center">
                  {['Option 1', 'Option 2', 'Option 3'].map((defaultOption) => (
                    <Button
                      key={defaultOption}
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => {
                        const options = config?.options || [];
                        onUpdate(index, { options: [...options, defaultOption] });
                      }}
                      className="text-xs h-6"
                    >
                      + {defaultOption}
                    </Button>
                  ))}
                </div>
              </div>
            )}
          </div>
          <p className="text-xs text-muted-foreground">
            Add the options users can choose from
          </p>
        </div>

        {/* Bulk text input as alternative */}
        {(config?.options || []).length > 0 && (
          <div className="space-y-2">
            <Label htmlFor={`field-options-bulk-${index}`} className="text-sm font-medium">
              Bulk Edit (one per line)
            </Label>
            <textarea
              id={`field-options-bulk-${index}`}
              value={config?.options?.join('\n') || ''}
              onChange={(e) => {
                const options = e.target.value.split('\n').map(opt => opt.trim()).filter(opt => opt);
                onUpdate(index, { options });
              }}
              className="flex min-h-[80px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
              placeholder="Option 1&#10;Option 2&#10;Option 3"
            />
          </div>
        )}

        <div className="space-y-3">
          <Label className="text-sm font-medium">Behavior Options</Label>
          <div className="space-y-3">
            <div className="flex items-center space-x-2">
              <Checkbox
                id={`field-multiple-select-${index}`}
                checked={config?.multiple || false}
                onCheckedChange={(checked) => onUpdate(index, { multiple: !!checked })}
              />
              <div className="space-y-1">
                <Label htmlFor={`field-multiple-select-${index}`} className="text-sm font-normal">
                  Allow multiple selections
                </Label>
                <p className="text-xs text-muted-foreground">
                  Users can select more than one option
                </p>
              </div>
            </div>
            
            <div className="flex items-center space-x-2">
              <Checkbox
                id={`field-allow-empty-${index}`}
                checked={config?.allow_empty !== false}
                onCheckedChange={(checked) => onUpdate(index, { allow_empty: !!checked })}
              />
              <div className="space-y-1">
                <Label htmlFor={`field-allow-empty-${index}`} className="text-sm font-normal">
                  Allow empty values
                </Label>
                <p className="text-xs text-muted-foreground">
                  Allow users to not select any option
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default SelectConfig; 