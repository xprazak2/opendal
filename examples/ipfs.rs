use std::sync::Arc;

use anyhow::Result;

use opendal::{services::ipfs, Accessor, Operator};

#[tokio::main]
async fn main() -> Result<()> {
  println!(
    r#"OpenDAL ipfs Example.
"#
);

  let mut builder = ipfs::Backend::build();

  let accessor: Arc<dyn Accessor> = builder.finish().await?;

  let op: Operator = Operator::new(accessor);

  let path = "/mfs/QmckbcLXxdgSHJVY2dHc2tN6Sz53zNe9C5YDbDdvSoNkVS/text.txt";

  let content = op.object(&path).read().await?;

  println!("This is crazy: {}", String::from_utf8_lossy(&content));

  Ok(())
}