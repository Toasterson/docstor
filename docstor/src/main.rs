use anyhow::Result;
use clap::{Parser, Subcommand};
use libdocapi::doc_stor_client::DocStorClient;
use libdocapi::Empty;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Ping,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli: Cli = Cli::parse();

    let mut client = DocStorClient::connect("http://127.0.0.1:10000").await?;

    match cli.commands {
        Commands::Ping => {
            let response = client.ping(Empty {}).await?;
            println!(
                "Pong from Server: {}",
                match response.into_inner().code {
                    0 => "Success",
                    _ => "Internal Error",
                }
            )
        }
    }

    Ok(())
}
