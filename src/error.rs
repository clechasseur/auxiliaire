//! Error types used by the [`auxiliaire`](crate) program.

use std::error::Error as StdError;
use std::fmt::{Display, Formatter};

use anyhow::Context;
use anyhow::Error as AnyhowError;
use anyhow::Result as AnyhowResult;

/// Error type used by the [`auxiliaire`](crate) program.
///
/// Currently mapped to [`anyhow::Error`].
pub type Error = AnyhowError;

/// Result type used by the [`auxiliaire`](crate) program.
///
/// Currently mapped to [`anyhow::Result`] in order to use our [`Error`] type.
pub type Result<T> = AnyhowResult<T>;

#[derive(Debug)]
pub(crate) struct MultiError(Vec<Error>);

impl MultiError {
    #[must_use]
    #[allow(dead_code)] // Only used in tests currently
    pub fn errors(&self) -> &[Error] {
        &self.0
    }

    pub fn check<C, F>(errors: Vec<Error>, context: F) -> Result<()>
    where
        F: FnOnce() -> C,
        C: Display + Send + Sync + 'static,
    {
        errors
            .is_empty()
            .then_some(())
            .ok_or_else(|| MultiError(errors))
            .with_context(context)
    }
}

impl Display for MultiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "multiple errors encountered:\n")?;
        self.0
            .iter()
            .enumerate()
            .try_fold((), |_, (i, error)| writeln!(f, "{i}: {error:?}\n"))
    }
}

impl StdError for MultiError {}

#[cfg(test)]
mod tests {
    mod multi_error {
        mod errors {
            use anyhow::anyhow;
            use assert_matches::assert_matches;

            use crate::error::MultiError;

            #[test]
            fn test_errors() {
                let errors = vec![anyhow!("foo"), anyhow!("bar")];
                let error = MultiError::check(errors, || "baz").unwrap_err();

                assert_matches!(error.source(), Some(err) => {
                    assert_matches!(err.downcast_ref::<MultiError>(), Some(multi_err) => {
                        assert_eq!(2, multi_err.errors().len());
                        assert_eq!("foo", multi_err.errors()[0].to_string());
                        assert_eq!("bar", multi_err.errors()[1].to_string());
                    });
                });
            }
        }

        mod check {
            use anyhow::anyhow;
            use assert_matches::assert_matches;

            use crate::error::MultiError;
            use crate::Error;

            #[test]
            fn no_error() {
                let errors: Vec<Error> = vec![];

                assert!(MultiError::check(errors, || "foo").is_ok());
            }

            #[test]
            fn some_errors() {
                let errors = vec![anyhow!("foo"), anyhow!("bar")];
                let result = MultiError::check(errors, || "baz");

                assert_matches!(result, Err(err) => {
                    assert!(!err.to_string().is_empty());

                    let mut source = err.source();
                    while let Some(err) = source {
                        assert!(!err.to_string().is_empty());
                        source = err.source();
                    }
                });
            }
        }
    }
}
