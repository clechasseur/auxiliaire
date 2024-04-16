use std::fmt::Display;
use std::future::Future;
use std::panic::resume_unwind;

use anyhow::anyhow;
use tokio::task::JoinSet;

use crate::error::MultiError;
use crate::Result;

#[derive(Debug)]
pub struct TaskPool {
    join_set: JoinSet<Result<()>>,
}

impl TaskPool {
    pub fn new() -> Self {
        Self { join_set: JoinSet::new() }
    }

    pub fn spawn<F>(&mut self, task: F)
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        self.join_set.spawn(task);
    }

    pub async fn join<C, F>(&mut self, context: F) -> Result<()>
    where
        F: FnOnce() -> C,
        C: Display + Send + Sync + 'static,
    {
        let mut errors = Vec::new();

        while let Some(join_result) = self.join_set.join_next().await {
            match join_result {
                Ok(Ok(_)) => (),
                Ok(Err(task_error)) => errors.push(task_error),
                Err(join_error) => match join_error.try_into_panic() {
                    Ok(panic_err) => resume_unwind(panic_err),
                    Err(join_error) => errors.push(anyhow!("Join error: {join_error}")),
                },
            }
        }

        MultiError::check(errors, context)
    }
}

//noinspection DuplicatedCode
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::Context;
    use assert_matches::assert_matches;
    use reqwest::get;
    use wiremock::http::Method::Get;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::download_limiter::DownloadLimiter;

    async fn get_mock_server() -> MockServer {
        let mock_server = MockServer::start().await;

        Mock::given(method(Get))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(10)))
            .mount(&mock_server)
            .await;

        mock_server
    }

    #[tokio::test]
    async fn test_one_download() {
        let mock_server = get_mock_server().await;
        let mut task_pool = TaskPool::new();
        let limiter = DownloadLimiter::new(1);

        task_pool.spawn(async move {
            let _permit = limiter.get_permit();
            let result = get(format!("{}/", mock_server.uri())).await;
            assert_matches!(result, Ok(response) if response.status().is_success());
            Ok(())
        });

        assert!(task_pool.join(|| "should not happen").await.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_downloads_no_limit() {
        let mock_server = get_mock_server().await;
        let mut task_pool = TaskPool::new();
        let limiter = DownloadLimiter::new(100);

        for _ in 0..10 {
            let uri = mock_server.uri();
            let limiter = limiter.clone();
            task_pool.spawn(async move {
                let _permit = limiter.get_permit();
                let result = get(format!("{}/", uri)).await;
                assert_matches!(result, Ok(response) if response.status().is_success());
                Ok(())
            });
        }

        assert!(task_pool.join(|| "should not happen").await.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_downloads_with_limit() {
        let mock_server = get_mock_server().await;
        let mut task_pool = TaskPool::new();
        let limiter = DownloadLimiter::new(2);

        for _ in 0..10 {
            let uri = mock_server.uri();
            let limiter = limiter.clone();
            task_pool.spawn(async move {
                let _permit = limiter.get_permit();
                let result = get(format!("{}/", uri)).await;
                assert_matches!(result, Ok(response) if response.status().is_success());
                Ok(())
            });
        }

        assert!(task_pool.join(|| "should not happen").await.is_ok());
    }

    #[tokio::test]
    async fn test_errors() {
        let mock_server = get_mock_server().await;
        let mut task_pool = TaskPool::new();
        let limiter = DownloadLimiter::new(100);

        for i in 0..10 {
            let uri = mock_server.uri();
            let limiter = limiter.clone();
            task_pool.spawn(async move {
                let _permit = limiter.get_permit();
                get(if i % 2 == 0 { format!("{}/", uri) } else { format!("{}/doesnotexist", uri) })
                    .await
                    .with_context(|| "download error")?
                    .error_for_status()
                    .with_context(|| "status error")
                    .map(|_| ())
            });
        }

        assert!(task_pool.join(|| "error occurred").await.is_err());
    }
}
