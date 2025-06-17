# OxideDB Authorization Rules Examples

This document demonstrates the comprehensive authorization rule system in OxideDB, which supports custom rules with variable substitution and expression evaluation.

## Available Variables

### Request Variables
- `@req.headers.x-api-key` - API key from headers
- `@req.headers.authorization` - Authorization header
- `@req.user.id` - Authenticated user ID
- `@req.user.role` - User role
- `@req.user.email` - User email address
- `@req.user.{custom_field}` - Custom fields from JWT claims

### Record Variables
- `@record.{field_name}` - Any field in the record
- `@record.user_id` - Record owner ID (common pattern)

### System Variables
- `@now` - Current timestamp
- `@now.hour` - Current hour (0-23)
- `@now.minute` - Current minute (0-59)
- `@now.day` - Current day of month (1-31)
- `@now.month` - Current month (1-12)
- `@now.year` - Current year
- `@now.weekday` - Current weekday (0=Sunday, 1=Monday, etc.)

## Rule Examples

### Basic Access Control

```javascript
// Public access
"true"

// No access
"false"

// Authenticated users only
"@req.user.id != ''"

// Superuser only
"@req.user.role = 'superuser'"
```

### API Key Based Access

```javascript
// Require any API key
"@req.headers.x-api-key != ''"

// Require specific API key
"@req.headers.x-api-key = 'your-secret-key'"

// Multiple valid API keys
"@req.headers.x-api-key = 'key1' || @req.headers.x-api-key = 'key2'"
```

### Owner-Based Access

```javascript
// Users can only access their own records
"@req.user.id = @record.user_id"

// Users can access their own records or admins can access any
"@req.user.id = @record.user_id || @req.user.role = 'admin'"

// Only active records can be accessed by owners
"@req.user.id = @record.user_id && @record.status = 'active'"
```

### Time-Based Access

```javascript
// Business hours only (9 AM - 5 PM)
"@now.hour >= 9 && @now.hour <= 17"

// Weekend access only
"@now.weekday = 0 || @now.weekday = 6"

// No access during maintenance window (2-4 AM)
"@now.hour < 2 || @now.hour >= 4"

// Monthly access window (first week of month)
"@now.day <= 7"
```

### IP-Based Access

```javascript
// Internal network only
"@req.headers.x-forwarded-for ~ '192.168.*'"

// Specific IP whitelist
"@req.headers.x-forwarded-for ~ '192.168.1.100' || @req.headers.x-forwarded-for ~ '10.0.0.*'"

// Block specific IPs
"@req.headers.x-forwarded-for !~ '192.168.1.50'"
```

### Complex Rules

```javascript
// Multi-factor rule: authenticated user from internal network during business hours
"@req.user.id != '' && @req.headers.x-forwarded-for ~ '192.168.*' && @now.hour >= 9 && @now.hour <= 17"

// Hierarchical access: owners always, managers during business hours, admins anytime
"@req.user.id = @record.user_id || (@req.user.role = 'manager' && @now.hour >= 9 && @now.hour <= 17) || @req.user.role = 'admin'"

// Content-based access: users can edit their own draft posts
"@req.user.id = @record.author_id && @record.status = 'draft'"

// Geographic and time restrictions
"@req.headers.x-country = 'US' && @now.hour >= 6 && @now.hour <= 22"
```

### Status-Based Rules

```javascript
// Only published content is publicly readable
"@record.status = 'published'"

// Users can edit their own drafts and pending posts
"@req.user.id = @record.user_id && (@record.status = 'draft' || @record.status = 'pending')"

// Admins can access everything, users only active records
"@req.user.role = 'admin' || @record.status = 'active'"
```

## Operators Supported

### Comparison Operators
- `=` - Equality
- `!=` - Not equal
- `>`, `<`, `>=`, `<=` - Numeric comparisons
- `~` - Pattern matching (glob-style with * and ?)
- `!~` - Negative pattern matching

### Logical Operators
- `&&` - Logical AND
- `||` - Logical OR

### Pattern Matching Examples

```javascript
// Wildcard matching
"@req.headers.user-agent ~ '*Chrome*'"

// Multiple patterns
"@req.headers.x-api-key ~ 'dev-*' || @req.headers.x-api-key ~ 'test-*'"

// File extension matching
"@record.filename ~ '*.pdf' || @record.filename ~ '*.doc'"
```

## Best Practices

1. **Start Simple**: Begin with basic rules and add complexity as needed
2. **Test Thoroughly**: Use the built-in test framework to verify rule behavior
3. **Document Rules**: Complex rules should be documented for maintainability
4. **Performance**: Simple rules evaluate faster than complex ones
5. **Security**: Always default to restrictive rules and explicitly allow access
6. **Fallbacks**: Consider what happens when variables are missing or empty

## Common Patterns

### Multi-Tenant Applications
```javascript
// Users can only access data from their organization
"@req.user.org_id = @record.org_id"
```

### Hierarchical Permissions
```javascript
// Managers can access their team's data
"@req.user.team_id = @record.team_id && @req.user.role = 'manager'"
```

### Audit Trail Protection
```javascript
// Audit logs are read-only except for system
"@req.user.role = 'system' || @req.method = 'GET'"
```

### Feature Flags
```javascript
// Beta features only for beta users
"@req.user.beta_user = true || @req.user.role = 'admin'"
``` 