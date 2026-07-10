use osom_config::{CaseNormalization, FieldDef, FieldType, OutputSchema};
use osom_config::schema::normalize_case;
use serde_json::{Map, Value};
use thiserror::Error;

/// Błąd formatowania danych.
#[derive(Debug, Error)]
pub enum FormatError {
    #[error("Błąd typu: {0}")]
    Type(String),
    #[error("Błąd walidacji: {0}")]
    Validation(String),
}

/// Sformatowane dane gotowe do zapisu.
#[derive(Debug, Clone)]
pub struct FormattedData {
    /// Rekordy jako wektor map klucz-wartość.
    pub records: Vec<Map<String, Value>>,
}

/// Formatuje surową odpowiedź JSON zgodnie ze schematem wyjściowym.
pub fn format_values(raw: &Value, schema: &OutputSchema) -> Result<FormattedData, FormatError> {
    let records = match raw {
        Value::Array(arr) => arr
            .iter()
            .map(|item| format_object(item, &schema.fields, &schema.case_normalization))
            .collect::<Result<Vec<_>, _>>()?,
        Value::Object(_) => {
            vec![format_object(raw, &schema.fields, &schema.case_normalization)?]
        }
        other => {
            return Err(FormatError::Type(format!(
                "Oczekiwano obiektu lub tablicy, otrzymano: {:?}",
                other
            )));
        }
    };
    Ok(FormattedData { records })
}

fn format_object(
    value: &Value,
    fields: &[FieldDef],
    case_norm: &Option<CaseNormalization>,
) -> Result<Map<String, Value>, FormatError> {
    let obj = match value {
        Value::Object(o) => o,
        other => {
            return Err(FormatError::Type(format!(
                "Oczekiwano obiektu, otrzymano: {:?}",
                other
            )));
        }
    };

    let mut result = Map::new();
    for field in fields {
        let val = obj.get(&field.name).cloned().unwrap_or_else(|| {
            field.default.clone().unwrap_or(Value::Null)
        });
        let formatted = format_value(&val, &field.field_type, case_norm)?;
        result.insert(field.name.clone(), formatted);
    }
    Ok(result)
}

fn format_value(
    value: &Value,
    field_type: &FieldType,
    case_norm: &Option<CaseNormalization>,
) -> Result<Value, FormatError> {
    match field_type {
        FieldType::String => match value {
            Value::String(s) => Ok(Value::String(normalize_case(s, case_norm))),
            Value::Null => Ok(Value::Null),
            other => Ok(Value::String(other.to_string())),
        },
        FieldType::Integer => match value {
            Value::Number(n) if n.is_i64() || n.is_u64() => Ok(value.clone()),
            Value::Null => Ok(Value::Null),
            other => Err(FormatError::Type(format!(
                "Oczekiwano integer, otrzymano {}",
                other
            ))),
        },
        FieldType::Decimal { .. } => match value {
            Value::Number(_) => Ok(value.clone()),
            Value::Null => Ok(Value::Null),
            other => Err(FormatError::Type(format!(
                "Oczekiwano decimal, otrzymano {}",
                other
            ))),
        },
        FieldType::Boolean => match value {
            Value::Bool(_) => Ok(value.clone()),
            Value::Null => Ok(Value::Null),
            other => Err(FormatError::Type(format!(
                "Oczekiwano boolean, otrzymano {}",
                other
            ))),
        },
        FieldType::Date { .. } | FieldType::DateTime { .. } => match value {
            Value::String(_) => Ok(value.clone()),
            Value::Null => Ok(Value::Null),
            other => Err(FormatError::Type(format!(
                "Oczekiwano string daty, otrzymano {}",
                other
            ))),
        },
        FieldType::Object { fields } => {
            format_object(value, fields, case_norm).map(Value::Object)
        }
        FieldType::Array { item_type } => match value {
            Value::Array(arr) => {
                let mut result = Vec::new();
                for item in arr {
                    result.push(format_value(item, item_type, case_norm)?);
                }
                Ok(Value::Array(result))
            }
            Value::Null => Ok(Value::Null),
            other => Err(FormatError::Type(format!(
                "Oczekiwano tablicy, otrzymano {}",
                other
            ))),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osom_config::{FieldDef, FieldType, OutputSchema};
    use serde_json::json;

    #[test]
    fn test_format_simple_object() {
        let schema = OutputSchema {
            name: "Test".to_string(),
            fields: vec![FieldDef {
                name: "name".to_string(),
                field_type: FieldType::String,
                required: true,
                description: "".to_string(),
                default: None,
            }],
            case_normalization: Some(CaseNormalization::Upper),
        };
        let raw = json!({"name": "hello"});
        let formatted = format_values(&raw, &schema).unwrap();
        assert_eq!(formatted.records[0]["name"], "HELLO");
    }

    #[test]
    fn test_format_with_default() {
        let schema = OutputSchema {
            name: "Test".to_string(),
            fields: vec![FieldDef {
                name: "status".to_string(),
                field_type: FieldType::String,
                required: false,
                description: "".to_string(),
                default: Some(json!("pending")),
            }],
            case_normalization: None,
        };
        let raw = json!({});
        let formatted = format_values(&raw, &schema).unwrap();
        assert_eq!(formatted.records[0]["status"], "pending");
    }

    #[test]
    fn test_format_array() {
        let schema = OutputSchema {
            name: "Test".to_string(),
            fields: vec![FieldDef {
                name: "id".to_string(),
                field_type: FieldType::Integer,
                required: true,
                description: "".to_string(),
                default: None,
            }],
            case_normalization: None,
        };
        let raw = json!([{"id": 1}, {"id": 2}]);
        let formatted = format_values(&raw, &schema).unwrap();
        assert_eq!(formatted.records.len(), 2);
    }

    #[test]
    fn test_format_nested_object() {
        let schema = OutputSchema {
            name: "Test".to_string(),
            fields: vec![FieldDef {
                name: "user".to_string(),
                field_type: FieldType::Object {
                    fields: vec![FieldDef {
                        name: "name".to_string(),
                        field_type: FieldType::String,
                        required: true,
                        description: "".to_string(),
                        default: None,
                    }],
                },
                required: true,
                description: "".to_string(),
                default: None,
            }],
            case_normalization: None,
        };
        let raw = json!({"user": {"name": "Alice"}});
        let formatted = format_values(&raw, &schema).unwrap();
        assert_eq!(formatted.records[0]["user"]["name"], "Alice");
    }
}
