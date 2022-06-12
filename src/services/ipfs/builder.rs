use std::sync::Arc;
use std::io::Result;


use log::info;

use crate::services::ipfs::Backend;
use crate::{Accessor};

#[derive(Default, Debug)]
pub struct Builder {}

impl Builder {
  pub async fn finish(&mut self) -> Result<Arc<dyn Accessor>> {
    info!("backend build started: {:?}", &self);

    info!("backend build finished: {:?}", &self);

    Ok(Arc::new(Backend{}))
  }
}
