use osom_config::{FieldDef, FieldType, OutputSchema};
use serde_json::Value;
use thiserror::Error;

/// Błąd walidacji schematu.
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Brak wymaganego pola: {0}")]
    MissingField(String),
    #[error("Niezgodność typu dla pola {0}: oczekiwano {1}, otrzymano {2}")]
    TypeMismatch(String, String, String),
    #[error("Nieprawidłowe elementy tablicy dla pola {0}")]
    InvalidArray(String),
    #[error("Nieznane pole: {0}")]
    UnknownField(String),
    #[error("Oczekiwano obiektu lub tablicy, otrzymano: {0}")]
    NotObjectOrArray(String),
}

/// Waliduje surową odpowiedź JSON pod kątem zgodności ze schematem.
pub fn validate(value: &Value, schema: &OutputSchema) -> Result<(), ValidationError> {
    match value {
        Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                validate_object(item, &schema.fields)
                    .map_err(|e| ValidationError::InvalidArray(format!("index {}: {}", i, e)))?;
            }
        }
        Value::Object(_) => {
            validate_object(value, &schema.fields)?;
        }
        other => {
            return Err(ValidationError::NotObjectOrArray(format!("{:?}", other)));
        }
    }
    Ok(())
}

fn validate_object(value: &Value, fields: &[FieldDef]) -> Result<(), ValidationError> {
    let obj = match value {
        Value::Object(o) => o,
        other => {
            return Err(ValidationError::TypeMismatch(
                "root".to_string(),
                "object".to_string(),
                format!("{:?}", other),
            ));
        }
    };

    for field in fields {
        let val = obj.get(&field.name);
        if field.required && val.is_none() {
            return Err(ValidationError::MissingField(field.name.clone()));
        }
        if let Some(v) = val {
            validate_type(&field.name, v, &field.field_type)?;
        }
    }

    Ok(())
}

fn validate_type(name: &str, value: &Value, field_type: &FieldType) -> Result<(), ValidationError> {
    match field_type {
        FieldType::String => {
            if !value.is_string() && !value.is_null() {
                return Err(ValidationError::TypeMismatch(
                    name.to_string(),
                    "string".to_string(),
                    format!("{:?}", value),
                ));
            }
        }
        FieldType::Integer => {
            if !value.is_i64() && !value.is_u64() && !value.is_null() {
                return Err(ValidationError::TypeMismatch(
                    name.to_string(),
                    "integer".to_string(),
                    format!("{:?}", value),
                ));
            }
        }
        FieldType::Decimal { .. } => {
            if !value.is_number() && !value.is_null() {
                return Err(ValidationError::TypeMismatch(
                    name.to_string(),
                    "decimal".to_string(),
                    format!("{:?}", value),
                ));
            }
        }
        FieldType::Boolean => {
            if !value.is_boolean() && !value.is_null() {
                return Err(ValidationError::TypeMismatch(
                    name.to_string(),
                    "boolean".to_string(),
                    format!("{:?}", value),
                ));
            }
        }
        FieldType::Date { .. } | FieldType::DateTime { .. } => {
            if !value.is_string() && !value.is_null() {
                return Err(ValidationError::TypeMismatch(
                    name.to_string(),
                    "date/datetime string".to_string(),
                    format!("{:?}", value),
                ));
            }
        }
        FieldType::Object { fields } => {
            validate_object(value, fields)?;
        }
        FieldType::Array { item_type } => match value {
            Value::Array(arr) => {
                for item in arr {
                    validate_type(name, item, item_type)?;
                }
            }
            Value::Null => {}
            other => {
                return Err(ValidationError::TypeMismatch(
                    name.to_string(),
                    "array".to_string(),
                    format!("{:?}", other),
                ));
            }
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use osom_config::{FieldDef, FieldType, OutputSchema};
    use serde_json::json;

    #[test]
    fn test_validate_simple_object() {
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
        let value = json!({"id": 42});
        assert!(validate(&value, &schema).is_ok());
    }

    #[test]
    fn test_validate_missing_required() {
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
        let value = json!({"name": "test"});
        assert!(matches!(
            validate(&value, &schema),
            Err(ValidationError::MissingField(_))
        ));
    }

    #[test]
    fn test_validate_array() {
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
        let value = json!([{"id": 1}, {"id": 2}]);
        assert!(validate(&value, &schema).is_ok());
    }

    #[test]
    fn test_validate_type_mismatch() {
        let schema = OutputSchema {
            name: "Test".to_string(),
            fields: vec![FieldDef {
                name: "active".to_string(),
                field_type: FieldType::Boolean,
                required: true,
                description: "".to_string(),
                default: None,
            }],
            case_normalization: None,
        };
        let value = json!({"active": "yes"});
        assert!(matches!(
            validate(&value, &schema),
            Err(ValidationError::TypeMismatch(_, _, _))
        ));
    }
}
