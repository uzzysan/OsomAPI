use osom_api::pipeline::{run_pipeline_with_options, PipelineOptions};
use osom_config::{Config, Destination, FieldDef, FieldType, LlmConfig, OutputConfig, OutputSchema};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Helper function to start a local mock HTTP server that simulates an Ollama API endpoint.
async fn start_mock_ollama_server(response_json: &str) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind mock server");
    let addr = listener.local_addr().expect("Failed to get local addr");
    let url = format!("http://{}", addr);

    let response_body = serde_json::json!({
        "response": response_json
    })
    .to_string();

    let server_task = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf).await;

            let http_response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = socket.write_all(http_response.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });

    (url, server_task)
}

fn create_test_config(ollama_url: String, destination: Destination) -> Config {
    Config {
        llm: LlmConfig::Ollama {
            model: "mock-model".to_string(),
            url: ollama_url,
        },
        output: OutputConfig {
            schema: OutputSchema {
                name: "produkty".to_string(),
                fields: vec![
                    FieldDef {
                        name: "id".to_string(),
                        field_type: FieldType::Integer,
                        required: true,
                        description: "ID".to_string(),
                        default: None,
                    },
                    FieldDef {
                        name: "nazwa".to_string(),
                        field_type: FieldType::String,
                        required: true,
                        description: "Nazwa".to_string(),
                        default: None,
                    },
                    FieldDef {
                        name: "cena".to_string(),
                        field_type: FieldType::Decimal {
                            precision: 10,
                            scale: 2,
                        },
                        required: true,
                        description: "Cena".to_string(),
                        default: None,
                    },
                    FieldDef {
                        name: "data_dodania".to_string(),
                        field_type: FieldType::Date {
                            format: "%Y-%m-%d".to_string(),
                        },
                        required: true,
                        description: "Data".to_string(),
                        default: None,
                    },
                    FieldDef {
                        name: "czy_dostepny".to_string(),
                        field_type: FieldType::Boolean,
                        required: true,
                        description: "Dostepnosc".to_string(),
                        default: None,
                    },
                ],
                case_normalization: None,
            },
            destination,
        },
        settings: osom_config::AppSettings {
            request_timeout_secs: 10,
            max_retries: 1,
            log_level: "info".to_string(),
        },
    }
}

