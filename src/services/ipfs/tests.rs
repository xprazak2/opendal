
use std::env;
use std::io::Result;
use std::sync::Arc;

use crate::Accessor;

/// In order to test ipfs service, please set the following environment variables:
/// - `OPENDAL_IPFS_TEST=on`: set to `on` to enable the test.
/// - `OPENDAL_IPFS_ROOT=/path/to/dir`: set the root dir.
pub async fn new() -> Result<Option<Arc<dyn Accessor>>> {
  let test_var = "OPENDAL_IPFS_TEST";
  let root_var = "OPENDAL_IPFS_ROOT";

  if env::var(test_var).is_err() || env::var(test_var).unwrap() != "on" {
    return Ok(None);
  }

  let root = env::var(root_var).unwrap_or_else(|_| "/".to_string());
  let root = format!("/{}/{}", root, uuid::Uuid::new_v4());

  let mut builder = super::Backend::build();
  builder.root(&root);

  Ok(Some(builder.finish().await?))
}
