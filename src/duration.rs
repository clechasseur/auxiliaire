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

#[cfg(test)]
mod tests {
    use std::ops::Add;

    use assert_matches::assert_matches;

    use super::*;

    mod friendly_duration {
        use super::*;

        mod from_str {
            use super::*;

            #[test]
            fn valid() {
                let duration = "12s".parse::<FriendlyDuration>();
                assert_matches!(duration, Ok(FriendlyDuration(duration)) => {
                    assert_eq!(SignedDuration::from_secs(12), duration);
                });
            }

            #[test]
            fn invalid() {
                let duration = "fourty-two".parse::<FriendlyDuration>();
                assert!(duration.is_err());
            }
        }
    }

    mod display {
        use super::*;

        #[test]
        fn friendly() {
            let duration =
                FriendlyDuration(SignedDuration::from_mins(2).add(SignedDuration::from_secs(3)));
            assert_eq!("2m 3s", duration.to_string());
        }
    }
}
