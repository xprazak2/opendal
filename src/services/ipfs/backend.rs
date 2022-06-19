use super::builder::Builder;

use crate::{Accessor, AccessorMetadata, BytesReader, BytesWriter, ObjectMetadata, ObjectMode, DirStreamer};
use crate::ops::{OpCreate, OpRead, OpWrite, OpStat, OpDelete, OpList};
use crate::io_util;

use async_trait::async_trait;
use futures::TryStreamExt;
use ipfs_api::response::FilesStatResponse;
use ipfs_api_backend_hyper::{IpfsClient, IpfsApi};
use ipfs_api;
use std::io;

/// Backend for IPFS service
#[derive(Debug, Clone)]
pub struct Backend {
  root: String
}

impl Backend {
    pub fn new(root: String) -> Self {
      Self { root }
    }

    pub fn build() -> Builder {
      Builder::default()
    }

    pub(crate) fn get_abs_path(&self, path: &str) -> String {
      if path == self.root {
        return path.to_string()
      }

      format!("{}{}", self.root, path.trim_start_matches(&self.root))
    }

    pub(crate) async fn files_stat(&self, path: &str) -> io::Result<ObjectMetadata> {
      let client = IpfsClient::default();
      let mut meta = ObjectMetadata::default();

      let res = client.files_stat(path).await.map_err(|err| crate::error::other(err))?;

      let mode: ObjectMode  = match res.typ.as_str() {
        "file" => ObjectMode::FILE,
        "directory" => ObjectMode::DIR,
        _ => ObjectMode::Unknown,
      };

      meta.set_mode(mode).set_content_length(res.size);

      Ok(meta)
    }

    pub(crate) async fn files_create(&self, path: &str) -> io::Result<()> {
      let client = IpfsClient::default();
      client.files_write(path, true, false, io::empty()).await.map_err(|err| crate::error::other(err))
    }

    pub(crate) async fn files_read(&self, path: &str, offset: Option<i64>, count: Option<i64>) -> io::Result<BytesReader> {
      let client = IpfsClient::default();
      let req = ipfs_api::request::FilesRead { path, offset, count };
      let reader = client
        .files_read_with_options(req)
        .map_err(|err| crate::error::other(err))
        .into_async_read();
      Ok(Box::new(reader))
    }

    pub(crate) async fn files_delete(&self, path: &str) -> io::Result<()> {
      let client = IpfsClient::default();
      client.files_rm(path, false).await.map_err(|err| crate::error::other(err))
    }

    // pub(crate) async fn files_write(&self, path: &str) -> io::Result<BytesWriter> {
    // pub(crate) async fn files_write(&self, path: &str) -> () {
    //   let client = IpfsClient::default();
    //   let files_write = ipfs_api::request::FilesWrite::builder().path(path).build();

    //   let (tx, body) = io_util::new_http_channel();

    //   let req = client.files_write_with_options(files_write, body);
    // }
}

#[async_trait]
impl Accessor for Backend {
  fn metadata(&self) -> AccessorMetadata {
    unimplemented!()
  }

  async fn create(&self, args: &OpCreate) -> io::Result<()> {
    let path = self.get_abs_path(args.path());
    self.files_create(&path).await
  }

  async fn read(&self, args: &OpRead) -> io::Result<BytesReader> {
    let path = self.get_abs_path(args.path());

    let offset = args.offset().map(|val| i64::try_from(val).ok()).flatten();
    let size = args.size().map(|val| i64::try_from(val).ok()).flatten();
    let reader = self.files_read(&path, offset, size).await?;
    Ok(reader)
  }

  async fn write(&self, args: &OpWrite) -> io::Result<BytesWriter> {
    let _ = args;
    unimplemented!()
  }

  async fn stat(&self, args: &OpStat) -> io::Result<ObjectMetadata> {
    let path = self.get_abs_path(args.path());
    self.files_stat(&path).await
  }

  async fn delete(&self, args: &OpDelete) -> io::Result<()> {
    let path = self.get_abs_path(args.path());
    self.files_delete(&path).await
  }

  async fn list(&self, args: &OpList) -> io::Result<DirStreamer> {
    let _ = args;
    unimplemented!()
  }
}
