//! Relationship operations for SQLite database
//!
//! This module implements relationship population and related record fetching
//! for the SQLite database backend.

use super::connection::SqliteDb;
use crate::{Record, db::SchemaAdapter};
use oxide_core::{AppError, FieldType, field_types::RelationshipConfig};
use tokio::task::spawn_blocking;
use tracing::{debug, warn};
use serde_json::{Value as JsonValue, Map};
use std::collections::{HashMap, HashSet};

impl SqliteDb {
    /// Populate relationship fields in records
    ///
    /// This method takes a list of records and populates their relationship fields
    /// by fetching the related records from the target collections.
    pub async fn populate_relationships(&self, collection: &str, records: &mut [Record]) -> Result<(), AppError> {
        if records.is_empty() {
            return Ok(());
        }

        debug!("Populating relationships for {} records in collection '{}'", records.len(), collection);

        // Get the collection schema to find relationship fields
        let schema = self.get_collection_schema(collection).await?;
        
        // Find all relationship fields in the schema
        let relationship_fields: Vec<(String, RelationshipConfig)> = schema.fields
            .iter()
            .filter_map(|(field_name, field_def)| {
                if let FieldType::Relationship(config) = &field_def.field_type {
                    Some((field_name.clone(), config.clone()))
                } else {
                    None
                }
            })
            .collect();

        if relationship_fields.is_empty() {
            debug!("No relationship fields found in collection '{}'", collection);
            return Ok(());
        }

        // For each relationship field, collect all unique IDs and fetch related records
        for (field_name, config) in relationship_fields {
            debug!("Populating relationship field '{}' -> '{}'", field_name, config.target_collection);
            
            // Collect all unique record IDs from this field across all records
            let mut all_ids = HashSet::new();
            
            for record in records.iter() {
                if let Some(field_value) = record.data.get(&field_name) {
                    match field_value {
                        JsonValue::String(id) if !id.is_empty() => {
                            all_ids.insert(id.clone());
                        }
                        JsonValue::Array(ids) => {
                            for id_value in ids {
                                if let JsonValue::String(id) = id_value {
                                    if !id.is_empty() {
                                        all_ids.insert(id.clone());
                                    }
                                }
                            }
                        }
                        JsonValue::Null => {
                            // Skip null values
                        }
                        _ => {
                            warn!("Invalid relationship field value in record {}: expected string or array, got {:?}", 
                                  record.id, field_value);
                        }
                    }
                }
            }

            if all_ids.is_empty() {
                debug!("No relationship IDs found for field '{}'", field_name);
                continue;
            }

            // Fetch all related records
            let ids_vec: Vec<String> = all_ids.into_iter().collect();
            let related_records = self.get_related_records(
                &config.target_collection, 
                &ids_vec, 
                config.display_field.as_deref()
            ).await?;

            // Now populate the relationship data in each record
            for record in records.iter_mut() {
                if let Some(field_value) = record.data.get(&field_name).cloned() {
                    let populated_value = match field_value {
                        JsonValue::String(id) if !id.is_empty() => {
                            // Single relationship
                            if let Some(related_data) = related_records.get(&id) {
                                related_data.clone()
                            } else {
                                JsonValue::Null // Related record not found
                            }
                        }
                        JsonValue::Array(ids) => {
                            // Multiple relationship
                            let populated_array: Vec<JsonValue> = ids.iter()
                                .filter_map(|id_value| {
                                    if let JsonValue::String(id) = id_value {
                                        related_records.get(id).cloned()
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            JsonValue::Array(populated_array)
                        }
                        _ => field_value, // Keep original value for invalid types
                    };

                    // Update the record data with populated relationship
                    if let JsonValue::Object(ref mut obj) = record.data {
                        obj.insert(format!("{}_populated", field_name), populated_value);
                    }
                }
            }
        }

        debug!("Successfully populated relationships for {} records", records.len());
        Ok(())
    }

    /// Populate specific relationship fields in records
    ///
    /// This method takes a list of records and populates only the specified relationship fields
    /// by fetching the related records from the target collections.
    pub async fn populate_specific_relationships(&self, collection: &str, records: &mut [Record], field_names: &[String]) -> Result<(), AppError> {
        if records.is_empty() || field_names.is_empty() {
            return Ok(());
        }

        debug!("Populating specific relationships for {} records in collection '{}', fields: {:?}", records.len(), collection, field_names);

        // Get the collection schema to find relationship fields
        let schema = self.get_collection_schema(collection).await?;
        
        // Find only the specified relationship fields in the schema
        let relationship_fields: Vec<(String, RelationshipConfig)> = schema.fields
            .iter()
            .filter_map(|(field_name, field_def)| {
                if field_names.contains(field_name) {
                    if let FieldType::Relationship(config) = &field_def.field_type {
                        Some((field_name.clone(), config.clone()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        if relationship_fields.is_empty() {
            debug!("No matching relationship fields found in collection '{}' for fields: {:?}", collection, field_names);
            return Ok(());
        }

        // For each relationship field, collect all unique IDs and fetch related records
        for (field_name, config) in relationship_fields {
            debug!("Populating relationship field '{}' -> '{}'", field_name, config.target_collection);
            
            // Collect all unique record IDs from this field across all records
            let mut all_ids = HashSet::new();
            
            for record in records.iter() {
                if let Some(field_value) = record.data.get(&field_name) {
                    match field_value {
                        JsonValue::String(id) if !id.is_empty() => {
                            all_ids.insert(id.clone());
                        }
                        JsonValue::Array(ids) => {
                            for id_value in ids {
                                if let JsonValue::String(id) = id_value {
                                    if !id.is_empty() {
                                        all_ids.insert(id.clone());
                                    }
                                }
                            }
                        }
                        JsonValue::Null => {
                            // Skip null values
                        }
                        _ => {
                            warn!("Invalid relationship field value in record {}: expected string or array, got {:?}", 
                                  record.id, field_value);
                        }
                    }
                }
            }

            if all_ids.is_empty() {
                debug!("No relationship IDs found for field '{}'", field_name);
                continue;
            }

            // Fetch all related records
            let ids_vec: Vec<String> = all_ids.into_iter().collect();
            let related_records = self.get_related_records(
                &config.target_collection, 
                &ids_vec, 
                config.display_field.as_deref()
            ).await?;

            // Now populate the relationship data in each record
            for record in records.iter_mut() {
                if let Some(field_value) = record.data.get(&field_name).cloned() {
                    let populated_value = match field_value {
                        JsonValue::String(id) if !id.is_empty() => {
                            // Single relationship
                            if let Some(related_data) = related_records.get(&id) {
                                related_data.clone()
                            } else {
                                JsonValue::Null // Related record not found
                            }
                        }
                        JsonValue::Array(ids) => {
                            // Multiple relationship
                            let populated_array: Vec<JsonValue> = ids.iter()
                                .filter_map(|id_value| {
                                    if let JsonValue::String(id) = id_value {
                                        related_records.get(id).cloned()
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            JsonValue::Array(populated_array)
                        }
                        _ => field_value, // Keep original value for invalid types
                    };

                    // Update the record data with populated relationship
                    if let JsonValue::Object(ref mut obj) = record.data {
                        obj.insert(format!("{}_populated", field_name), populated_value);
                    }
                }
            }
        }

        debug!("Successfully populated specific relationships for {} records", records.len());
        Ok(())
    }

    /// Get related records for a specific relationship field
    ///
    /// This method retrieves related records for a specific relationship field value.
    pub async fn get_related_records(
        &self, 
        target_collection: &str, 
        record_ids: &[String],
        display_field: Option<&str>
    ) -> Result<HashMap<String, JsonValue>, AppError> {
        if record_ids.is_empty() {
            return Ok(HashMap::new());
        }

        debug!("Fetching {} related records from collection '{}'", record_ids.len(), target_collection);

        // Get the target collection schema
        let target_schema = self.get_collection_schema(target_collection).await?;
        let schema_adapter = super::schema_adapter::SqliteSchemaAdapter::new();
        let table_name = schema_adapter.get_table_name(&target_schema.name);
        
        let record_ids = record_ids.to_vec();
        let connection = self.connection.clone();
        let target_schema_clone = target_schema.clone();
        let display_field = display_field.map(|s| s.to_string());

        let related_records = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            // Build the SQL query with IN clause
            let placeholders = record_ids.iter().enumerate()
                .map(|(i, _)| format!("?{}", i + 1))
                .collect::<Vec<_>>()
                .join(", ");
            
            let query = format!("SELECT * FROM {} WHERE id IN ({})", table_name, placeholders);
            
            let mut stmt = conn.prepare(&query)
                .map_err(|e| AppError::database(format!("Failed to prepare query: {}", e)))?;

            // Bind the record IDs
            for (index, id) in record_ids.iter().enumerate() {
                stmt.raw_bind_parameter(index + 1, id)
                    .map_err(|e| AppError::database(format!("Failed to bind parameter: {}", e)))?;
            }

            let mut results = HashMap::new();
            let rows = stmt.query_map([], |row| {
                // Convert the SQLite row to a Record object
                let mut data_map = Map::new();
                
                // Convert all fields according to the schema
                for (field_name, field_def) in &target_schema_clone.fields {
                    let value = match field_def.field_type.sql_type() {
                        "TEXT" => {
                            if let Ok(Some(text_val)) = row.get::<_, Option<String>>(field_name.as_str()) {
                                JsonValue::String(text_val)
                            } else {
                                JsonValue::Null
                            }
                        }
                        "REAL" => {
                            if let Ok(Some(real_val)) = row.get::<_, Option<f64>>(field_name.as_str()) {
                                JsonValue::Number(
                                    serde_json::Number::from_f64(real_val).unwrap_or_else(|| serde_json::Number::from(0))
                                )
                            } else {
                                JsonValue::Null
                            }
                        }
                        "INTEGER" => {
                            if let Ok(Some(int_val)) = row.get::<_, Option<i64>>(field_name.as_str()) {
                                // For boolean fields stored as INTEGER, convert back to boolean
                                if matches!(field_def.field_type, FieldType::Boolean) {
                                    JsonValue::Bool(int_val != 0)
                                } else {
                                    JsonValue::Number(serde_json::Number::from(int_val))
                                }
                            } else {
                                JsonValue::Null
                            }
                        }
                        _ => JsonValue::Null, // Fallback
                    };
                    data_map.insert(field_name.clone(), value);
                }

                // Add metadata fields
                let record_id: String = row.get("id")?;
                let created_at: i64 = row.get("created_at")?;
                let updated_at: i64 = row.get("updated_at")?;
                
                data_map.insert("id".to_string(), JsonValue::String(record_id.clone()));
                data_map.insert("created_at".to_string(), JsonValue::Number(serde_json::Number::from(created_at)));
                data_map.insert("updated_at".to_string(), JsonValue::Number(serde_json::Number::from(updated_at)));

                Ok((record_id, JsonValue::Object(data_map)))
            })
            .map_err(|e| AppError::database(format!("Failed to execute query: {}", e)))?;

            for row_result in rows {
                match row_result {
                    Ok((id, record_data)) => {
                        // If a display field is specified, create a simplified representation
                        if let Some(ref display_field_name) = display_field {
                            if let JsonValue::Object(ref obj) = record_data {
                                if let Some(display_value) = obj.get(display_field_name) {
                                    // Create a simplified object with id and display field
                                    let mut simplified = Map::new();
                                    simplified.insert("id".to_string(), JsonValue::String(id.clone()));
                                    simplified.insert("display".to_string(), display_value.clone());
                                    results.insert(id, JsonValue::Object(simplified));
                                } else {
                                    // Display field not found, use full record
                                    results.insert(id, record_data);
                                }
                            } else {
                                results.insert(id, record_data);
                            }
                        } else {
                            // No display field specified, use full record
                            results.insert(id, record_data);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to process related record: {}", e);
                    }
                }
            }

            Ok::<HashMap<String, JsonValue>, AppError>(results)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        debug!("Successfully fetched {} related records", related_records.len());
        Ok(related_records)
    }
} 