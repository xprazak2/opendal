use std::sync::Arc;
use std::env;

use anyhow::Result;

use futures::StreamExt;
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

  let path = "/mfs/QmckbcLXxdgSHJVY2dHc2tN6Sz53zNe9C5YDbDdvSoNkVS/file.txt";

  let content = op.object(&path).read().await?;

  println!("File content: {}", String::from_utf8_lossy(&content));

  let dd = "/mfs/QmckbcLXxdgSHJVY2dHc2tN6Sz53zNe9C5YDbDdvSoNkVS/";

  let mut list = op.object(&dd).list().await?;

  while let Some(res) = list.next().await {
    let item = res?;
    println!("{}", item.path())
  }

  Ok(())
}