import React, { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/components/ui/select';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Plus } from 'lucide-react';
import type { AuthOperation, CollectionPermissionsInfo, CrudOperation } from '@/types/api';

export interface CreateRuleInput {
  collection: string;
  operation: CrudOperation | AuthOperation;
  rule: string;
  description?: string;
}

interface AddRuleDialogProps {
  permissionsData: CollectionPermissionsInfo[];
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
  onCreateRule?: (rule: CreateRuleInput) => Promise<void> | void;
}

const crudOperations: Array<{ value: CrudOperation; label: string }> = [
  { value: 'create', label: 'Create' },
  { value: 'read', label: 'Read' },
  { value: 'update', label: 'Update' },
  { value: 'delete', label: 'Delete' },
  { value: 'list', label: 'List' },
];

const authOperations: Array<{ value: AuthOperation; label: string }> = [
  { value: 'login', label: 'Login' },
  { value: 'register', label: 'Register' },
  { value: 'token_validation', label: 'Token Validation' },
  { value: 'token_refresh', label: 'Token Refresh' },
  { value: 'logout', label: 'Logout' },
  { value: 'get_current_user', label: 'Get Current User' },
  { value: 'list_auth_collections', label: 'List Auth Collections' },
];

export const AddRuleDialog: React.FC<AddRuleDialogProps> = ({
  permissionsData,
  isOpen,
  onOpenChange,
  onCreateRule
}) => {
  const [selectedCollection, setSelectedCollection] = useState("");
  const [selectedOperation, setSelectedOperation] = useState("");
  const [ruleExpression, setRuleExpression] = useState("");
  const [description, setDescription] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  const resetForm = () => {
    setSelectedCollection("");
    setSelectedOperation("");
    setRuleExpression("");
    setDescription("");
  };

  const handleOpenChange = (open: boolean) => {
    if (!open) {
      resetForm();
    }
    onOpenChange(open);
  };

  const handleCollectionChange = (collection: string) => {
    setSelectedCollection(collection);
    setSelectedOperation("");
  };

  const handleCreateRule = async () => {
    if (onCreateRule) {
      setIsSubmitting(true);
      try {
        await onCreateRule({
          collection: selectedCollection,
          operation: selectedOperation as CrudOperation | AuthOperation,
          rule: ruleExpression.trim(),
          description: description.trim() || undefined,
        });
        resetForm();
        onOpenChange(false);
      } finally {
        setIsSubmitting(false);
      }
    }
  };

  const selectedCollectionInfo = permissionsData.find(info => info.collection_name === selectedCollection);
  const isAuthCollection = selectedCollectionInfo?.collection_type === 'auth';
  const availableOperations = isAuthCollection
    ? [...crudOperations, ...authOperations]
    : crudOperations;
  const canCreateRule = Boolean(selectedCollection && selectedOperation && ruleExpression.trim());

  return (
    <Dialog open={isOpen} onOpenChange={handleOpenChange}>
      <DialogTrigger asChild>
        <Button className="w-full sm:w-auto">
          <Plus className="h-4 w-4 mr-2" />
          Add Rule
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-2xl mx-4">
        <DialogHeader>
          <DialogTitle>Create Access Rule</DialogTitle>
          <DialogDescription>Define a new access rule for a collection</DialogDescription>
        </DialogHeader>
        <div className="space-y-4">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <Label htmlFor="rule-collection">Collection</Label>
              <Select value={selectedCollection} onValueChange={handleCollectionChange}>
                <SelectTrigger>
                  <SelectValue placeholder="Select collection" />
                </SelectTrigger>
                <SelectContent>
                  {permissionsData.map((info) => (
                    <SelectItem key={info.collection_name} value={info.collection_name}>
                      {info.collection_name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div>
              <Label htmlFor="rule-operation">Operation</Label>
              <Select value={selectedOperation} onValueChange={setSelectedOperation}>
                <SelectTrigger>
                  <SelectValue placeholder="Select operation" />
                </SelectTrigger>
                <SelectContent>
                  {availableOperations.map((operation) => (
                    <SelectItem key={operation.value} value={operation.value}>
                      {operation.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
          <div>
            <Label htmlFor="rule-expression">Rule Expression</Label>
            <Textarea
              id="rule-expression"
              placeholder="@req.headers.x-api-key = 'your-secret-key'"
              className="font-mono text-sm min-h-[100px]"
              value={ruleExpression}
              onChange={(e) => setRuleExpression(e.target.value)}
            />
            <p className="text-xs text-muted-foreground mt-1">
              Use expressions like @req.headers.x-api-key, @req.user.id, @record.field_name
            </p>
          </div>
          <div>
            <Label htmlFor="rule-description">Description</Label>
            <Input
              id="rule-description"
              placeholder="Describe what this rule does..."
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => handleOpenChange(false)}>
            Cancel
          </Button>
          <Button
            onClick={handleCreateRule}
            disabled={!canCreateRule || isSubmitting}
          >
            {isSubmitting ? 'Saving...' : 'Create Rule'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
};
