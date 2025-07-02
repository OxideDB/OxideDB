# Permissions Components Architecture

This document outlines the refactored permissions system structure following React best practices for maintainability and scalability.

## File Structure

```
ui/src/
├── components/permissions/
│   ├── index.ts                    # Component exports
│   ├── AccessRulesTab.tsx          # Main access rules management tab
│   ├── CollectionRulesTab.tsx      # Collection overview tab
│   ├── RuleExamplesTab.tsx         # Rule examples and documentation tab
│   ├── PermissionRuleEditor.tsx    # Rule editing component
│   └── AddRuleDialog.tsx           # Dialog for adding new rules
├── hooks/permissions/
│   └── usePermissions.ts           # Custom hook for permissions state management
├── utils/permissions/
│   ├── permissionUtils.tsx         # Icon and display utility functions
│   └── constants.ts                # Rule examples and variable constants
└── pages/
    └── Permissions.tsx             # Main page component (now simplified)
```

## Component Breakdown

### 1. Main Page Component (`Permissions.tsx`)
- **Size**: Reduced from ~928 lines to ~89 lines
- **Responsibility**: Layout, tab navigation, and state coordination
- **Dependencies**: Uses custom hook and imported tab components

### 2. Custom Hook (`usePermissions.ts`)
- **Responsibility**: Manages all permissions-related API calls and state
- **Returns**: Permissions data, loading states, error handling, and action functions
- **Benefits**: Reusable logic, centralized state management

### 3. Utility Functions (`permissionUtils.tsx`)
- **Functions**: 
  - `getPermissionLevelDisplay()` - Permission level styling and icons
  - `getCrudOperationIcon()` - CRUD operation icons
  - `getAuthOperationIcon()` - Auth operation icons
  - `getOperationColor()` - Operation color coding
  - `sortCrudOperations()` - Consistent CRUD operation ordering
  - `sortAuthOperations()` - Consistent auth operation ordering

### 4. Constants (`constants.ts`)
- **Content**: Rule examples, available variables documentation
- **Benefits**: Centralized configuration, easy to update

### 5. Tab Components

#### `AccessRulesTab.tsx`
- **Responsibility**: Main rules management interface
- **Features**: Edit mode, rule creation, mobile/desktop layouts
- **Components Used**: `PermissionRuleEditor`, `AddRuleDialog`

#### `CollectionRulesTab.tsx`
- **Responsibility**: Overview of all collection permissions
- **Features**: Read-only collection rules display

#### `RuleExamplesTab.tsx`
- **Responsibility**: Documentation and examples
- **Features**: Rule examples, variable reference

#### `PermissionRuleEditor.tsx`
- **Responsibility**: In-line rule editing interface
- **Features**: CRUD/Auth operation management, rule expressions

#### `AddRuleDialog.tsx`
- **Responsibility**: Dialog for creating new rules
- **Features**: Collection/operation selection, rule validation

## Benefits of This Architecture

### 1. **Maintainability**
- Small, focused components with single responsibilities
- Clear separation of concerns
- Easy to locate and modify specific functionality

### 2. **Reusability**
- Utility functions can be used across different components
- Custom hook can be used in other permission-related pages
- Components can be easily imported and reused

### 3. **Testability**
- Each component can be tested in isolation
- Utility functions are pure functions, easy to unit test
- Custom hook can be tested separately from UI components

### 4. **Performance**
- Components can be lazy-loaded if needed
- Smaller bundle sizes per component
- Better tree-shaking opportunities

### 5. **Developer Experience**
- Easier to navigate and understand codebase
- Multiple developers can work on different components simultaneously
- Clear component boundaries and interfaces

## Usage Example

```tsx
// Using the main component
import Permissions from '@/pages/Permissions';

// Using individual components
import { AccessRulesTab, usePermissions } from '@/components/permissions';

function CustomPermissionsPage() {
  const permissions = usePermissions();
  
  return (
    <AccessRulesTab 
      permissionsData={permissions.permissionsData}
      onUpdatePermissions={permissions.updateCollectionPermissions}
    />
  );
}
```

## Migration Notes

- The original monolithic component has been completely refactored
- All functionality has been preserved but reorganized
- API interfaces remain the same
- No breaking changes to parent components or consumers
- TypeScript types and error handling preserved
