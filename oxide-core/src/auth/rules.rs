//! Permission rule evaluation
//!
//! This module provides a comprehensive rule evaluator for custom permission expressions.
//! Rules are evaluated in the context of a specific request and can access request metadata,
//! user information, record data, and system variables.

use super::permissions::PermissionContext;
use crate::AppError;
use chrono::{DateTime, Datelike, Timelike, Utc};
use regex::Regex;
use sha2::{Digest, Sha256};
use tracing::debug;

/// Rule evaluator for custom permission expressions
///
/// This evaluator supports a comprehensive set of variables and operators for creating
/// flexible authorization rules. Rules are evaluated in the context of a specific
/// request and can access request metadata, user information, record data, and system variables.
///
/// ## Supported Variables
///
/// ### Request Variables
/// - `@req.headers.{header-name}` - Access HTTP headers (e.g., `@req.headers.x-api-key`)
/// - `@req.headers.{header-name}.sha256` - Access a SHA-256 hex digest of a header value
/// - `@req.user.id` - Authenticated user ID
/// - `@req.user.role` - User role (user, superuser, or custom)
/// - `@req.user.email` - User email address
/// - `@req.user.auth_collection` - Collection the user authenticated from
/// - `@req.user.{custom_field}` - Custom fields from JWT claims
///
/// ### Record Variables
/// - `@record.{field_name}` - Any field in the record being accessed
/// - `@record.user_id` - Common pattern for record owner ID
///
/// ### System Variables
/// - `@now` - Current Unix timestamp
/// - `@now.hour` - Current hour (0-23)
/// - `@now.minute` - Current minute (0-59)
/// - `@now.day` - Current day of month (1-31)
/// - `@now.month` - Current month (1-12)
/// - `@now.year` - Current year
/// - `@now.weekday` - Current weekday (0=Sunday, 1=Monday, etc.)
///
/// ## Supported Operators
///
/// ### Comparison Operators
/// - `=` - Equality
/// - `!=` - Not equal
/// - `>`, `<`, `>=`, `<=` - Numeric comparisons
/// - `~` - Pattern matching (supports * and ? wildcards)
///
/// ### Logical Operators
/// - `&&` - Logical AND
/// - `||` - Logical OR
///
/// ## Example Rules
///
/// ```text
/// // Public access
/// "true"
///
/// // Require any API key
/// "@req.headers.x-api-key != ''"
///
/// // Require specific API key without storing the raw key in the rule
/// "@req.headers.x-api-key.sha256 = '<sha256-hex>'"
///
/// // Authenticated users only
/// "@req.user.id != ''"
///
/// // Admin users only
/// "@req.user.role = 'superuser'"
///
/// // Owner access (users can only access their own records)
/// "@req.user.id = @record.user_id"
///
/// // IP whitelist
/// "@req.headers.x-forwarded-for ~ '192.168.1.*'"
///
/// // Time-based access (business hours only)
/// "@now.hour >= 9 && @now.hour <= 17"
///
/// // Complex rule combining multiple conditions
/// "@req.user.role = 'admin' || (@req.user.id = @record.user_id && @record.status = 'active')"
/// ```
///
/// ## Implementation Notes
///
/// - String values are automatically quoted during expansion
/// - Missing variables default to empty strings
/// - Numeric comparisons work with both integers and floats
/// - Pattern matching uses glob-style wildcards converted to regex
/// - Boolean literals `true` and `false` are supported
/// - Complex expressions are evaluated left-to-right with standard operator precedence
pub struct RuleEvaluator<'a> {
    context: &'a PermissionContext,
    current_time: DateTime<Utc>,
}

impl<'a> RuleEvaluator<'a> {
    /// Create a new rule evaluator
    pub fn new(context: &'a PermissionContext) -> Self {
        Self {
            context,
            current_time: Utc::now(),
        }
    }

    /// Evaluate a rule expression
    pub fn evaluate(&self, rule_expr: &str) -> Result<bool, AppError> {
        debug!("🔍 RuleEvaluator: Starting permission rule evaluation");

        let expression = RuleParser::parse(rule_expr)?;
        let result = self.evaluate_rule_expression(&expression);
        debug!("🔍 RuleEvaluator: Final result: {:?}", result);

        result
    }

