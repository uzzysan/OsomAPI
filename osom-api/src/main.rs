mod pipeline;

use clap::Parser;
use osom_config::Config;
use pipeline::run_pipeline;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "osom-api")]
#[command(about = "Universal API for processing data through LLM")]
struct Cli {
    #[arg(short, long, help = "Path to input file")]
    input: String,
    #[arg(short, long, default_value = "config.toml", help = "Path to config file")]
    config: String,
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
    info!("Plik wejściowy: {}, Konfiguracja: {}", cli.input, cli.config);

    run_pipeline(&config, &cli.input)
        .await
        .map_err(|e| anyhow::anyhow!("Błąd potoku przetwarzania: {}", e))?;

    Ok(())
}
