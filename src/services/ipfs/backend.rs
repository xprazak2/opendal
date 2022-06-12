use super::builder::Builder;

use crate::{Accessor, AccessorMetadata, BytesReader, BytesWriter, ObjectMetadata, DirStreamer};
use crate::ops::{OpCreate, OpRead, OpWrite, OpStat, OpDelete, OpList};

use anyhow::{Context, Error};

use async_trait::async_trait;
use bytes::{Bytes, BufMut};
use futures::StreamExt;
use futures::TryStreamExt;
use ipfs_api_backend_hyper::{IpfsClient, IpfsApi};
use ipfs_api;
use std::io;


/// Backend for IPFS service
#[derive(Debug, Clone)]
pub struct Backend {}

impl Backend {
    pub fn build() -> Builder {
      Builder::default()
    }

    pub(crate) fn get_abs_path(&self, path: &str) -> String {
      if path == "/" {
        return path.to_string()
      }

      let root = "/";

      format!("{}{}", root, path.trim_start_matches(root))
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
}

#[async_trait]
impl Accessor for Backend {
  fn metadata(&self) -> AccessorMetadata {
    unimplemented!()
  }

  async fn create(&self, args: &OpCreate) -> io::Result<()> {
    let _ = args;
    unimplemented!()
  }

  async fn read(&self, args: &OpRead) -> io::Result<BytesReader> {
    let path = self.get_abs_path(args.path());

    let offset = args.offset().map(|val| i64::try_from(val).ok()).flatten();
    let size = args.size().map(|val| i64::try_from(val).ok()).flatten();
    let res = self.files_read(&path, offset, size).await?;
    Ok(res)
  }

  async fn write(&self, args: &OpWrite) -> io::Result<BytesWriter> {
    let _ = args;
    unimplemented!()
  }

  async fn stat(&self, args: &OpStat) -> io::Result<ObjectMetadata> {
    let _ = args;
    unimplemented!()
  }

  async fn delete(&self, args: &OpDelete) -> io::Result<()> {
    let _ = args;
    unimplemented!()
  }

  async fn list(&self, args: &OpList) -> io::Result<DirStreamer> {
    let _ = args;

    // let path = self.get_abs_path(args.path());
    // debug!("object {} list start", &path);

    // let dir_stream = DirStream::new(Arc::new(self.clone()));

    // Ok(Box::new(dir_stream))
    unimplemented!()

  }
}