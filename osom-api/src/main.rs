use clap::Parser;
use osom_api::pipeline::{PipelineOptions, run_pipeline_with_options};
use osom_config::Config;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "osom-api")]
#[command(about = "Universal API for processing data through LLM")]
struct Cli {
    #[arg(short, long, help = "Path to input file or directory")]
    input: String,
    #[arg(short, long, default_value = "config.toml", help = "Path to config file")]
    config: String,
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
    info!("Plik wejściowy: {}, Konfiguracja: {}", cli.input, cli.config);

    let options = PipelineOptions {
        dry_run: cli.dry_run,
        output_override: cli.output,
        pattern: cli.pattern,
    };

    run_pipeline_with_options(&config, &cli.input, &options)
        .await
        .map_err(|e| anyhow::anyhow!("Błąd potoku przetwarzania: {}", e))?;

    Ok(())
}
