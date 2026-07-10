use crate::config::{CaseNormalization, FieldDef, FieldType, OutputSchema};

/// Normalizes the case of a string value according to the schema setting.
pub fn normalize_case(value: &str, normalization: &Option<CaseNormalization>) -> String {
    match normalization {
        None => value.to_string(),
        Some(CaseNormalization::Lower) => value.to_lowercase(),
        Some(CaseNormalization::Upper) => value.to_uppercase(),
        Some(CaseNormalization::Title) => {
            let mut result = String::new();
            let mut new_word = true;
            for c in value.chars() {
                if new_word && c.is_alphabetic() {
                    result.push(c.to_ascii_uppercase());
                    new_word = false;
                } else {
                    result.push(c);
                    if c.is_whitespace() {
                        new_word = true;
                    }
                }
            }
            result
        }
        Some(CaseNormalization::Snake) => {
            value
                .replace(" ", "_")
                .replace("-", "_")
                .to_lowercase()
        }
        Some(CaseNormalization::Camel) => {
            let snake = value.replace(" ", "_").replace("-", "_").to_lowercase();
            let parts: Vec<&str> = snake.split('_').collect();
            let mut result = String::new();
            for (i, part) in parts.iter().enumerate() {
                if i == 0 {
                    result.push_str(part);
                } else {
                    let mut chars = part.chars();
                    if let Some(first) = chars.next() {
                        result.push(first.to_ascii_uppercase());
                        result.extend(chars);
                    }
                }
            }
            result
        }
        Some(CaseNormalization::Pascal) => {
            let snake = value.replace(" ", "_").replace("-", "_").to_lowercase();
            let parts: Vec<&str> = snake.split('_').collect();
            let mut result = String::new();
            for part in parts {
                let mut chars = part.chars();
                if let Some(first) = chars.next() {
                    result.push(first.to_ascii_uppercase());
                    result.extend(chars);
                }
            }
            result
        }
    }
}
