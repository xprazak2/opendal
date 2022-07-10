use bytes::Buf;
use futures::Sink;
use futures::channel::mpsc::Receiver;
use ipfs_api::IpfsApi;
use ipfs_api::IpfsClient;
use pin_project::pin_project;

use bytes::Bytes;

use std::error::Error;
use std::io;

use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use futures::channel::mpsc::Sender;
use futures::AsyncWrite;
use futures::ready;
use futures::Future;

use crate::ops::OpWrite;
use crate::error::other;

use futures_lite::future::FutureExt;
pub struct IpfsReqFuture {
  rx: Receiver<Bytes>,
  client: IpfsClient,
  path: String,
  content: Option<Bytes>,
  req_fut: Option<Pin<Box<dyn Future<Output = Result<(), ipfs_api::Error>> + Send>>>,
}

impl IpfsReqFuture {
  pub fn new(rx: Receiver<Bytes>, client: IpfsClient, path: String) -> Self {
    Self { rx, client, path, content: None, req_fut: None }
  }

  fn poll_content(&mut self) -> io::Result<Option<Bytes>> {
    match self.rx.try_next() {
      Ok(Some(payload)) => {
        println!("Message");
        Ok(Some(payload))
      },
      Ok(None) => {
        println!("Nothing");
        Ok(None)
      },
      Err(err) => {
        unreachable!("channel should contain a message when polled: {:?}", err)
      },
    }
  }

  fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<<IpfsReqFuture as Future>::Output> {
    let ct = self.content.clone().unwrap().reader();
    println!("Fire away!");
    let res = self.client.files_write(&self.path, true, true, ct).poll(cx);
    println!("Polling...");
    match res {
      Poll::Ready(Ok(data)) => Poll::Ready(Ok(data)),
      Poll::Ready(Err(e)) => Poll::Ready(Err(other(e))),
      Poll::Pending => {
        println!("Pending..");
        Poll::Pending
      },
    }
  }
}

impl Future for IpfsReqFuture {
  type Output = std::result::Result<(), io::Error>;

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let pr = match &self.content {
      Some(_) => {
        self.poll_write(cx)
      },
      None => {
        if let Some(bytes) = self.poll_content()? {
          self.content = Some(bytes.clone());
          println!("Content: {:?}", String::from_utf8_lossy(&self.content.as_ref().unwrap()));
          // store future in self for future poll in the next iteration....
          self.req_fut = Some(self.client.files_write(&self.path, true, true, bytes.reader()));
          let res = self.req_fut.and_then(|ft| Some(ft.poll(cx)));
          println!("Res: {:?}", res);
          if let Some(result) = res {
            match result {
              Poll::Ready(Ok(data)) => Poll::Ready(Ok(data)),
              Poll::Ready(Err(e)) => Poll::Ready(Err(other(e))),
              Poll::Pending => Poll::Pending,
            }
          } else {
            unreachable!("Request future is always created");
          }
        } else {
          Poll::Pending
        }
      }
    };
    println!("Poll returned: {:?}", pr);
    pr
  }
}

type WriteFut = Pin<Box<dyn futures::Future<Output = Result<(), ipfs_api::Error>> + std::marker::Send>>;

#[pin_project]
pub(crate) struct RequestWriter {
  op: OpWrite,
  tx: Sender<Bytes>,
  fut: IpfsReqFuture,
  handle: String,
}

impl RequestWriter {
  pub fn new(op: &OpWrite, tx: Sender<Bytes>, fut: IpfsReqFuture, handle: String) -> Self {
    Self { op: op.clone(), tx, fut, handle }
  }

  fn poll_response(self: &mut Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::result::Result<(), io::Error>> {
    match Pin::new(&mut self.fut).poll(cx) {
      Poll::Ready(Ok(resp)) => Poll::Ready(Ok(())),
      Poll::Ready(Err(e)) => Poll::Ready(Err(other(e))),
      Poll::Pending => Poll::Pending,
    }
  }
}

impl AsyncWrite for RequestWriter {
  fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
    let pp = ready!(self.tx.poll_ready(cx).map_err(other))?;
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