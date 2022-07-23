use bytes::Bytes;

use std::io;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::sync::Arc;

use futures::channel::mpsc::Sender;
use futures::AsyncWrite;
use futures::ready;
use futures::Future;
use futures::future::BoxFuture;
use futures::Sink;
use futures::channel::mpsc::Receiver;

use crate::error::other;

use super::Backend;

enum State {
  Init,
  Sending(BoxFuture<'static, io::Result<()>>),
}

pub struct IpfsWriterFuture {
  rx: Receiver<Bytes>,
  backend: Arc<Backend>,
  path: String,
  state: State,
}

impl IpfsWriterFuture {
  pub fn new(rx: Receiver<Bytes>, backend: Arc<Backend>, path: String) -> Self {
    Self { rx, backend, path, state: State::Init }
  }

  fn poll_content(&mut self) -> io::Result<Option<Bytes>> {
    match self.rx.try_next() {
      Ok(Some(payload)) => {
        Ok(Some(payload))
      },
      Ok(None) => {
        unreachable!("channel should not be closed.")
      },
      Err(err) => {
        unreachable!("channel should contain a message when polled: {:?}", err)
      },
    }
  }
}

impl Future for IpfsWriterFuture {
  type Output = std::result::Result<(), io::Error>;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let backend = self.backend.clone();
    let path = self.path.clone();

    match &mut self.state {
      State::Init => {
        if let Some(bytes) = self.poll_content()? {
          let fut = async move {
            let resp = backend.files_write(&path, bytes).await?;
            Ok(resp)
          };
          self.state = State::Sending(Box::pin(fut));
          self.poll(cx)
        } else {
          Poll::Pending
        }
      },
      State::Sending(fut) => {
        let resp = ready!(Pin::new(fut).poll(cx))?;
        Poll::Ready(Ok(resp))
      },
    }
  }
}

pub(crate) struct RequestWriter {
  tx: Sender<Bytes>,
  fut: IpfsWriterFuture,
}

impl RequestWriter {
  pub fn new(tx: Sender<Bytes>, fut: IpfsWriterFuture) -> Self {
    Self { tx, fut}
  }

  fn poll_response(self: &mut Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::result::Result<(), io::Error>> {
    match Pin::new(&mut self.fut).poll(cx) {
      Poll::Ready(Ok(resp)) => Poll::Ready(Ok(resp)),
      Poll::Ready(Err(e)) => Poll::Ready(Err(other(e))),
      Poll::Pending => Poll::Pending,
    }
  }
}

impl AsyncWrite for RequestWriter {
  fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
    ready!(self.tx.poll_ready(cx).map_err(other))?;
    let size = buf.len();
    self.tx.start_send(Bytes::from(buf.to_vec())).map_err(other)?;
    Poll::Ready(Ok(size))
  }

  fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    Pin::new(&mut self.tx).poll_flush(cx).map_err(other)
  }

  fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    if let Err(err) = ready!(Pin::new(&mut self.tx).poll_close(cx)) {
      return Poll::Ready(Err(other(err)));
    }

    self.poll_response(cx)
  }
}