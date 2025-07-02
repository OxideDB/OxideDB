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
import type { CollectionPermissionsInfo } from '@/types/api';

interface AddRuleDialogProps {
  permissionsData: CollectionPermissionsInfo[];
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
  onCreateRule?: (rule: any) => void;
}

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

  const handleCreateRule = () => {
    if (onCreateRule) {
      onCreateRule({
        collection: selectedCollection,
        operation: selectedOperation,
        rule: ruleExpression,
        description: description
      });
    }
    
    // Reset form
    setSelectedCollection("");
    setSelectedOperation("");
    setRuleExpression("");
    setDescription("");
    onOpenChange(false);
  };

  const selectedCollectionInfo = permissionsData.find(info => info.collection_name === selectedCollection);
  const isAuthCollection = selectedCollectionInfo?.collection_type === 'auth';

  return (
    <Dialog open={isOpen} onOpenChange={onOpenChange}>
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
              <Select value={selectedCollection} onValueChange={setSelectedCollection}>
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
                  {/* CRUD Operations */}
                  <SelectItem value="create">Create</SelectItem>
                  <SelectItem value="read">Read</SelectItem>
                  <SelectItem value="update">Update</SelectItem>
                  <SelectItem value="delete">Delete</SelectItem>
                  <SelectItem value="list">List</SelectItem>
                  
                  {/* Auth Operations - Only show for auth collections */}
                  {isAuthCollection && (
                    <>
                      <SelectItem value="login">Login</SelectItem>
                      <SelectItem value="register">Register</SelectItem>
                      <SelectItem value="token_validation">Token Validation</SelectItem>
                      <SelectItem value="token_refresh">Token Refresh</SelectItem>
                      <SelectItem value="logout">Logout</SelectItem>
                      <SelectItem value="get_current_user">Get Current User</SelectItem>
                      <SelectItem value="list_auth_collections">List Auth Collections</SelectItem>
                    </>
                  )}
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
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button 
            onClick={handleCreateRule}
            disabled={!selectedCollection || !selectedOperation || !ruleExpression}
          >
            Create Rule
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
};
