use std::fmt::Display;
use std::future::Future;
use std::panic::resume_unwind;

use anyhow::Context;
use tokio::task::{AbortHandle, JoinSet};

use crate::Result;
use crate::error::MultiError;

#[derive(Debug, Default)]
pub struct TaskPool {
    join_set: JoinSet<Result<()>>,
}

impl TaskPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn<F>(&mut self, task: F) -> AbortHandle
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        self.join_set.spawn(task)
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
                    Err(join_error) => errors.push(
                        Err::<(), _>(join_error)
                            .with_context(|| "join error")
                            .unwrap_err(),
                    ),
                },
            }
        }

        MultiError::check(errors, context)
    }
}

//noinspection DuplicatedCode
#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use anyhow::Context;
    use assert_matches::assert_matches;
    use itertools::Itertools;
    use mini_exercism::http::get;
    use test_log::test;
    use tokio::sync::Mutex;
    use tokio::task::JoinError;
    use wiremock::http::Method;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::limiter::Limiter;

    async fn get_mock_server() -> MockServer {
        let mock_server = MockServer::start().await;

        Mock::given(method(Method::GET))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(10)))
            .mount(&mock_server)
            .await;

        mock_server
    }

    #[test(tokio::test)]
    async fn test_one_download() {
        let mock_server = get_mock_server().await;
        let mut task_pool = TaskPool::new();
        let limiter = Limiter::new(1);

        task_pool.spawn(async move {
            let _permit = limiter.get_permit();
            let result = get(format!("{}/", mock_server.uri())).await;
            assert_matches!(result, Ok(response) if response.status().is_success());
            Ok(())
        });

        assert!(task_pool.join(|| "should not happen").await.is_ok());
    }

    #[test(tokio::test)]
    async fn test_multiple_downloads_no_limit() {
        let mock_server = get_mock_server().await;
        let mut task_pool = TaskPool::new();
        let limiter = Limiter::new(100);

        for _ in 0..10 {
            let uri = mock_server.uri();
            let limiter = limiter.clone();
            task_pool.spawn(async move {
                let _permit = limiter.get_permit();
                let result = get(format!("{uri}/")).await;
                assert_matches!(result, Ok(response) if response.status().is_success());
                Ok(())
            });
        }

        assert!(task_pool.join(|| "should not happen").await.is_ok());
    }

    #[test(tokio::test)]
    async fn test_multiple_downloads_with_limit() {
        let mock_server = get_mock_server().await;
        let mut task_pool = TaskPool::new();
        let limiter = Limiter::new(2);

        for _ in 0..10 {
            let uri = mock_server.uri();
            let limiter = limiter.clone();
            task_pool.spawn(async move {
                let _permit = limiter.get_permit();
                let result = get(format!("{uri}/")).await;
                assert_matches!(result, Ok(response) if response.status().is_success());
                Ok(())
            });
        }

        assert!(task_pool.join(|| "should not happen").await.is_ok());
    }

    #[test(tokio::test)]
    async fn test_errors() {
        let mock_server = get_mock_server().await;
        let mut task_pool = TaskPool::new();
        let limiter = Limiter::new(100);

        for i in 0..10 {
            let uri = mock_server.uri();
            let limiter = limiter.clone();
            task_pool.spawn(async move {
                let _permit = limiter.get_permit();
                get(if i % 2 == 0 { format!("{uri}/") } else { format!("{uri}/doesnotexist") })
                    .await
                    .with_context(|| "download error")?
                    .error_for_status()
                    .with_context(|| "status error")
                    .map(|_| ())
            });
        }

        assert!(task_pool.join(|| "error occurred").await.is_err());
    }

    #[test(tokio::test)]
    #[should_panic]
    async fn test_panic() {
        let mut task_pool = TaskPool::new();

        task_pool.spawn(async {
            panic!("foo");
        });
        let _ = task_pool.join(|| "baz").await;
    }

    #[test(tokio::test)]
    async fn test_abort() {
        let mutex = Arc::new(Mutex::new(0));
        let task_mutex = Arc::clone(&mutex);

        let _lock = mutex.lock().await;

        let mut task_pool = TaskPool::new();
        let abort_handle = task_pool.spawn(async move {
            let _lock = task_mutex.lock().await;
            unreachable!("should be cancelled before we reach this point");
        });

        abort_handle.abort();
        assert_matches!(task_pool.join(|| "foo").await, Err(err) => {
            assert_eq!("foo", err.to_string());
            assert_matches!(err.source(), Some(err) => {
                assert_matches!(err.downcast_ref::<MultiError>(), Some(multi_err) => {
                    assert!(!multi_err.to_string().is_empty());

                    assert_matches!(multi_err.errors().iter().exactly_one(), Ok(err) => {
                        assert_eq!("join error", err.to_string());

                        assert_matches!(err.source(), Some(err) => {
                            assert_matches!(err.downcast_ref::<JoinError>(), Some(join_error) => {
                                assert!(join_error.is_cancelled());
                            });
                        });
                    });
                });
            });
        });
    }
}
