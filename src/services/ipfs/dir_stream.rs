use futures::future::BoxFuture;
use futures::ready;
use futures::Future;
use ipfs_api::response::FilesLsResponse;
use std::io;
use std::sync::Arc;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;


use super::Backend;

use crate::DirEntry;
use crate::ObjectMode;

pub struct DirStream {
  backend: Arc<Backend>,
  state: State,
  path: String,
}

enum State {
  Idle,
  Sending(BoxFuture<'static, io::Result<FilesLsResponse>>),
  Listing((FilesLsResponse, usize)),
}

impl DirStream {
  pub fn new(backend: Arc<Backend>, path: &str) -> Self {
    Self { backend, state: State::Idle, path: path.to_string() }
  }
}

impl futures::Stream for DirStream {
  type Item = io::Result<DirEntry>;

  fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_> ) -> Poll<Option<Self::Item>> {
    let backend = self.backend.clone();

    match &mut self.state {
      State::Idle => {
        let path = self.path.clone();

        let fut = async move {
          let resp = backend.files_list(&path).await?;
          Ok(resp)
        };
        self.state = State::Sending(Box::pin(fut));
        self.poll_next(cx)
      },
      State::Sending(fut) => {
        let contents = ready!(Pin::new(fut).poll(cx))?;

        self.state = State::Listing((contents, 0));
        self.poll_next(cx)
      },
      State::Listing((output, idx)) => {
        while *idx < output.entries.len() {
          let object = &output.entries[*idx];
          *idx += 1;

          // https://github.com/ipfs/specs/blob/main/UNIXFS.md#data-format
          // https://github.com/ferristseng/rust-ipfs-api/blob/master/ipfs-api-prelude/src/response/files.rs#L18-L28
          let mode = match object.typ {
            1 => ObjectMode::DIR,
            2 => ObjectMode::FILE,
            _ => ObjectMode::Unknown,
          };

          let de = DirEntry::new(backend.clone(), mode, &object.name);
          return Poll::Ready(Some(Ok(de)));
        }

        Poll::Ready(None)
      }
    }
  }
}