use anyhow::Result;
use rocksdb::{DBCompressionType, Options, DB};
use std::path::Path;

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
