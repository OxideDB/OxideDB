import React from 'react';
import { Label } from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import type { FileConfig as FileConfigType } from './types';

interface FileConfigProps {
  index: number;
  config: FileConfigType | undefined;
  onUpdate: (index: number, config: Partial<FileConfigType>) => void;
}

const FileConfig: React.FC<FileConfigProps> = ({
  index,
  config,
  onUpdate,
}) => {
  return (
    <div className="col-span-full space-y-4 p-4 border rounded-md bg-muted/50">
      <h4 className="font-medium text-sm">File Configuration</h4>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        <div className="space-y-2">
          <Label htmlFor={`field-max-size-${index}`}>Max File Size (MB)</Label>
          <Input
            id={`field-max-size-${index}`}
            type="number"
            value={config?.max_file_size ? config.max_file_size / (1024 * 1024) : 10}
            onChange={(e) => {
              const sizeMB = parseFloat(e.target.value) || 10;
              onUpdate(index, {
                max_file_size: sizeMB * 1024 * 1024
              });
            }}
            placeholder="10"
          />
        </div>
        
        <div className="space-y-2">
          <Label htmlFor={`field-mime-types-${index}`}>Allowed MIME Types (optional)</Label>
          <Input
            id={`field-mime-types-${index}`}
            value={config?.allowed_mime_types?.join(', ') || ''}
            onChange={(e) => {
              const types = e.target.value ? e.target.value.split(',').map(t => t.trim()).filter(t => t) : null;
              onUpdate(index, {
                allowed_mime_types: types
              });
            }}
            placeholder="image/*, application/pdf"
          />
        </div>

        <div className="space-y-2">
          <Label>File Options</Label>
          <div className="space-y-2">
            <div className="flex items-center space-x-2">
              <input
                type="checkbox"
                id={`field-multiple-files-${index}`}
                checked={config?.multiple || false}
                onChange={(e) => onUpdate(index, {
                  multiple: e.target.checked
                })}
                className="h-4 w-4 rounded border border-input bg-background text-primary focus:ring-2 focus:ring-ring focus:ring-offset-2"
              />
              <Label htmlFor={`field-multiple-files-${index}`}>Multiple Files</Label>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default FileConfig; 