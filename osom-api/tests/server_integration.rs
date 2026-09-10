use osom_api::server::{create_router, AppState};
use osom_config::{Config, Destination, EndpointConfig, FieldDef, FieldType, LlmConfig, OutputConfig, OutputSchema, ServerConfig};
use std::sync::Arc;
use tokio::sync::RwLock;

fn create_test_config() -> Config {
    Config {
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 0,
        },
        llm: LlmConfig::Ollama {
            model: "llama3".to_string(),
            url: "http://localhost:11434".to_string(),
        },
        output: OutputConfig {
            schema: OutputSchema {
                name: "test_entity".to_string(),
                fields: vec![
                    FieldDef {
                        name: "id".to_string(),
                        field_type: FieldType::Integer,
                        required: true,
                        description: "ID".to_string(),
                        default: None,
                    },
                    FieldDef {
                        name: "title".to_string(),
                        field_type: FieldType::String,
                        required: true,
                        description: "Title".to_string(),
                        default: None,
                    },
                ],
                case_normalization: None,
            },
            destination: Destination::Json {
                path: "/tmp/opencode/server_out.json".to_string(),
                pretty: true,
            },
        },
        endpoints: Vec::new(),
        settings: osom_config::AppSettings::default(),
    }
}

async fn spawn_test_server() -> (String, tokio::task::JoinHandle<()>, String) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind listener");
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    let config_path = format!("/tmp/opencode/test_server_config_{}.toml", addr.port());
    let config = create_test_config();
    let _ = config.save_to_toml_file(&config_path);

    let state = AppState {
        config: Arc::new(RwLock::new(config)),
        config_path: config_path.clone(),
    };

    let app = create_router(state);
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (base_url, handle, config_path)
}

#[tokio::test]
async fn test_server_status_and_index() {
    let (base_url, handle, config_path) = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 1. GET / serves HTML Web UI
    let res = client.get(&base_url).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let html = res.text().await.unwrap();
    assert!(html.contains("OsomAPI"));
    assert!(html.contains("Endpoints"));

    // 2. GET /api/status
    let res = client
        .get(format!("{}/api/status", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let status_json: serde_json::Value = res.json().await.unwrap();
    assert_eq!(status_json["status"], "running");
    assert_eq!(status_json["llm_provider"], "ollama");

    let _ = std::fs::remove_file(config_path);
    handle.abort();
}

#[tokio::test]
async fn test_server_preview_extraction() {
    let (base_url, handle, config_path) = spawn_test_server().await;
    let client = reqwest::Client::new();

    let sample_csv = "first_name,last_name,department\nJohn,Doe,Engineering\n";
    let res = client
        .post(format!("{}/api/preview?filename=sample.csv", base_url))
        .header("Content-Type", "text/csv")
        .body(sample_csv)
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let fields: Vec<osom_parser::InputFieldSample> = res.json().await.unwrap();
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].name, "first_name");
    assert_eq!(fields[0].sample_value.as_deref(), Some("John"));

    let _ = std::fs::remove_file(config_path);
    handle.abort();
}

#[tokio::test]
async fn test_server_endpoint_crud() {
    let (base_url, handle, config_path) = spawn_test_server().await;
    let client = reqwest::Client::new();

    // List initial endpoints (synthesized default)
    let res = client
        .get(format!("{}/api/endpoints", base_url))
        .send()
        .await
        .unwrap();
    let initial_eps: Vec<EndpointConfig> = res.json().await.unwrap();
    assert_eq!(initial_eps.len(), 1);
    assert_eq!(initial_eps[0].id, "default");

    // Create a new endpoint
    let new_endpoint = EndpointConfig {
        id: "logistics".to_string(),
        name: "Logistics API".to_string(),
        description: "Cargo and shipments".to_string(),
        llm: None,
        output: OutputConfig {
            schema: OutputSchema {
                name: "shipment".to_string(),
                fields: vec![FieldDef {
                    name: "tracking_no".to_string(),
                    field_type: FieldType::String,
                    required: true,
                    description: "Tracking number".to_string(),
                    default: None,
                }],
                case_normalization: None,
            },
            destination: Destination::Json {
                path: "/tmp/opencode/logistics.json".to_string(),
                pretty: true,
            },
        },
        field_mappings: Vec::new(),
    };

    let res = client
        .post(format!("{}/api/endpoints", base_url))
        .json(&new_endpoint)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // Verify endpoint is listed
    let res = client
        .get(format!("{}/api/endpoints", base_url))
        .send()
        .await
        .unwrap();
    let eps: Vec<EndpointConfig> = res.json().await.unwrap();
    assert_eq!(eps.len(), 1);
    assert_eq!(eps[0].id, "logistics");

    // Dry-run on the new endpoint
    let sample_csv = "tracking_no,carrier\nTRK12345,FedEx\n";
    let dry_res = client
        .post(format!("{}/api/dry-run/logistics", base_url))
        .body(sample_csv)
        .send()
        .await
        .unwrap();
    assert_eq!(dry_res.status(), 200);
    let dry_json: serde_json::Value = dry_res.json().await.unwrap();
    assert_eq!(dry_json["endpoint_id"], "logistics");
    assert!(dry_json["prompt"].as_str().unwrap().contains("shipment"));

    // Delete the endpoint
    let del_res = client
        .delete(format!("{}/api/endpoints/logistics", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(del_res.status(), 200);

    let _ = std::fs::remove_file(config_path);
    handle.abort();
}
