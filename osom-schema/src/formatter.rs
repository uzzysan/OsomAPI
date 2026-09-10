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
        FieldType::Decimal { scale, .. } => match value {
            Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    let multiplier = 10f64.powi(*scale as i32);
                    let rounded = (f * multiplier).round() / multiplier;
                    let rounded_val = serde_json::Number::from_f64(rounded)
                        .map(Value::Number)
                        .unwrap_or_else(|| value.clone());
                    Ok(rounded_val)
                } else {
                    Ok(value.clone())
                }
            }
            Value::String(s) => {
                if let Ok(f) = s.trim().parse::<f64>() {
                    let multiplier = 10f64.powi(*scale as i32);
                    let rounded = (f * multiplier).round() / multiplier;
                    serde_json::Number::from_f64(rounded)
                        .map(Value::Number)
                        .ok_or_else(|| FormatError::Type("Niepoprawna liczba dziesiętna".into()))
                } else {
                    Err(FormatError::Type(format!(
                        "Nie można sparsować jako decimal: {}",
                        s
                    )))
                }
            }
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
        FieldType::Date { format } => match value {
            Value::String(s) => {
                let formatted_date = parse_and_format_date(s, format)?;
                Ok(Value::String(formatted_date))
            }
            Value::Null => Ok(Value::Null),
            other => Err(FormatError::Type(format!(
                "Oczekiwano string daty, otrzymano {}",
                other
            ))),
        },
        FieldType::DateTime { format } => match value {
            Value::String(s) => {
                let formatted_dt = parse_and_format_datetime(s, format)?;
                Ok(Value::String(formatted_dt))
            }
            Value::Null => Ok(Value::Null),
            other => Err(FormatError::Type(format!(
                "Oczekiwano string daty i czasu, otrzymano {}",
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

fn parse_and_format_date(s: &str, target_format: &str) -> Result<String, FormatError> {
    if let Ok(date) = chrono::NaiveDate::parse_from_str(s, target_format) {
        return Ok(date.format(target_format).to_string());
    }
    let formats = ["%Y-%m-%d", "%d-%m-%Y", "%d.%m.%Y", "%Y/%m/%d", "%m/%d/%Y"];
    for fmt in formats {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(s, fmt) {
            return Ok(date.format(target_format).to_string());
        }
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Ok(dt.date_naive().format(target_format).to_string());
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Ok(dt.date().format(target_format).to_string());
    }
    Err(FormatError::Type(format!(
        "Nie można sparsować daty '{}' do formatu '{}'",
        s, target_format
    )))
}

fn parse_and_format_datetime(s: &str, target_format: &str) -> Result<String, FormatError> {
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, target_format) {
        return Ok(dt.format(target_format).to_string());
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Ok(dt.naive_utc().format(target_format).to_string());
    }
    let formats = [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M:%S",
    ];
    for fmt in formats {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            return Ok(dt.format(target_format).to_string());
        }
    }
    Err(FormatError::Type(format!(
        "Nie można sparsować daty i czasu '{}' do formatu '{}'",
        s, target_format
    )))
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

    #[test]
    fn test_format_decimal_rounding() {
        let schema = OutputSchema {
            name: "Test".to_string(),
            fields: vec![FieldDef {
                name: "price".to_string(),
                field_type: FieldType::Decimal {
                    precision: 10,
                    scale: 2,
                },
                required: true,
                description: "".to_string(),
                default: None,
            }],
            case_normalization: None,
        };
        let raw = json!({"price": 12.34567});
        let formatted = format_values(&raw, &schema).unwrap();
        assert_eq!(formatted.records[0]["price"], 12.35);

        // From string representation
        let raw_str = json!({"price": "99.999"});
        let formatted_str = format_values(&raw_str, &schema).unwrap();
        assert_eq!(formatted_str.records[0]["price"], 100.0);
    }

    #[test]
    fn test_format_date_normalization() {
        let schema = OutputSchema {
            name: "Test".to_string(),
            fields: vec![FieldDef {
                name: "date".to_string(),
                field_type: FieldType::Date {
                    format: "%Y-%m-%d".to_string(),
                },
                required: true,
                description: "".to_string(),
                default: None,
            }],
            case_normalization: None,
        };
        // Standard ISO date
        let raw1 = json!({"date": "2024-05-20"});
        let formatted1 = format_values(&raw1, &schema).unwrap();
        assert_eq!(formatted1.records[0]["date"], "2024-05-20");

        // European date format normalized to %Y-%m-%d
        let raw2 = json!({"date": "20-05-2024"});
        let formatted2 = format_values(&raw2, &schema).unwrap();
        assert_eq!(formatted2.records[0]["date"], "2024-05-20");

        // RFC3339 datetime normalized to date
        let raw3 = json!({"date": "2024-05-20T14:30:00Z"});
        let formatted3 = format_values(&raw3, &schema).unwrap();
        assert_eq!(formatted3.records[0]["date"], "2024-05-20");
    }
}
