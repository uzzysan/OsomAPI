use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, Json},
    routing::{delete, get, post},
    Router,
};
use osom_config::{Config, EndpointConfig};
use osom_parser::extract_input_fields;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::pipeline::run_endpoint_pipeline;

/// Stan współdzielony serwera HTTP
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    pub config_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileQuery {
    pub filename: Option<String>,
}

async fn index_handler() -> Html<&'static str> {
    Html(include_str!("web/index.html"))
}

async fn status_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let cfg = state.config.read().await;
    let provider = match &cfg.llm {
        osom_config::LlmConfig::Ollama { .. } => "ollama",
        osom_config::LlmConfig::Gemini { .. } => "gemini",
        osom_config::LlmConfig::OpenAi { .. } => "openai",
        osom_config::LlmConfig::Anthropic { .. } => "anthropic",
        osom_config::LlmConfig::Copilot { .. } => "copilot",
    };
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "status": "running",
        "llm_provider": provider,
        "endpoints_count": cfg.list_endpoints().len(),
        "server": {
            "host": cfg.server.host,
            "port": cfg.server.port,
        }
    }))
}

async fn get_config_handler(State(state): State<AppState>) -> Json<Config> {
    let cfg = state.config.read().await;
    Json(cfg.clone())
}

async fn update_config_handler(
    State(state): State<AppState>,
    Json(new_config): Json<Config>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    new_config
        .validate()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Validation error: {}", e)))?;

    new_config
        .save_to_toml_file(&state.config_path)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to save config: {}", e),
            )
        })?;

    *state.config.write().await = new_config;
    Ok(Json(serde_json::json!({ "success": true })))
}

async fn list_endpoints_handler(State(state): State<AppState>) -> Json<Vec<EndpointConfig>> {
    let cfg = state.config.read().await;
    Json(cfg.list_endpoints())
}

async fn create_or_update_endpoint_handler(
    State(state): State<AppState>,
    Json(endpoint): Json<EndpointConfig>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if endpoint.id.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Endpoint ID cannot be empty".into()));
    }
    if endpoint.output.schema.fields.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Schema must have at least one field".into(),
        ));
    }

    let mut cfg = state.config.write().await;
    cfg.add_or_update_endpoint(endpoint.clone());
    cfg.save_to_toml_file(&state.config_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to save config: {}", e),
        )
    })?;

    Ok(Json(
        serde_json::json!({ "success": true, "endpoint": endpoint }),
    ))
}

async fn delete_endpoint_handler(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut cfg = state.config.write().await;
    if cfg.remove_endpoint(&id) {
        cfg.save_to_toml_file(&state.config_path).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to save config: {}", e),
            )
        })?;
        Ok(Json(serde_json::json!({ "success": true })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            format!("Endpoint '{}' not found", id),
        ))
    }
}

async fn preview_handler(
    Query(query): Query<FileQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Vec<osom_parser::InputFieldSample>>, (StatusCode, String)> {
    let filename = query.filename.or_else(|| {
        headers
            .get("x-filename")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    });
    let ext = filename
        .as_deref()
        .and_then(|f| std::path::Path::new(f).extension())
        .and_then(|e| e.to_str());

    let fields = extract_input_fields(&body, ext)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Preview extraction failed: {}", e)))?;
    Ok(Json(fields))
}

async fn process_handler(
    Path(endpoint_id): Path<String>,
    State(state): State<AppState>,
    Query(query): Query<FileQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    handle_execution(endpoint_id, query, headers, state, body, false).await
}

async fn dry_run_handler(
    Path(endpoint_id): Path<String>,
    State(state): State<AppState>,
    Query(query): Query<FileQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    handle_execution(endpoint_id, query, headers, state, body, true).await
}

async fn handle_execution(
    endpoint_id: String,
    query: FileQuery,
    headers: HeaderMap,
    state: AppState,
    body: Bytes,
    dry_run: bool,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let cfg = state.config.read().await.clone();
    let endpoint = cfg.find_endpoint(&endpoint_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Endpoint '{}' not found", endpoint_id),
        )
    })?;

    let filename = query.filename.or_else(|| {
        headers
            .get("x-filename")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    });
    let ext = filename
        .as_deref()
        .and_then(|f| std::path::Path::new(f).extension())
        .and_then(|e| e.to_str());

    let (prompt, formatted) =
        run_endpoint_pipeline(&cfg, &endpoint, &body, ext, dry_run, !dry_run)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Pipeline execution error: {}", e),
                )
            })?;

    if dry_run {
        Ok(Json(serde_json::json!({
            "success": true,
            "endpoint_id": endpoint.id,
            "prompt": prompt
        })))
    } else {
        Ok(Json(serde_json::json!({
            "success": true,
            "endpoint_id": endpoint.id,
            "records_count": formatted.records.len(),
            "records": formatted.records
        })))
    }
}

/// Tworzy instancję routera Axum ze wszystkimi endpointami API i UI.
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/api/status", get(status_handler))
        .route(
            "/api/config",
            get(get_config_handler).post(update_config_handler),
        )
        .route(
            "/api/endpoints",
            get(list_endpoints_handler).post(create_or_update_endpoint_handler),
        )
        .route("/api/endpoints/:id", delete(delete_endpoint_handler))
        .route("/api/preview", post(preview_handler))
        .route("/api/process/:endpoint_id", post(process_handler))
        .route("/api/dry-run/:endpoint_id", post(dry_run_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Uruchamia serwer HTTP
pub async fn run_server(
    config: Config,
    config_path: String,
    host_override: Option<String>,
    port_override: Option<u16>,
) -> Result<(), anyhow::Error> {
    let host = host_override.unwrap_or_else(|| config.server.host.clone());
    let port = port_override.unwrap_or(config.server.port);

    let state = AppState {
        config: Arc::new(RwLock::new(config)),
        config_path,
    };

    let app = create_router(state);
    let addr = format!("{}:{}", host, port);
    info!("Uruchamianie serwera OsomAPI na http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
