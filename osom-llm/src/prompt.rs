use osom_config::{FieldDef, FieldType, OutputSchema};

/// Buduje prompt LLM na podstawie danych wejściowych i schematu wyjściowego
pub struct PromptBuilder;

impl PromptBuilder {
    /// Tworzy pełny prompt z danych wejściowych i schematu wyjściowego
    ///
    /// # Arguments
    /// * `input` — Tekstowe dane wejściowe do przetworzenia
    /// * `schema` — Schemat wyjściowy definiujący oczekiwaną strukturę
    ///
    /// # Returns
    /// Gotowy prompt do wysłania do modelu LLM
    pub fn build(input: &str, schema: &OutputSchema) -> String {
        let mut prompt = format!(
            "Przetwórz poniższe dane wejściowe zgodnie ze schematem '{}'.\n\n",
            schema.name
        );

        prompt.push_str("## Dane wejściowe\n");
        prompt.push_str(input);
        prompt.push_str("\n\n");

        prompt.push_str("## Schemat wyjściowy\n");
        prompt.push_str("Zwróć wynik jako JSON z następującymi polami:\n\n");

        for field in &schema.fields {
            let desc = Self::describe_field(field);
            prompt.push_str(&format!("- {}: {}\n", field.name, desc));
        }

        prompt.push_str("\n## Instrukcje\n");
        prompt.push_str(
            "1. Zwróć TYLKO obiekt JSON lub tablicę obiektów JSON bez żadnych dodatkowych komentarzy, markdown lub wyjaśnień.\n",
        );
        prompt.push_str("2. Nie używaj bloków kodu (```json).\n");
        prompt.push_str(
            "3. Jeśli dane wejściowe nie zawierają informacji dla wymaganego pola, użyj wartości null.\n",
        );
        prompt.push_str("4. Upewnij się, że typy danych są zgodne ze schematem.\n");
        prompt.push_str("5. Jeśli dane wejściowe zawierają wiele rekordów, zwróć tablicę obiektów JSON.\n");

        if let Some(ref norm) = schema.case_normalization {
            prompt.push_str(&format!(
                "6. Wartości tekstowe powinny być znormalizowane do formatu: {:?}.\n",
                format!("{:?}", norm).to_lowercase()
            ));
        }

        prompt
    }

    /// Tworzy uproszczony prompt bez schematu
    pub fn build_simple(input: &str, instruction: &str) -> String {
        format!(
            "{}\n\nDane wejściowe:\n{}\n\nZwróć wynik jako czysty tekst bez formatowania markdown.",
            instruction, input
        )
    }

    fn describe_field(field: &FieldDef) -> String {
        let required = if field.required {
            " (wymagane)"
        } else {
            " (opcjonalne)"
        };
        let type_desc = match &field.field_type {
            FieldType::String => "tekst".to_string(),
            FieldType::Integer => "liczba całkowita".to_string(),
            FieldType::Decimal { precision, scale } => format!(
                "liczba dziesiętna (precyzja: {}, skala: {})",
                precision, scale
            ),
            FieldType::Boolean => "wartość logiczna (true/false)".to_string(),
            FieldType::Date { format } => format!("data (format: {})", format),
            FieldType::DateTime { format } => format!("data i czas (format: {})", format),
            FieldType::Object { fields } => {
                format!("obiekt zagnieżdżony z {} polami", fields.len())
            }
            FieldType::Array { item_type } => {
                format!("tablica elementów typu {}", Self::describe_type_compact(item_type))
            }
        };

        let mut desc = format!("{}{} — {}", field.name, required, type_desc);
        if !field.description.is_empty() {
            desc.push_str(&format!(" ({})", field.description));
        }
        if let Some(ref default) = field.default {
            desc.push_str(&format!(" [domyślnie: {}]", default));
        }
        desc
    }