    /// Validate a custom permission rule without evaluating it against request data.
    pub fn validate_syntax(rule_expr: &str) -> Result<(), AppError> {
        RuleParser::parse(rule_expr).map(|_| ())
    }

    fn evaluate_rule_expression(&self, expression: &RuleExpression) -> Result<bool, AppError> {
        match expression {
            RuleExpression::Boolean(value) => Ok(*value),
            RuleExpression::Comparison(left, operator, right) => {
                let left = self.evaluate_value(left)?;
                let right = self.evaluate_value(right)?;
                evaluate_comparison(left, *operator, right)
            }
            RuleExpression::Logical(left, operator, right) => {
                let left = self.evaluate_rule_expression(left)?;

                match operator {
                    LogicalOperator::And if !left => Ok(false),
                    LogicalOperator::Or if left => Ok(true),
                    LogicalOperator::And => Ok(self.evaluate_rule_expression(right)?),
                    LogicalOperator::Or => Ok(self.evaluate_rule_expression(right)?),
                }
            }
        }
    }

    fn evaluate_value(&self, value: &RuleValueExpression) -> Result<RuleValue, AppError> {
        match value {
            RuleValueExpression::String(value) => Ok(RuleValue::String(value.clone())),
            RuleValueExpression::Number(value) => Ok(RuleValue::Number(*value)),
            RuleValueExpression::Boolean(value) => Ok(RuleValue::Boolean(*value)),
            RuleValueExpression::Variable(variable) => self.resolve_variable(variable),
        }
    }

    fn resolve_variable(&self, variable: &str) -> Result<RuleValue, AppError> {
        if let Some(header_path) = variable.strip_prefix("@req.headers.") {
            return self.resolve_request_header(header_path);
        }

        if let Some(field_name) = variable.strip_prefix("@req.user.") {
            return Ok(self.resolve_request_user(field_name));
        }

        if let Some(field_name) = variable.strip_prefix("@record.") {
            return Ok(self.resolve_record_field(field_name));
        }

        if variable == "@now" {
            return Ok(RuleValue::Number(self.current_time.timestamp() as f64));
        }

        if let Some(field_name) = variable.strip_prefix("@now.") {
            return self.resolve_system_time(field_name);
        }

        Err(rule_validation_error(format!(
            "Unsupported variable '{}'",
            variable
        )))
    }

    fn resolve_request_header(&self, header_path: &str) -> Result<RuleValue, AppError> {
        let (header_name, hash_value) = header_path
            .strip_suffix(".sha256")
            .map(|name| (name, true))
            .unwrap_or((header_path, false));

        validate_header_name(header_name)?;

        debug!("🔍 Header lookup: Looking for header '{}'", header_name);

        if let Some(headers) = self.context.metadata.get("headers") {
            if let Some(header_value) = headers.get(header_name).and_then(|value| value.as_str()) {
                return Ok(RuleValue::String(header_rule_value(
                    header_value,
                    hash_value,
                )));
            }

            if let Some(headers_obj) = headers.as_object() {
                let target_header_lower = header_name.to_lowercase();

                for (key, value) in headers_obj {
                    if key.to_lowercase() == target_header_lower {
                        if let Some(value_str) = value.as_str() {
                            return Ok(RuleValue::String(header_rule_value(value_str, hash_value)));
                        }
                    }
                }
            }
        }

        Ok(RuleValue::String(String::new()))
    }

    fn resolve_request_user(&self, field_name: &str) -> RuleValue {
        let Some(claims) = &self.context.user_claims else {
            return RuleValue::String(String::new());
        };

        match field_name {
            "id" => RuleValue::String(claims.sub.clone()),
            "role" => RuleValue::String(claims.role.clone()),
            "email" => RuleValue::String(claims.email.clone()),
            "auth_collection" => RuleValue::String(claims.auth_collection.clone()),
            _ => claims
                .custom
                .get(field_name)
                .map(json_to_rule_value)
                .unwrap_or_else(|| RuleValue::String(String::new())),
        }
    }

