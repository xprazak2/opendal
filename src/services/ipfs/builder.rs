use std::sync::Arc;
use std::io::Result;


use log::info;

use crate::services::ipfs::Backend;
use crate::{Accessor};
use crate::error::other;
use anyhow::anyhow;
use std::collections::HashMap;
use crate::error::BackendError;

#[derive(Default, Debug)]
pub struct Builder {
  root: Option<String>,
}

impl Builder {
  /// Set root for backend.
  pub fn root(&mut self, root: &str) -> &mut Self {
    self.root = if root.is_empty() {
        None
    } else {
        Some(root.to_string())
    };

    self
  }

  fn root_string(&self) -> Result<String> {
    match &self.root {
      None => Ok("/".to_string()),
      Some(v) => {
          debug_assert!(!v.is_empty());

          let mut v = v.clone();

          if !v.starts_with('/') {
              return Err(other(BackendError::new(
                  HashMap::from([("root".to_string(), v.clone())]),
                  anyhow!("Root must start with /"),
              )));
          }
          if !v.ends_with('/') {
              v.push('/');
          }

          Ok(v)
      }
    }
  }

  pub async fn finish(&mut self) -> Result<Arc<dyn Accessor>> {
    let root = self.root_string()?;
    info!("backend build finished: {:?}", &self);
    Ok(Arc::new(Backend::new(root)))
  }
}


#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_root_string() {
    let mut builder = Builder::default();
    builder.root("/foo/bar");
    assert_eq!(builder.root_string().unwrap(), "/foo/bar/".to_string());

    builder.root("foo");
    assert!(builder.root_string().is_err());
  }

  #[tokio::test]
  async fn test_finish() {
    let mut builder = Builder::default();
    builder.root("foo");

    assert!(builder.finish().await.is_err());

    builder.root("/");
    assert!(builder.finish().await.is_ok());
  }
}
