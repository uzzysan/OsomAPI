use clap::Parser;

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
    let _cli = Cli::parse();
    println!("OsomAPI - Universal LLM Data Processor");
    Ok(())
}