    fn resolve_record_field(&self, field_name: &str) -> RuleValue {
        self.context
            .record_data
            .as_ref()
            .and_then(|record_data| record_data.get(field_name))
            .map(json_to_rule_value)
            .unwrap_or_else(|| RuleValue::String(String::new()))
    }

    fn resolve_system_time(&self, field_name: &str) -> Result<RuleValue, AppError> {
        let value = match field_name {
            "hour" => self.current_time.hour() as f64,
            "minute" => self.current_time.minute() as f64,
            "day" => self.current_time.day() as f64,
            "month" => self.current_time.month() as f64,
            "year" => self.current_time.year() as f64,
            "weekday" => self.current_time.weekday().num_days_from_sunday() as f64,
            _ => {
                return Err(rule_validation_error(format!(
                    "Unsupported @now field '{}'",
                    field_name
                )));
            }
        };

        Ok(RuleValue::Number(value))
    }
}

#[derive(Debug, Clone)]
enum RuleExpression {
    Boolean(bool),
    Comparison(RuleValueExpression, ComparisonOperator, RuleValueExpression),
    Logical(Box<RuleExpression>, LogicalOperator, Box<RuleExpression>),
}

#[derive(Debug, Clone)]
enum RuleValueExpression {
    String(String),
    Number(f64),
    Boolean(bool),
    Variable(String),
}

#[derive(Debug, Clone, Copy)]
enum ComparisonOperator {
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Matches,
}

#[derive(Debug, Clone, Copy)]
enum LogicalOperator {
    And,
    Or,
}

#[derive(Debug, Clone)]
enum RuleValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

impl RuleValue {
    fn as_string(&self) -> String {
        match self {
            RuleValue::String(value) => value.clone(),
            RuleValue::Number(value) => {
                if value.fract() == 0.0 {
                    format!("{}", *value as i64)
                } else {
                    value.to_string()
                }
            }
            RuleValue::Boolean(value) => value.to_string(),
        }
    }

