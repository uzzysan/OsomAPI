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
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    info!("OsomAPI – Universalny Procesor Danych LLM");
    info!("Plik wejściowy: {}, Konfiguracja: {}", cli.input, cli.config);

    let config = Config::from_toml_file(&cli.config)
        .map_err(|e| anyhow::anyhow!("Błąd ładowania konfiguracji: {}", e))?;

    run_pipeline(&config, &cli.input)
        .await
        .map_err(|e| anyhow::anyhow!("Błąd potoku przetwarzania: {}", e))?;

    Ok(())
}