    fn describe_type_compact(field_type: &FieldType) -> String {
        match field_type {
            FieldType::String => "tekst".to_string(),
            FieldType::Integer => "liczba_całkowita".to_string(),
            FieldType::Decimal { .. } => "liczba_dziesiętna".to_string(),
            FieldType::Boolean => "boolean".to_string(),
            FieldType::Date { .. } => "data".to_string(),
            FieldType::DateTime { .. } => "datetime".to_string(),
            FieldType::Object { .. } => "obiekt".to_string(),
            FieldType::Array { .. } => "tablica".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osom_config::{CaseNormalization, FieldDef, FieldType, OutputSchema};

    fn create_test_schema() -> OutputSchema {
        OutputSchema {
            name: "TestSchema".to_string(),
            fields: vec![
                FieldDef {
                    name: "name".to_string(),
                    field_type: FieldType::String,
                    required: true,
                    description: "Imię osoby".to_string(),
                    default: None,
                },
                FieldDef {
                    name: "age".to_string(),
                    field_type: FieldType::Integer,
                    required: false,
                    description: String::new(),
                    default: Some(serde_json::json!(0)),
                },
            ],
            case_normalization: None,
        }
    }

    #[test]
    fn test_build_prompt_contains_schema_name() {
        let schema = create_test_schema();
        let prompt = PromptBuilder::build("Jan Kowalski, 30 lat", &schema);
        assert!(prompt.contains("TestSchema"));
    }

    #[test]
    fn test_build_prompt_contains_fields() {
        let schema = create_test_schema();
        let prompt = PromptBuilder::build("Jan Kowalski, 30 lat", &schema);
        assert!(prompt.contains("name"));
        assert!(prompt.contains("age"));
        assert!(prompt.contains("wymagane"));
        assert!(prompt.contains("opcjonalne"));
    }

    #[test]
    fn test_build_prompt_contains_input() {
        let input = "Testowe dane wejściowe";
        let schema = create_test_schema();
        let prompt = PromptBuilder::build(input, &schema);
        assert!(prompt.contains(input));
    }

    #[test]
    fn test_build_prompt_json_instruction() {
        let schema = create_test_schema();
        let prompt = PromptBuilder::build("test", &schema);
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn test_build_simple_prompt() {
        let prompt = PromptBuilder::build_simple("dane", "przetwórz to");
        assert!(prompt.contains("dane"));
        assert!(prompt.contains("przetwórz to"));
    }

    #[test]
    fn test_describe_field_decimal() {
        let field = FieldDef {
            name: "price".to_string(),
            field_type: FieldType::Decimal {
                precision: 10,
                scale: 2,
            },
            required: true,
            description: "Cena produktu".to_string(),
            default: None,
        };
        let desc = PromptBuilder::describe_field(&field);
        assert!(desc.contains("liczba dziesiętna"));
        assert!(desc.contains("precyzja: 10"));
        assert!(desc.contains("skala: 2"));
    }

    #[test]
    fn test_describe_field_date() {
        let field = FieldDef {
            name: "created_at".to_string(),
            field_type: FieldType::Date {
                format: "%Y-%m-%d".to_string(),
            },
            required: true,
            description: String::new(),
            default: None,
        };
        let desc = PromptBuilder::describe_field(&field);
        assert!(desc.contains("data"));
        assert!(desc.contains("%Y-%m-%d"));
    }

    #[test]
    fn test_build_prompt_with_case_normalization() {
        let schema = OutputSchema {
            name: "NormSchema".to_string(),
            fields: vec![FieldDef {
                name: "title".to_string(),
                field_type: FieldType::String,
                required: true,
                description: String::new(),
                default: None,
            }],
            case_normalization: Some(CaseNormalization::Snake),
        };
        let prompt = PromptBuilder::build("test", &schema);
        assert!(prompt.contains("snake"));
    }

    #[test]
    fn test_describe_field_array() {
        let field = FieldDef {
            name: "tags".to_string(),
            field_type: FieldType::Array {
                item_type: Box::new(FieldType::String),
            },
            required: false,
            description: "Etykiety".to_string(),
            default: None,
        };
        let desc = PromptBuilder::describe_field(&field);
        assert!(desc.contains("tablica"));
        assert!(desc.contains("tekst"));
        assert!(desc.contains("Etykiety"));
    }

    #[test]
    fn test_describe_field_object() {
        let field = FieldDef {
            name: "address".to_string(),
            field_type: FieldType::Object {
                fields: vec![
                    FieldDef {
                        name: "street".to_string(),
                        field_type: FieldType::String,
                        required: true,
                        description: String::new(),
                        default: None,
                    },
                    FieldDef {
                        name: "city".to_string(),
                        field_type: FieldType::String,
                        required: true,
                        description: String::new(),
                        default: None,
                    },
                ],
            },
            required: true,
            description: String::new(),
            default: None,
        };
        let desc = PromptBuilder::describe_field(&field);
        assert!(desc.contains("obiekt zagnieżdżony"));
        assert!(desc.contains("2 polami"));
    }
}