#[tokio::test]
async fn test_full_pipeline_e2e_json_output() {
    let mock_llm_reply = r#"```json
[
  {
    "id": 1,
    "nazwa": "Laptop Dell",
    "cena": 4999.991,
    "data_dodania": "2024-01-15",
    "czy_dostepny": true
  }
]
```"#;

    let (mock_url, server_handle) = start_mock_ollama_server(mock_llm_reply).await;

    let out_path = "/tmp/opencode/e2e_out.json";
    let _ = tokio::fs::remove_file(out_path).await;

    let config = create_test_config(
        mock_url,
        Destination::Json {
            path: out_path.to_string(),
            pretty: true,
        },
    );

    let input_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../sample_input.csv");
    let options = PipelineOptions {
        dry_run: false,
        output_override: None,
        pattern: None,
    };

    let result = run_pipeline_with_options(&config, input_path, &options).await;
    assert!(result.is_ok(), "Pipeline failed: {:?}", result.err());

    // Verify output file
    let content = tokio::fs::read_to_string(out_path)
        .await
        .expect("Failed to read output json");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("Invalid json written");
    assert_eq!(parsed.as_array().unwrap().len(), 1);
    assert_eq!(parsed[0]["nazwa"], "Laptop Dell");
    assert_eq!(parsed[0]["cena"], 4999.99); // Decimal rounded to scale 2
    assert_eq!(parsed[0]["data_dodania"], "2024-01-15");

    let _ = tokio::fs::remove_file(out_path).await;
    let _ = server_handle.await;
}

#[tokio::test]
async fn test_full_pipeline_e2e_sqlite_output() {
    let mock_llm_reply = r#"[
  {
    "id": 2,
    "nazwa": "Myszka bezprzewodowa",
    "cena": 149.50,
    "data_dodania": "2024-02-01",
    "czy_dostepny": true
  }
]"#;

    let (mock_url, server_handle) = start_mock_ollama_server(mock_llm_reply).await;

    let db_path = "/tmp/opencode/e2e_out.db";
    let _ = tokio::fs::remove_file(db_path).await;

    let config = create_test_config(
        mock_url,
        Destination::Database {
            connection: osom_config::DbConnection::Sqlite {
                path: db_path.to_string(),
            },
            table_name: Some("produkty".to_string()),
        },
    );

    let input_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../sample_input.csv");
    let options = PipelineOptions {
        dry_run: false,
        output_override: None,
        pattern: None,
    };

    let result = run_pipeline_with_options(&config, input_path, &options).await;
    assert!(result.is_ok(), "Pipeline failed: {:?}", result.err());

    // Verify database row
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", db_path))
        .await
        .expect("Failed to connect to sqlite");
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM produkty")
        .fetch_one(&pool)
        .await
        .expect("Failed to query row count");
    assert_eq!(count.0, 1);

    let _ = tokio::fs::remove_file(db_path).await;
    let _ = server_handle.await;
}

#[tokio::test]
async fn test_full_pipeline_e2e_xml_output_with_override() {
    let mock_llm_reply = r#"[
  {
    "id": 3,
    "nazwa": "Klawiatura",
    "cena": 299.00,
    "data_dodania": "2024-03-01",
    "czy_dostepny": false
  }
]"#;

    let (mock_url, server_handle) = start_mock_ollama_server(mock_llm_reply).await;

    let default_path = "/tmp/opencode/never_created.xml";
    let override_path = "/tmp/opencode/actual_override.xml";
    let _ = tokio::fs::remove_file(default_path).await;
    let _ = tokio::fs::remove_file(override_path).await;

    let config = create_test_config(
        mock_url,
        Destination::Xml {
            path: default_path.to_string(),
            pretty: true,
        },
    );

    let input_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../sample_input.csv");
    let options = PipelineOptions {
        dry_run: false,
        output_override: Some(override_path.to_string()),
        pattern: None,
    };

    let result = run_pipeline_with_options(&config, input_path, &options).await;
    assert!(result.is_ok(), "Pipeline failed: {:?}", result.err());

    assert!(!tokio::fs::try_exists(default_path).await.unwrap_or(false));
    assert!(tokio::fs::try_exists(override_path).await.unwrap_or(false));

    let xml_content = tokio::fs::read_to_string(override_path).await.unwrap();
    assert!(xml_content.contains("<records>"));
    assert!(xml_content.contains("<nazwa>Klawiatura</nazwa>"));

    let _ = tokio::fs::remove_file(override_path).await;
    let _ = server_handle.await;
}

#[tokio::test]
async fn test_batch_pipeline_directory_processing() {
    let mock_llm_reply = r#"[
  {
    "id": 100,
    "nazwa": "Batch item",
    "cena": 50.0,
    "data_dodania": "2024-05-01",
    "czy_dostepny": true
  }
]"#;

    // Start a multi-connection mock server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind mock server");
    let addr = listener.local_addr().expect("Failed to get local addr");
    let mock_url = format!("http://{}", addr);

    let response_body = serde_json::json!({ "response": mock_llm_reply }).to_string();
    let server_task = tokio::spawn(async move {
        // Accept up to 2 requests for the 2 batch files
        for _ in 0..2 {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 4096];
                let _ = socket.read(&mut buf).await;
                let http_response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                let _ = socket.write_all(http_response.as_bytes()).await;
                let _ = socket.flush().await;
            }
        }
    });

    let batch_dir = "/tmp/opencode/test_batch_input";
    let _ = tokio::fs::remove_dir_all(batch_dir).await;
    tokio::fs::create_dir_all(batch_dir).await.unwrap();

    let file1 = format!("{}/doc1.csv", batch_dir);
    let file2 = format!("{}/doc2.json", batch_dir);
    tokio::fs::write(&file1, "id,nazwa,cena,data_dodania,czy_dostepny\n1,A,10,2024-01-01,true\n").await.unwrap();
    tokio::fs::write(&file2, r#"{"id": 2, "nazwa": "B", "cena": 20, "data_dodania": "2024-01-02", "czy_dostepny": false}"#).await.unwrap();

    let out_path = "/tmp/opencode/test_batch_out.json";
    let _ = tokio::fs::remove_file(out_path).await;

    let config = create_test_config(
        mock_url,
        Destination::Json {
            path: out_path.to_string(),
            pretty: true,
        },
    );

    let options = PipelineOptions {
        dry_run: false,
        output_override: None,
        pattern: None,
    };

    let result = run_pipeline_with_options(&config, batch_dir, &options).await;
    assert!(result.is_ok(), "Batch pipeline failed: {:?}", result.err());

    let content = tokio::fs::read_to_string(out_path).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    let records = parsed.as_array().unwrap();
    assert_eq!(records.len(), 2, "Expected 2 records from 2 files");

    let _ = tokio::fs::remove_file(out_path).await;
    let _ = tokio::fs::remove_dir_all(batch_dir).await;
    let _ = server_task.await;
}