    fn as_number(&self) -> Option<f64> {
        match self {
            RuleValue::Number(value) => Some(*value),
            RuleValue::String(value) => value.parse::<f64>().ok(),
            RuleValue::Boolean(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum RuleToken {
    Boolean(bool),
    String(String),
    Number(f64),
    Variable(String),
    LeftParen,
    RightParen,
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Matches,
    And,
    Or,
    End,
}

struct RuleParser {
    tokens: Vec<RuleToken>,
    position: usize,
}

impl RuleParser {
    fn parse(input: &str) -> Result<RuleExpression, AppError> {
        let input = input.trim();
        if input.is_empty() {
            return Err(rule_validation_error(
                "Rule expression cannot be empty".to_string(),
            ));
        }

        let mut parser = Self {
            tokens: lex_rule(input)?,
            position: 0,
        };
        let expression = parser.parse_or()?;
        parser.expect_end()?;
        Ok(expression)
    }

    fn parse_or(&mut self) -> Result<RuleExpression, AppError> {
        let mut expression = self.parse_and()?;

        while matches!(self.current(), RuleToken::Or) {
            self.advance();
            let right = self.parse_and()?;
            expression =
                RuleExpression::Logical(Box::new(expression), LogicalOperator::Or, Box::new(right));
        }

        Ok(expression)
    }

    fn parse_and(&mut self) -> Result<RuleExpression, AppError> {
        let mut expression = self.parse_primary()?;

        while matches!(self.current(), RuleToken::And) {
            self.advance();
            let right = self.parse_primary()?;
            expression = RuleExpression::Logical(
                Box::new(expression),
                LogicalOperator::And,
                Box::new(right),
            );
        }

        Ok(expression)
    }

    fn parse_primary(&mut self) -> Result<RuleExpression, AppError> {
        if matches!(self.current(), RuleToken::LeftParen) {
            self.advance();
            let expression = self.parse_or()?;
            self.expect_right_paren()?;
            return Ok(expression);
        }

        let left = self.parse_value()?;
        let Some(operator) = self.parse_comparison_operator() else {
            return match left {
                RuleValueExpression::Boolean(value) => Ok(RuleExpression::Boolean(value)),
                _ => Err(rule_validation_error(
                    "Expected comparison operator".to_string(),
                )),
            };
        };
        let right = self.parse_value()?;

        Ok(RuleExpression::Comparison(left, operator, right))
    }

    fn parse_value(&mut self) -> Result<RuleValueExpression, AppError> {
        let value = match self.current().clone() {
            RuleToken::Boolean(value) => RuleValueExpression::Boolean(value),
            RuleToken::String(value) => RuleValueExpression::String(value),
            RuleToken::Number(value) => RuleValueExpression::Number(value),
            RuleToken::Variable(value) => RuleValueExpression::Variable(value),
            token => {
                return Err(rule_validation_error(format!(
                    "Expected value, found {:?}",
                    token
                )));
            }
        };

        self.advance();
        Ok(value)
    }

    fn parse_comparison_operator(&mut self) -> Option<ComparisonOperator> {
        let operator = match self.current() {
            RuleToken::Equal => ComparisonOperator::Equal,
            RuleToken::NotEqual => ComparisonOperator::NotEqual,
            RuleToken::GreaterThan => ComparisonOperator::GreaterThan,
            RuleToken::GreaterThanOrEqual => ComparisonOperator::GreaterThanOrEqual,
            RuleToken::LessThan => ComparisonOperator::LessThan,
            RuleToken::LessThanOrEqual => ComparisonOperator::LessThanOrEqual,
            RuleToken::Matches => ComparisonOperator::Matches,
            _ => return None,
        };

        self.advance();
        Some(operator)
    }

    fn expect_right_paren(&mut self) -> Result<(), AppError> {
        if !matches!(self.current(), RuleToken::RightParen) {
            return Err(rule_validation_error("Expected ')'".to_string()));
        }

        self.advance();
        Ok(())
    }

    fn expect_end(&self) -> Result<(), AppError> {
        if !matches!(self.current(), RuleToken::End) {
            return Err(rule_validation_error(format!(
                "Unexpected token {:?}",
                self.current()
            )));
        }

        Ok(())
    }

    fn current(&self) -> &RuleToken {
        self.tokens.get(self.position).unwrap_or(&RuleToken::End)
    }

    fn advance(&mut self) {
        self.position = self.position.saturating_add(1);
    }
}

fn header_rule_value(value: &str, hash_value: bool) -> String {
    if hash_value {
        sha256_hex(value)
    } else {
        value.to_string()
    }
}

fn sha256_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn lex_rule(input: &str) -> Result<Vec<RuleToken>, AppError> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some((index, ch)) = chars.peek().copied() {
        match ch {
            c if c.is_whitespace() => {
                chars.next();
            }
            '(' => {
                chars.next();
                tokens.push(RuleToken::LeftParen);
            }
            ')' => {
                chars.next();
                tokens.push(RuleToken::RightParen);
            }
            '&' => {
                chars.next();
                if matches!(chars.next(), Some((_, '&'))) {
                    tokens.push(RuleToken::And);
                } else {
                    return Err(rule_validation_error(format!(
                        "Expected '&&' at byte {}",
                        index
                    )));
                }
            }
            '|' => {
                chars.next();
                if matches!(chars.next(), Some((_, '|'))) {
                    tokens.push(RuleToken::Or);
                } else {
                    return Err(rule_validation_error(format!(
                        "Expected '||' at byte {}",
                        index
                    )));
                }
            }
            '=' => {
                chars.next();
                tokens.push(RuleToken::Equal);
            }
            '!' => {
                chars.next();
                if matches!(chars.next(), Some((_, '='))) {
                    tokens.push(RuleToken::NotEqual);
                } else {
                    return Err(rule_validation_error(format!(
                        "Expected '!=' at byte {}",
                        index
                    )));
                }
            }
            '>' => {
                chars.next();
                if matches!(chars.peek(), Some((_, '='))) {
                    chars.next();
                    tokens.push(RuleToken::GreaterThanOrEqual);
                } else {
                    tokens.push(RuleToken::GreaterThan);
                }
            }
            '<' => {
                chars.next();
                if matches!(chars.peek(), Some((_, '='))) {
                    chars.next();
                    tokens.push(RuleToken::LessThanOrEqual);
                } else {
                    tokens.push(RuleToken::LessThan);
                }
            }
            '~' => {
                chars.next();
                tokens.push(RuleToken::Matches);
            }
            '\'' | '"' => tokens.push(read_quoted_string(&mut chars)?),
            '@' => tokens.push(read_variable(&mut chars)?),
            '-' | '0'..='9' => tokens.push(read_number(&mut chars)?),
            c if c.is_ascii_alphabetic() => tokens.push(read_identifier(&mut chars)?),
            _ => {
                return Err(rule_validation_error(format!(
                    "Unexpected character '{}' at byte {}",
                    ch, index
                )));
            }
        }
    }

    tokens.push(RuleToken::End);
    Ok(tokens)
}

fn read_quoted_string<I>(chars: &mut std::iter::Peekable<I>) -> Result<RuleToken, AppError>
where
    I: Iterator<Item = (usize, char)>,
{
    let Some((start, quote)) = chars.next() else {
        return Err(rule_validation_error("Expected quoted string".to_string()));
    };
    let mut value = String::new();
    let mut escaped = false;

    for (_, ch) in chars.by_ref() {
        if escaped {
            value.push(ch);
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        if ch == quote {
            return Ok(RuleToken::String(value));
        }

        value.push(ch);
    }

    Err(rule_validation_error(format!(
        "Unterminated string starting at byte {}",
        start
    )))
}

fn read_variable<I>(chars: &mut std::iter::Peekable<I>) -> Result<RuleToken, AppError>
where
    I: Iterator<Item = (usize, char)>,
{
    let mut value = String::new();

    while let Some((_, ch)) = chars.peek().copied() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '@' | '.' | '_' | '-') {
            value.push(ch);
            chars.next();
        } else {
            break;
        }
    }

    validate_variable_name(&value)?;
    Ok(RuleToken::Variable(value))
}

fn read_number<I>(chars: &mut std::iter::Peekable<I>) -> Result<RuleToken, AppError>
where
    I: Iterator<Item = (usize, char)>,
{
    let mut value = String::new();
    let mut has_digit = false;
    let mut has_decimal = false;

    if matches!(chars.peek(), Some((_, '-'))) {
        value.push('-');
        chars.next();
    }

    while let Some((_, ch)) = chars.peek().copied() {
        match ch {
            '0'..='9' => {
                has_digit = true;
                value.push(ch);
                chars.next();
            }
            '.' if !has_decimal => {
                has_decimal = true;
                value.push(ch);
                chars.next();
            }
            _ => break,
        }
    }

    if !has_digit {
        return Err(rule_validation_error("Invalid numeric literal".to_string()));
    }

    value
        .parse::<f64>()
        .map(RuleToken::Number)
        .map_err(|e| rule_validation_error(format!("Invalid numeric literal '{}': {}", value, e)))
}

fn read_identifier<I>(chars: &mut std::iter::Peekable<I>) -> Result<RuleToken, AppError>
where
    I: Iterator<Item = (usize, char)>,
{
    let mut value = String::new();

    while let Some((_, ch)) = chars.peek().copied() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            value.push(ch);
            chars.next();
        } else {
            break;
        }
    }

