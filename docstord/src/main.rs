mod store;

use std::collections::HashMap;
use crate::store::RocksStore;
use anyhow::{anyhow, Result};
use clap::Parser;
use libdocapi::doc_stor_server::{DocStor, DocStorServer};
use libdocapi::{Document, DocumentMetadata, Empty, ListFilter, Status as StatusMessage};
use serde_derive::Deserialize;
use std::fs;
use std::net::{AddrParseError, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use rocksdb::{IteratorMode, WriteBatch};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use sha3::{Digest, Sha3_512};
use store::DocumentMetadata as StoreDocumentMetadata;
use std::time::{SystemTime, UNIX_EPOCH};

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
    store: Arc<RocksStore>,
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
        let mut hasher = Sha3_512::new();
        let doc = request.into_inner();
        let mut meta: StoreDocumentMetadata = doc.meta.ok_or(
            Status::invalid_argument("Request has no metadata attached")
        )?.into();

        // write input message
        hasher.update(&doc.blob);

        // read hash digest
        let computed_hash = hasher.finalize();
        meta.hash = format!("{:x}", computed_hash);

        //setup creation time
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH).map_err(|err|
            Status::internal(format!("System clock problem: {}", err))
        )?;
        meta.creation_date = since_the_epoch.as_secs() as i64;

        meta.tags.push("INBOX".into());

        let mut meta_blob: Vec<u8> = vec![];
        ciborium::ser::into_writer(&meta, &mut meta_blob).map_err(|err|
            Status::internal(format!("{}", err))
        )?;

        let mut batch = WriteBatch::default();
        batch.put((meta.hash.clone() + "_meta").as_bytes(), meta_blob.as_slice());
        batch.put((meta.hash.clone() + "_data").as_bytes(), doc.blob.as_slice());
        self.store.db.write(batch).map_err(|err|
            Status::internal(format!("{}", err))
        )?;

        Ok(Response::new(StatusMessage {
            code: 0,
            message: None,
        }))
    }

    type ListDocumentStream = ReceiverStream<Result<DocumentMetadata, Status>>;

    async fn list_document(&self, request: Request<ListFilter>) -> std::result::Result<Response<Self::ListDocumentStream>, Status> {
        #[allow(unused_mut)]
        let (mut tx, rx) = mpsc::channel(4);
        let filter = request.into_inner();

        let thread_store = self.store.clone();
        tokio::spawn(async move {
            for (key, value) in thread_store.db.iterator(IteratorMode::Start) {
                let key_string = String::from_utf8(key.to_vec()).unwrap();
                if key_string.ends_with("_meta") {
                    let meta: StoreDocumentMetadata = ciborium::de::from_reader(value.to_vec().as_slice()).unwrap();
                    if !filter.tags.is_empty() {
                        if !meta.tags.clone().into_iter().filter(|t| {
                            filter.tags.contains(t)
                        }).collect::<Vec<String>>().is_empty() {
                            tx.send(Ok(meta.into())).await.unwrap();
                        }
                    } else if !filter.user_data.is_empty() {
                        if !meta.user_data.clone().into_iter().filter(|(t,_)| {
                            filter.user_data.contains(t)
                        }).collect::<HashMap<String, String>>().is_empty() {
                            tx.send(Ok(meta.into())).await.unwrap();
                        }
                    } else {
                        tx.send(Ok(meta.into())).await.unwrap();
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

impl DocStorService {
    pub fn with_config(cfg: Config) -> Result<Self> {
        Ok(DocStorService {
            config: Box::new(cfg.clone()),
            store: Arc::new(RocksStore::new(
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
            store: Arc::new(RocksStore::new(store_path)?),
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
