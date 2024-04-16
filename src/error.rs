//! Error types used by the [`exsb`](crate) program.

use std::fmt::{Display, Formatter};

use anyhow::Context;

/// Error type used by the [`exsb`](crate) program.
///
/// Currently mapped to [`anyhow::Error`].
pub type Error = anyhow::Error;

/// Result type used by the [`exsb`](crate) program.
///
/// Currently mapped to [`anyhow::Result`] in order to use our [`Error`] type.
pub type Result<T> = anyhow::Result<T>;

#[derive(Debug)]
pub(crate) struct MultiError(Vec<Error>);

impl MultiError {
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
        writeln!(f, "Multiple errors encountered:\n")?;
        self.0
            .iter()
            .enumerate()
            .try_fold((), |_, (i, error)| writeln!(f, "{i}: {error}\n"))
    }
}

impl std::error::Error for MultiError {}
