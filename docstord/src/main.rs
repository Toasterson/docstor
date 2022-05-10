mod store;

use crate::store::RocksStore;
use anyhow::{anyhow, Result};
use clap::Parser;
use futures_core::Stream;
use libdocapi::doc_stor_server::{DocStor, DocStorServer};
use libdocapi::{Document, DocumentMetadata, Empty, ReturnCode, Status as StatusMessage};
use serde_derive::Deserialize;
use std::fs;
use std::net::{AddrParseError, SocketAddr};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

#[derive(Debug, Error)]
enum DaemonError {
    #[error("config does not specify database directory")]
    NoDatabaseSpecifiedInConfig,
    #[error("Store path must be supplied in config or as commandline. None found")]
    NoDatabasePathProvided,
}

#[derive(Debug)]
struct DocStorService {
    config: Box<Config>,
    store: Box<RocksStore>,
    verbose: bool,
    debug: bool,
}

#[tonic::async_trait]
impl DocStor for DocStorService {
    async fn ping(
        &self,
        _: Request<Empty>,
    ) -> std::result::Result<Response<StatusMessage>, Status> {
        Ok(Response::new(StatusMessage {
            code: 0,
            message: None,
        }))
    }

    async fn upload_document(
        &self,
        request: Request<Document>,
    ) -> Result<Response<StatusMessage>, Status> {
        todo!()
    }
}

impl DocStorService {
    pub fn with_config(cfg: Config) -> Result<Self> {
        Ok(DocStorService {
            config: Box::new(cfg.clone()),
            store: Box::new(RocksStore::new(
                cfg.database
                    .ok_or(DaemonError::NoDatabaseSpecifiedInConfig)?,
            )?),
            verbose: false,
            debug: false,
        })
    }

    pub fn new<P: AsRef<Path>>(store_path: P) -> Result<Self> {
        Ok(DocStorService {
            config: Box::new(Default::default()),
            store: Box::new(RocksStore::new(store_path)?),
            verbose: false,
            debug: false,
        })
    }

    pub fn enable_verbose(&mut self) {
        self.verbose = true
    }

    pub fn enable_debug(&mut self) {
        self.debug = true
    }

    pub fn get_listen_addr(&self) -> Result<SocketAddr, AddrParseError> {
        self.config.listen.parse()
    }
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(short, long, parse(from_os_str), value_name = "DIR")]
    store: Option<PathBuf>,

    /// Sets a custom config file
    #[clap(short, long, parse(from_os_str), value_name = "FILE")]
    config: Option<PathBuf>,

    /// Turn debugging information on
    #[clap(short, long, parse(from_occurrences))]
    debug: usize,
}

#[derive(Debug, Default, Deserialize, Clone)]
struct Config {
    #[serde(alias = "db")]
    database: Option<PathBuf>,

    // Network Address to listen on
    listen: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli: Cli = Cli::parse();

    let config: Option<Config> = if let Some(config_path) = cli.config.as_deref() {
        let toml_str = fs::read_to_string(config_path)?;
        Some(toml::from_str(&toml_str)?)
    } else {
        None
    };

    let mut doc_store = if let Some(cfg) = config {
        DocStorService::with_config(cfg)
    } else if let Some(store_path) = cli.store.as_deref() {
        DocStorService::new(store_path)
    } else {
        Err(anyhow!(DaemonError::NoDatabasePathProvided))
    }?;

    // You can see how many times a particular flag or argument occurred
    // Note, only flags can have multiple occurrences
    match cli.debug {
        0 => {}
        1 => doc_store.enable_verbose(),
        2 => doc_store.enable_debug(),
        _ => println!("Don't be crazy"),
    }

    let srv_addr = doc_store.get_listen_addr()?;

    let svc = DocStorServer::new(doc_store);

    Server::builder().add_service(svc).serve(srv_addr).await?;

    Ok(())
}
