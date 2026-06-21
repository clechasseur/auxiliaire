//! Duration types wrappers.

use std::fmt::{Display, Formatter};
use std::str::FromStr;

use jiff::SignedDuration;

/// Wrapper of [`SignedDuration`] whose [`Display`] implementation always returns
/// the [friendly format].
///
/// [friendly format]: https://docs.rs/jiff/0.2.28/jiff/struct.SignedDuration.html#parsing-and-printing
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FriendlyDuration(pub SignedDuration);

impl FromStr for FriendlyDuration {
    type Err = <SignedDuration as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let duration: SignedDuration = s.parse()?;
        Ok(Self(duration))
    }
}

impl Display for FriendlyDuration {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#}", self.0)
    }
}
