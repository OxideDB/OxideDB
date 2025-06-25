import React, { useState, useRef, useCallback } from 'react';
import { Upload, X, File, AlertCircle, CheckCircle, Loader2, Download, Eye } from 'lucide-react';
import { Button } from './button';
import { Progress } from './progress';
import { Badge } from './badge';
import { Card, CardContent } from './card';
import { Input } from './input';
import { Label } from './label';
import { Alert, AlertDescription } from './alert';
import type { FileReference, FileFieldConfig, FileMetadata } from '../../types/api';

interface FileUploadProps {
  value: FileReference | FileReference[] | null;
  onChange: (value: FileReference | FileReference[] | null) => void;
  config: FileFieldConfig;
  collection: string;
  disabled?: boolean;
  error?: string;
}

interface UploadProgress {
  fileId: string;
  fileName: string;
  progress: number;
  status: 'uploading' | 'completed' | 'error';
  error?: string;
}

const formatFileSize = (bytes: number): string => {
  if (bytes === 0) return '0 Bytes';
  const k = 1024;
  const sizes = ['Bytes', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
};

const getFileIcon = (mimeType: string) => {
  if (mimeType.startsWith('image/')) return '🖼️';
  if (mimeType.startsWith('video/')) return '🎥';
  if (mimeType.startsWith('audio/')) return '🎵';
  if (mimeType.includes('pdf')) return '📄';
  if (mimeType.includes('document') || mimeType.includes('msword')) return '📝';
  if (mimeType.includes('spreadsheet') || mimeType.includes('excel')) return '📊';
  if (mimeType.includes('presentation') || mimeType.includes('powerpoint')) return '📋';
  if (mimeType.includes('zip') || mimeType.includes('archive')) return '📦';
  return '📁';
};

export const FileUpload: React.FC<FileUploadProps> = ({
  value,
  onChange,
  config,
  collection,
  disabled = false,
  error
}) => {
  const [uploading, setUploading] = useState<UploadProgress[]>([]);
  const [dragActive, setDragActive] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const validateFile = useCallback((file: File): string | null => {
    // Check file size
    if (config.max_file_size && file.size > config.max_file_size) {
      return `File size (${formatFileSize(file.size)}) exceeds maximum allowed size (${formatFileSize(config.max_file_size)})`;
    }

    // Check MIME type
    if (config.allowed_mime_types && config.allowed_mime_types.length > 0) {
      const isAllowed = config.allowed_mime_types.some(allowedType => {
        if (allowedType === '*') return true;
        if (allowedType.endsWith('/*')) {
          return file.type.startsWith(allowedType.slice(0, -2));
        }
        return file.type === allowedType;
      });

      if (!isAllowed) {
        return `File type ${file.type} is not allowed. Allowed types: ${config.allowed_mime_types.join(', ')}`;
      }
    }

    return null;
  }, [config]);

  const handleFileUpload = useCallback(async (files: FileList) => {
    if (disabled) return;

    const fileArray = Array.from(files);
    
    // Validate files
    const validationErrors: string[] = [];
    fileArray.forEach((file, index) => {
      const error = validateFile(file);
      if (error) {
        validationErrors.push(`File ${index + 1} (${file.name}): ${error}`);
      }
    });

    if (validationErrors.length > 0) {
      // Handle validation errors
      console.error('File validation errors:', validationErrors);
      return;
    }

    // Check if we're adding too many files for single file fields
    if (!config.multiple && fileArray.length > 1) {
      console.error('Cannot upload multiple files to single file field');
      return;
    }

    // Check if we're exceeding the multiple file limit
    const currentFiles = config.multiple ? (Array.isArray(value) ? value : []) : [];
    if (!config.multiple && currentFiles.length > 0 && fileArray.length > 0) {
      console.error('Single file field already has a file');
      return;
    }

    // Start upload process
    const { apiService } = await import('../../services/api');
    
    const uploadPromises = fileArray.map(async (file) => {
      const fileId = `${file.name}-${Date.now()}-${Math.random()}`;
      const progress: UploadProgress = {
        fileId,
        fileName: file.name,
        progress: 0,
        status: 'uploading'
      };

      setUploading(prev => [...prev, progress]);

      try {
        const metadata = await apiService.uploadFile(
          collection,
          file,
          undefined,
          (progressPercent) => {
            setUploading(prev => prev.map(p => 
              p.fileId === fileId 
                ? { ...p, progress: progressPercent }
                : p
            ));
          }
        );

        // Convert FileMetadata to FileReference
        const fileRef: FileReference = {
          file_id: metadata.file_id,
          name: metadata.name,
          mime_type: metadata.mime_type,
          size: metadata.size,
          path: metadata.path
        };

        // Update progress
        setUploading(prev => prev.map(p => 
          p.fileId === fileId 
            ? { ...p, status: 'completed', progress: 100 }
            : p
        ));

        // Update the field value
        if (config.multiple) {
          const existingFiles = Array.isArray(value) ? value : [];
          onChange([...existingFiles, fileRef]);
        } else {
          onChange(fileRef);
        }

        // Remove from uploading after a delay
        setTimeout(() => {
          setUploading(prev => prev.filter(p => p.fileId !== fileId));
        }, 2000);

      } catch (uploadError) {
        const errorMessage = uploadError instanceof Error ? uploadError.message : 'Upload failed';
        setUploading(prev => prev.map(p => 
          p.fileId === fileId 
            ? { ...p, status: 'error', error: errorMessage }
            : p
        ));
      }
    });

    await Promise.allSettled(uploadPromises);
  }, [disabled, validateFile, config, collection, value, onChange]);

  const handleDrag = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.type === 'dragenter' || e.type === 'dragover') {
      setDragActive(true);
    } else if (e.type === 'dragleave') {
      setDragActive(false);
    }
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDragActive(false);
    
    if (disabled) return;
    
    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      handleFileUpload(e.dataTransfer.files);
    }
  }, [disabled, handleFileUpload]);

  const handleFileInputChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      handleFileUpload(e.target.files);
    }
    // Reset input value to allow re-uploading the same file
    e.target.value = '';
  }, [handleFileUpload]);

  const removeFile = useCallback((fileToRemove: FileReference) => {
    if (config.multiple && Array.isArray(value)) {
      onChange(value.filter(f => f.file_id !== fileToRemove.file_id));
    } else {
      onChange(null);
    }
  }, [config.multiple, value, onChange]);

  const downloadFile = useCallback(async (fileRef: FileReference) => {
    try {
      const { apiService } = await import('../../services/api');
      const blob = await apiService.downloadFile(collection, fileRef.file_id);
      
      // Create download link
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = fileRef.name;
      document.body.appendChild(a);
      a.click();
      window.URL.revokeObjectURL(url);
      document.body.removeChild(a);
    } catch (downloadError) {
      console.error('Download failed:', downloadError);
    }
  }, [collection]);

  const currentFiles = React.useMemo(() => {
    if (!value) return [];
    return Array.isArray(value) ? value : [value];
  }, [value]);

  const canUploadMore = React.useMemo(() => {
    if (disabled) return false;
    if (!config.multiple) return currentFiles.length === 0;
    return true; // Multiple files can always add more
  }, [disabled, config.multiple, currentFiles.length]);

  return (
    <div className="space-y-3">
      {/* Current Files */}
      {currentFiles.length > 0 && (
        <div className="space-y-2">
          {currentFiles.map((fileRef) => (
            <Card key={fileRef.file_id} className="p-3">
              <div className="flex items-center justify-between">
                <div className="flex items-center space-x-3 min-w-0 flex-1">
                  <div className="text-2xl">
                    {getFileIcon(fileRef.mime_type)}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="font-medium truncate">{fileRef.name}</div>
                    <div className="text-sm text-muted-foreground">
                      {formatFileSize(fileRef.size)} • {fileRef.mime_type}
                    </div>
                  </div>
                </div>
                <div className="flex items-center space-x-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => downloadFile(fileRef)}
                    className="h-8 w-8 p-0"
                  >
                    <Download className="h-4 w-4" />
                  </Button>
                  {!disabled && (
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => removeFile(fileRef)}
                      className="h-8 w-8 p-0 text-destructive hover:text-destructive"
                    >
                      <X className="h-4 w-4" />
                    </Button>
                  )}
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}

      {/* Upload Progress */}
      {uploading.length > 0 && (
        <div className="space-y-2">
          {uploading.map((upload) => (
            <Card key={upload.fileId} className="p-3">
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <div className="flex items-center space-x-2">
                    {upload.status === 'uploading' && <Loader2 className="h-4 w-4 animate-spin" />}
                    {upload.status === 'completed' && <CheckCircle className="h-4 w-4 text-green-500" />}
                    {upload.status === 'error' && <AlertCircle className="h-4 w-4 text-red-500" />}
                    <span className="font-medium">{upload.fileName}</span>
                  </div>
                  <span className="text-sm text-muted-foreground">
                    {upload.progress}%
                  </span>
                </div>
                <Progress value={upload.progress} className="h-2" />
                {upload.status === 'error' && upload.error && (
                  <div className="text-sm text-red-500">{upload.error}</div>
                )}
              </div>
            </Card>
          ))}
        </div>
      )}

      {/* Upload Area */}
      {canUploadMore && (
        <div
          className={`
            relative border-2 border-dashed rounded-lg p-6 text-center transition-colors
            ${dragActive ? 'border-primary bg-primary/5' : 'border-muted-foreground/25'}
            ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer hover:border-primary/50'}
          `}
          onDragEnter={handleDrag}
          onDragLeave={handleDrag}
          onDragOver={handleDrag}
          onDrop={handleDrop}
          onClick={() => !disabled && fileInputRef.current?.click()}
        >
          <Input
            ref={fileInputRef}
            type="file"
            className="hidden"
            multiple={config.multiple}
            accept={config.allowed_mime_types?.join(',')}
            onChange={handleFileInputChange}
            disabled={disabled}
          />
          
          <div className="space-y-2">
            <Upload className="h-8 w-8 mx-auto text-muted-foreground" />
            <div>
              <p className="text-sm font-medium">
                {dragActive ? 'Drop files here' : 'Click to upload or drag and drop'}
              </p>
              <p className="text-xs text-muted-foreground mt-1">
                {config.allowed_mime_types && config.allowed_mime_types.length > 0 && (
                  <>Allowed: {config.allowed_mime_types.join(', ')}<br /></>
                )}
                {config.max_file_size && (
                  <>Max size: {formatFileSize(config.max_file_size)}<br /></>
                )}
                {config.multiple ? 'Multiple files allowed' : 'Single file only'}
              </p>
            </div>
          </div>
        </div>
      )}

      {/* Error Display */}
      {error && (
        <Alert variant="destructive">
          <AlertCircle className="h-4 w-4" />
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      {/* Field Info */}
      <div className="text-xs text-muted-foreground">
        {config.required && <Badge variant="destructive" className="mr-2">Required</Badge>}
        {config.multiple && <Badge variant="outline" className="mr-2">Multiple</Badge>}
        File field for {collection} collection
      </div>
    </div>
  );
}; 