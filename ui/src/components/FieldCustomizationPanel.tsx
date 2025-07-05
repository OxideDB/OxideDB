import React from 'react';
import { Eye, EyeOff, RotateCcw, GripVertical } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import type { CollectionSchema } from '../types/api';
import type { FieldSize } from '../types/fieldCustomization';

interface FieldCustomizationPanelProps {
  fieldCustomization: any; // Using any for now since the hook type is complex
  schema: CollectionSchema;
}

/**
 * Component for field customization controls
 * Provides drag-and-drop reordering, visibility toggles, and size controls
 */
export const FieldCustomizationPanel: React.FC<FieldCustomizationPanelProps> = ({
  fieldCustomization,
  schema,
}) => {
  return (
    <div className="mb-6 p-4 border rounded-lg bg-muted/50">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4 mb-4">
        <div>
          <h3 className="text-lg font-medium">Field Customization</h3>
          <p className="text-sm text-muted-foreground">
            Drag fields to reorder, toggle visibility, and adjust display settings
          </p>
        </div>
        <div className="flex flex-col sm:flex-row gap-2">
          <Button 
            variant="outline" 
            size="sm" 
            onClick={fieldCustomization.showAllFields}
            className="w-full sm:w-auto"
          >
            <Eye className="h-4 w-4 mr-2" />
            Show All
          </Button>
          <Button 
            variant="outline" 
            size="sm" 
            onClick={fieldCustomization.hideAllFields}
            className="w-full sm:w-auto"
          >
            <EyeOff className="h-4 w-4 mr-2" />
            Hide All
          </Button>
          <Button 
            variant="outline" 
            size="sm" 
            onClick={fieldCustomization.resetToDefault}
            className="w-full sm:w-auto"
          >
            <RotateCcw className="h-4 w-4 mr-2" />
            Reset
          </Button>
        </div>
      </div>
      
      <div className="grid gap-2">
        {fieldCustomization.getAllFieldsWithCustomization().map((field: any) => (
          <div
            key={field.fieldName}
            className="flex items-center gap-2 p-2 bg-background rounded border"
            draggable
            onDragStart={() => fieldCustomization.handleDragStart(field.fieldName)}
            onDragOver={fieldCustomization.handleDragOver}
            onDrop={(e) => fieldCustomization.handleDrop(e, field.fieldName)}
          >
            <div className="h-6 w-6 flex items-center justify-center text-muted-foreground">
              <GripVertical className="h-4 w-4" />
            </div>
            
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-6 w-6 p-0"
              onClick={() => fieldCustomization.toggleFieldVisibility(field.fieldName)}
            >
              {field.customization.visible ? (
                <Eye className="h-3 w-3" />
              ) : (
                <EyeOff className="h-3 w-3 text-muted-foreground" />
              )}
            </Button>

            <Select 
              value={field.customization.size} 
              onValueChange={(size: FieldSize) => fieldCustomization.setFieldSize(field.fieldName, size)}
            >
              <SelectTrigger className="h-6 w-16">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="full">Full</SelectItem>
                <SelectItem value="half">Half</SelectItem>
                <SelectItem value="third">Third</SelectItem>
                <SelectItem value="quarter">Quarter</SelectItem>
              </SelectContent>
            </Select>

            <Badge variant="outline" className="text-xs">
              Order: {field.customization.order}
            </Badge>
            
            <span className="text-sm font-medium text-muted-foreground ml-2">
              {field.fieldName}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}; 