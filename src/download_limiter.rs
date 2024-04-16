use std::sync::Arc;

use tokio::sync::{Semaphore, SemaphorePermit};

#[derive(Debug, Clone)]
pub struct DownloadLimiter(Arc<Semaphore>);

#[derive(Debug)]
pub struct DownloadPermit<'a> {
    // Initially we were using a tuple struct here, but there seems to be an issue in Nightly
    // that triggers a warning for an unused field. See:
    // https://github.com/rust-lang/rust/issues/119645
    //
    // To work around it, we'll use a named field prefixed with `_`, which seems to fix the warning.
    _permit: SemaphorePermit<'a>,
}

impl DownloadLimiter {
    pub fn new(max_downloads: usize) -> Self {
        Self(Arc::new(Semaphore::new(max_downloads)))
    }

    pub async fn get_permit(&self) -> DownloadPermit<'_> {
        DownloadPermit { _permit: self.0.acquire().await.unwrap() }
    }
}
