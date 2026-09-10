use clap::Parser;
use osom_api::pipeline::{PipelineOptions, run_pipeline_with_options};
use osom_api::server::run_server;
use osom_config::Config;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "osom-api")]
#[command(about = "Universal API for processing data through LLM")]
struct Cli {
    #[arg(short, long, help = "Path to input file or directory")]
    input: Option<String>,
    #[arg(short, long, default_value = "config.toml", help = "Path to config file")]
    config: String,
    #[arg(long, help = "Run HTTP server and Web UI dashboard")]
    serve: bool,
    #[arg(long, help = "Server host address override [default: 0.0.0.0]")]
    host: Option<String>,
    #[arg(long, help = "Server port override [default: 8080]")]
    port: Option<u16>,
    #[arg(long, help = "Display prompt without calling the LLM")]
    dry_run: bool,
    #[arg(short, long, help = "Override output destination file path")]
    output: Option<String>,
    #[arg(short, long, help = "File extension / pattern filter when scanning directory (e.g. *.csv)")]
    pattern: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let config = Config::from_toml_file(&cli.config)
        .map_err(|e| anyhow::anyhow!("Błąd ładowania konfiguracji: {}", e))?;

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .or_else(|_| tracing_subscriber::EnvFilter::try_new(&config.settings.log_level))
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init();

    info!("OsomAPI – Universalny Procesor Danych LLM");

    // Jeśli podano flagę --serve lub nie podano pliku wejściowego, uruchamiamy serwer HTTP i Web UI
    if cli.serve || cli.input.is_none() {
        run_server(config, cli.config, cli.host, cli.port).await?;
        return Ok(());
    }

    let input_path = cli.input.unwrap();
    info!("Plik wejściowy: {}, Konfiguracja: {}", input_path, cli.config);

    let options = PipelineOptions {
        dry_run: cli.dry_run,
        output_override: cli.output,
        pattern: cli.pattern,
    };

    run_pipeline_with_options(&config, &input_path, &options)
        .await
        .map_err(|e| anyhow::anyhow!("Błąd potoku przetwarzania: {}", e))?;

    Ok(())
}
