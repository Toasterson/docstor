use std::fs;
use std::path::PathBuf;
use std::str::Chars;
use anyhow::Result;
use clap::{Parser, Subcommand};
use prettytable::{format, row, cell, Table};
use libdocapi::doc_stor_client::DocStorClient;
use libdocapi::{Document, DocumentMetadata, Empty, ListFilter};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Ping,
    Upload {
        //Name of the uploaded file
        #[clap(short, long)]
        name: String,

        // The file to upload
        file: PathBuf,
    },
    #[clap(alias="ls")]
    List
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
        Commands::Upload { name, file } => {
            let buff = fs::read(file)?;

            let meta = DocumentMetadata{
                path: name,
                hash: "".to_string(),
                creation_date: 0,
                tags: vec![],
                user_data: Default::default()
            };

            let request = Document{meta: Some(meta), blob: buff};
            let response = client.upload_document(request).await?;
            println!(
                "File Uploaded: {}",
                match response.into_inner().code {
                    0 => "Success",
                    _ => "Internal Error",
                }
            )
        }
        Commands::List => {
            let mut stream = client.list_document(ListFilter{
                tags: vec![],
                user_data: vec![]
            }).await?.into_inner();
            let mut table = Table::new();
            let format = format::FormatBuilder::new()
                .column_separator('|')
                .borders('|')
                .separators(&[format::LinePosition::Top,
                    format::LinePosition::Bottom],
                            format::LineSeparator::new('-', '+', '+', '+'))
                .padding(1, 1)
                .build();
            table.set_format(format);

            table.set_titles(row!["NAME", "HASH", "TAGS", "DATA"]);
            #[allow(unused_variables)]
            while let Some(meta) = stream.message().await? {
                table.add_row(row![
                         meta.path,
                         meta.hash.chars().take(20).collect::<String>(),
                         meta.tags.join(", "),
                         meta.user_data.into_iter().map(|(k,v)| {
                             format!("{}={}",k,v)
                         }).collect::<Vec<String>>().join(", ")
                    ]
                );
            }
            table.printstd();
        }
    }

    Ok(())
}