    match value.as_str() {
        "true" => Ok(RuleToken::Boolean(true)),
        "false" => Ok(RuleToken::Boolean(false)),
        _ => Err(rule_validation_error(format!(
            "Unsupported bare identifier '{}'",
            value
        ))),
    }
}

fn validate_variable_name(variable: &str) -> Result<(), AppError> {
    if variable == "@now" {
        return Ok(());
    }

    if let Some(header_path) = variable.strip_prefix("@req.headers.") {
        let header_name = header_path.strip_suffix(".sha256").unwrap_or(header_path);
        return validate_header_name(header_name);
    }

    if let Some(field_name) = variable.strip_prefix("@req.user.") {
        return validate_identifier("request user field", field_name);
    }

    if let Some(field_name) = variable.strip_prefix("@record.") {
        return validate_identifier("record field", field_name);
    }

    if let Some(field_name) = variable.strip_prefix("@now.") {
        return match field_name {
            "hour" | "minute" | "day" | "month" | "year" | "weekday" => Ok(()),
            _ => Err(rule_validation_error(format!(
                "Unsupported @now field '{}'",
                field_name
            ))),
        };
    }

    Err(rule_validation_error(format!(
        "Unsupported variable '{}'",
        variable
    )))
}

fn validate_identifier(kind: &str, value: &str) -> Result<(), AppError> {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Ok(());
    }

    Err(rule_validation_error(format!(
        "Invalid {} '{}'",
        kind, value
    )))
}

