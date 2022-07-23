use super::builder::Builder;
use super::dir_stream::DirStream;
use super::request_writer::{RequestWriter, IpfsWriterFuture};

use crate::{Accessor, AccessorMetadata, BytesReader, BytesWriter, ObjectMetadata, ObjectMode, DirStreamer};
use crate::ops::{OpCreate, OpRead, OpWrite, OpStat, OpDelete, OpList};

use std::fmt;
use std::sync::Arc;
use async_trait::async_trait;
use bytes::{Bytes, Buf};
use futures::TryStreamExt;
use ipfs_api::response::FilesLsResponse;
use ipfs_api::{IpfsClient, IpfsApi};
use ipfs_api;
use std::io;
use futures::channel::mpsc::{self};

/// Backend for IPFS service
#[derive(Clone)]
pub struct Backend {
  root: String,
  client: IpfsClient,
}

impl fmt::Debug for Backend {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ipfs::Backend").field("root", &self.root).finish()
  }
}

impl Backend {
    /// Constructor for ipfs builder
    pub fn new(root: String) -> Self {
      Self { root, client: IpfsClient::default() }
    }

    /// Create a default builder for ipfs.
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
      let mut meta = ObjectMetadata::default();

      let res = self.client.files_stat(path).await.map_err(|err| crate::error::other(err))?;

      let mode: ObjectMode  = match res.typ.as_str() {
        "file" => ObjectMode::FILE,
        "directory" => ObjectMode::DIR,
        _ => ObjectMode::Unknown,
      };

      meta.set_mode(mode).set_content_length(res.size);

      Ok(meta)
    }

    pub(crate) async fn files_create(&self, path: &str) -> io::Result<()> {
      self.client.files_write(path, true, false, io::empty()).await.map_err(|err| crate::error::other(err))
    }

    pub(crate) async fn files_read(&self, path: &str, offset: Option<i64>, count: Option<i64>) -> io::Result<BytesReader> {
      let req = ipfs_api::request::FilesRead { path, offset, count };
      let reader = self.client
        .files_read_with_options(req)
        .map_err(|err| crate::error::other(err))
        .into_async_read();
      Ok(Box::new(reader))
    }

    pub(crate) async fn files_delete(&self, path: &str) -> io::Result<()> {
      self.client.files_rm(path, false).await.map_err(|err| crate::error::other(err))
    }

    pub(crate) async fn files_list(&self, path: &str) -> io::Result<FilesLsResponse> {
      self.client.files_ls(Some(path)).await.map_err(|err| crate::error::other(err))
    }

    pub(crate) async fn files_write(&self, path: &str, data: Bytes) -> io::Result<()> {
      self.client.files_write(path, true, true, data.reader()).await.map_err(|err| crate::error::other(err))
    }
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
    let path = self.get_abs_path(args.path()).clone();

    let (tx, rx) = mpsc::channel::<Bytes>(0);

    let req_fut = IpfsWriterFuture::new(rx, Arc::new(self.clone()), path);

    let req_writer = RequestWriter::new(tx, req_fut);

    Ok(Box::new(req_writer))
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
    let path = self.get_abs_path(args.path());
    Ok(Box::new(DirStream::new(Arc::new(self.clone()), &path)))
  }
}
