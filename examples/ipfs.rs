use std::sync::Arc;
use std::env;

use anyhow::Result;

use futures::StreamExt;
use log::info;
use opendal::{services::ipfs, Accessor, Operator};

#[tokio::main]
async fn main() -> Result<()> {
  if env::var("RUST_LOG").is_err() {
    env::set_var("RUST_LOG", "debug");
  }
  env_logger::init();

  println!(
    r#"OpenDAL IPFS Example.

Available Environment Values:

- OPENDAL_IPFS_ROOT: root path in mutable file system, default: /
"#
);

  let mut builder = ipfs::Backend::build();
  // root must be absolute path in MFS.
  builder.root(&env::var("OPENDAL_IPFS_ROOT").unwrap_or_else(|_| "/".to_string()));

  // Will use endpoint specified in ~/.ipfs/api, falls back to `localhost:5001`
  let accessor: Arc<dyn Accessor> = builder.finish().await?;

  let op: Operator = Operator::new(accessor);

  let path = "/file.txt";
  info!("try to write file: {}", &path);
  op.object(&path).write("Hello, world!").await?;
  info!("write file successful!");

  let content = op.object(&path).read().await?;
  info!("File content: {}", String::from_utf8_lossy(&content));

  let root = "/";
  let mut list = op.object(&root).list().await?;
  info!("Listing entries in {}", &root);
  while let Some(res) = list.next().await {
    let item = res?;
    info!("Found entry: {}", item.path())
  }

  Ok(())
}