fn validate_header_name(value: &str) -> Result<(), AppError> {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Ok(());
    }

    Err(rule_validation_error(format!(
        "Invalid request header variable '{}'",
        value
    )))
}

fn json_to_rule_value(value: &serde_json::Value) -> RuleValue {
    match value {
        serde_json::Value::Bool(value) => RuleValue::Boolean(*value),
        serde_json::Value::Number(value) => value
            .as_f64()
            .map(RuleValue::Number)
            .unwrap_or_else(|| RuleValue::String(value.to_string())),
        serde_json::Value::String(value) => RuleValue::String(value.clone()),
        serde_json::Value::Null => RuleValue::String(String::new()),
        other => RuleValue::String(other.to_string()),
    }
}

fn evaluate_comparison(
    left: RuleValue,
    operator: ComparisonOperator,
    right: RuleValue,
) -> Result<bool, AppError> {
    match operator {
        ComparisonOperator::Equal => Ok(values_equal(&left, &right)),
        ComparisonOperator::NotEqual => Ok(!values_equal(&left, &right)),
        ComparisonOperator::GreaterThan => compare_numbers(left, right, |left, right| left > right),
        ComparisonOperator::GreaterThanOrEqual => {
            compare_numbers(left, right, |left, right| left >= right)
        }
        ComparisonOperator::LessThan => compare_numbers(left, right, |left, right| left < right),
        ComparisonOperator::LessThanOrEqual => {
            compare_numbers(left, right, |left, right| left <= right)
        }
        ComparisonOperator::Matches => {
            let regex = Regex::new(&glob_pattern_to_regex(&right.as_string()))
                .map_err(|e| rule_validation_error(format!("Invalid match pattern: {}", e)))?;
            Ok(regex.is_match(&left.as_string()))
        }
    }
}

fn values_equal(left: &RuleValue, right: &RuleValue) -> bool {
    match (left, right) {
        (RuleValue::Number(left), RuleValue::Number(right)) => left == right,
        (RuleValue::Boolean(left), RuleValue::Boolean(right)) => left == right,
        _ => left.as_string() == right.as_string(),
    }
}

fn compare_numbers<F>(left: RuleValue, right: RuleValue, compare: F) -> Result<bool, AppError>
where
    F: FnOnce(f64, f64) -> bool,
{
    let left = left.as_number().ok_or_else(|| {
        rule_validation_error("Numeric comparison requires numeric left value".to_string())
    })?;
    let right = right.as_number().ok_or_else(|| {
        rule_validation_error("Numeric comparison requires numeric right value".to_string())
    })?;

    Ok(compare(left, right))
}

fn glob_pattern_to_regex(pattern: &str) -> String {
    let mut regex = String::from("^");

    for ch in pattern.chars() {
        match ch {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            _ => regex.push_str(&regex::escape(&ch.to_string())),
        }
    }

    regex.push('$');
    regex
}

fn rule_validation_error(message: String) -> AppError {
    AppError::validation("rule".to_string(), message)
}
