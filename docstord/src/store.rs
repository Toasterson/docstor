use std::collections::HashMap;
use anyhow::Result;
use rocksdb::{DBCompressionType, Options, DB};
use std::path::Path;
use serde::{Deserialize, Serialize};
use libdocapi::DocumentMetadata as APIDocumentMetadata;

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub path: String,
    pub hash: String,
    pub creation_date: i64,
    pub tags: Vec<String>,
    pub user_data: HashMap<String, String>,
}

impl Into<APIDocumentMetadata> for DocumentMetadata {
    fn into(self) -> APIDocumentMetadata {
        APIDocumentMetadata{
            path: self.path,
            hash: self.hash,
            creation_date: self.creation_date,
            tags: self.tags,
            user_data: self.user_data,
        }
    }
}

impl From<APIDocumentMetadata> for DocumentMetadata {
    fn from(req_data: APIDocumentMetadata) -> Self {
        DocumentMetadata{
            path: req_data.path,
            hash: req_data.hash,
            creation_date: req_data.creation_date,
            tags: req_data.tags,
            user_data: req_data.user_data
        }
    }
}

#[derive(Debug)]
pub struct RocksStore {
    pub db: DB,
}

impl RocksStore {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_compression_type(DBCompressionType::Zstd);
        opts.set_db_log_dir(path.as_ref().join("db_log"));
        opts.set_wal_dir(path.as_ref().join("wal"));
        Ok(RocksStore {
            db: DB::open(&opts, path)?,
        })
    }
}
