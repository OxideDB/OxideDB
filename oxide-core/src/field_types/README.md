# Extensible Field Type System

This directory contains the extensible field type system for OxideDB. Each field type is defined in its own module, making it easy to add new field types and customize their behavior.

## Architecture

- Each field type implements the `FieldTypeDefinition` trait
- Field types can define custom validation logic
- Field types can specify if they require special processing (e.g., hashing)
- Field types define their SQL storage type
- The main `FieldType` enum aggregates all available field types

## Built-in Field Types

- **Text** (`text.rs`) - Basic string field
- **Number** (`number.rs`) - Numeric field (integer or float)
- **Boolean** (`boolean.rs`) - Boolean field with smart conversion
- **Date** (`date.rs`) - Date/timestamp field with ISO 8601 support
- **JSON** (`json.rs`) - JSON object field (accepts any JSON value)
- **Email** (`email.rs`) - Email field with basic validation
- **URL** (`url.rs`) - URL field with protocol validation
- **Password** (`password.rs`) - Password field with automatic hashing
- **Phone** (`phone.rs`) - Phone number with validation

## Adding New Field Types

To add a new field type, follow these steps:

### 1. Create the Field Type Module

Create a new file in this directory (e.g., `phone.rs` for a phone number field):

```rust
//! Phone number field type definition

use super::FieldTypeDefinition;
use serde_json::Value as JsonValue;

/// Phone number field type
#[derive(Debug, Clone)]
pub struct PhoneFieldType;

impl FieldTypeDefinition for PhoneFieldType {
    fn type_name(&self) -> &'static str {
        "phone"
    }
    
    fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String> {
        // Your validation logic here
        if let Some(phone) = value.as_str() {
            // Validate phone number format
            // Return Ok(()) if valid, Err(message) if invalid
        } else {
            return Err(format!("Field '{}' must be a string", field_name));
        }
        Ok(())
    }
    
    fn convert_value(&self, value: &JsonValue) -> Result<JsonValue, String> {
        // Optional: implement automatic type conversion
        // Default implementation just returns the value unchanged
        Ok(value.clone())
    }
    
    fn sql_type(&self) -> &'static str {
        "TEXT" // or "INTEGER", "REAL", etc.
    }
    
    fn requires_hashing(&self) -> bool {
        false // Set to true if this field should be automatically hashed
    }
}
```

### 2. Add to Module Declaration

Add your new module to `mod.rs`:

```rust
pub mod phone;
pub use phone::PhoneFieldType;
```

### 3. Add to FieldType Enum

Add a new variant to the `FieldType` enum in `mod.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    // ... existing variants ...
    /// Phone number field
    Phone,
}
```

### 4. Update the Definition Method

Add your field type to the `definition()` method:

```rust
impl FieldType {
    pub fn definition(&self) -> Box<dyn FieldTypeDefinition> {
        match self {
            // ... existing cases ...
            FieldType::Phone => Box::new(PhoneFieldType),
        }
    }
}
```

### 5. Update Frontend (Optional)

If you have a UI, add the new field type to the frontend type definitions and form options.

## Field Type Trait Methods

### Required Methods

- `type_name()` - Returns the string identifier for the field type
- `validate()` - Validates a value against the field type rules
- `sql_type()` - Returns the SQL column type for database storage

### Optional Methods

- `convert_value()` - Converts values to the expected type (for auto-conversion)
- `requires_hashing()` - Returns true if the field should be automatically hashed

## Validation vs Conversion

The field type system supports both validation and automatic type conversion:

- **Validation** ensures data meets the field type requirements
- **Conversion** automatically transforms compatible values to the correct type

When auto-conversion is enabled in the schema validator, conversion happens **before** validation, allowing flexible input that gets normalized to the expected format.

## Examples

See `phone.rs` for a complete example of a custom field type with validation and normalization.

## Testing

Each field type should include comprehensive tests covering:

- Valid value validation
- Invalid value rejection
- Type conversion (if implemented)
- Edge cases and error conditions

Run tests with:
```bash
cargo test --package oxide-core field_types
```