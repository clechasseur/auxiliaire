use std::sync::Arc;

use tokio::sync::{Semaphore, SemaphorePermit};

#[derive(Debug, Clone)]
pub struct Limiter(Arc<Semaphore>);

#[derive(Debug)]
pub struct Permit<'a>(#[allow(unused)] SemaphorePermit<'a>);

impl Limiter {
    pub fn new(limit: usize) -> Self {
        Self(Arc::new(Semaphore::new(limit)))
    }

    pub async fn get_permit(&self) -> Permit<'_> {
        Permit(self.0.acquire().await.unwrap())
    }
}
