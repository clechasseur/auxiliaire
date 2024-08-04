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

#[cfg(test)]
mod tests {
    mod limiter {
        use std::sync::Arc;

        use test_log::test;
        use tokio::task;

        use crate::limiter::Limiter;

        #[test(tokio::test)]
        async fn test_permit() {
            let limiter = Arc::new(Limiter::new(1));
            let task_limiter = Arc::clone(&limiter);
            let permit = limiter.get_permit().await;

            let join_handle = task::spawn(async move {
                let _permit = task_limiter.get_permit().await;
            });

            drop(permit);
            assert!(join_handle.await.is_ok());
        }
    }
}
