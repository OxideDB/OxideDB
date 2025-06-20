import React from 'react';
import { Button } from '../ui/button';
import { Label } from '../ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../ui/select';
import { Tooltip, TooltipContent, TooltipTrigger } from '../ui/tooltip';
import { 
  ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight 
} from 'lucide-react';

interface PaginationControlsProps {
  offset: number;
  limit: number;
  total: number;
  hasMore: boolean;
  onPageChange: (newOffset: number) => void;
  onLimitChange: (newLimit: number) => void;
  loading?: boolean;
}

export const PaginationControls: React.FC<PaginationControlsProps> = ({ 
  offset, 
  limit, 
  total, 
  hasMore, 
  onPageChange, 
  onLimitChange,
  loading = false
}) => {
  const currentPage = Math.floor(offset / limit) + 1;
  const totalPages = total > 0 ? Math.ceil(total / limit) : 1;
  const hasNext = hasMore;
  const hasPrev = offset > 0;

  return (
    <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4">
      {/* Items per page selector */}
      <div className="flex items-center space-x-2">
        <Label htmlFor="page-size">Show</Label>
        <Select 
          value={limit.toString()} 
          onValueChange={(value) => onLimitChange(parseInt(value))}
          disabled={loading}
        >
          <SelectTrigger id="page-size" className="w-[80px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="25">25</SelectItem>
            <SelectItem value="50">50</SelectItem>
            <SelectItem value="100">100</SelectItem>
            <SelectItem value="200">200</SelectItem>
          </SelectContent>
        </Select>
        <span className="text-sm text-muted-foreground">entries per page</span>
      </div>

      {/* Page navigation */}
      <div className="flex flex-col sm:flex-row items-start sm:items-center gap-2 sm:gap-4">
        <span className="text-sm text-muted-foreground">
          {total > 0 
            ? `${offset + 1}-${Math.min(offset + limit, total)} of ${total.toLocaleString()}` 
            : `${offset + 1}-${offset + limit}`
          }
        </span>
        
        <div className="flex items-center space-x-1">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                onClick={() => onPageChange(0)}
                disabled={!hasPrev || loading}
                aria-label="Go to first page"
              >
                <ChevronsLeft className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Go to first page</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                onClick={() => onPageChange(Math.max(0, offset - limit))}
                disabled={!hasPrev || loading}
                aria-label="Go to previous page"
              >
                <ChevronLeft className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Previous page</TooltipContent>
          </Tooltip>
          
          <span className="text-sm px-2 py-1 bg-muted rounded min-w-[80px] text-center">
            Page {currentPage} {total > 0 ? `of ${totalPages}` : ''}
          </span>
          
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                onClick={() => onPageChange(offset + limit)}
                disabled={!hasNext || loading}
                aria-label="Go to next page"
              >
                <ChevronRight className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Next page</TooltipContent>
          </Tooltip>

          {total > 0 && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => onPageChange((totalPages - 1) * limit)}
                  disabled={!hasNext || loading}
                  aria-label="Go to last page"
                >
                  <ChevronsRight className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Go to last page</TooltipContent>
            </Tooltip>
          )}
        </div>
      </div>
    </div>
  );
}; 