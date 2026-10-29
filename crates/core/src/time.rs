use std::{fmt, str::FromStr, time::Duration};

use derive_more::{Display, Error, From};
use humantime::DurationError;
use serde::{Deserialize, Serialize};

/// The duration is outside the supported range.
#[derive(Debug, Display, Error)]
#[display("duration must be between 1s and 1000 years")]
pub struct DurationRangeError;

#[derive(Debug, Display, Error, From)]
pub enum ParseNonZeroDurationError {
    #[display("invalid duration: {_0}")]
    Syntax(DurationError),
    #[display("{_0}")]
    Range(DurationRangeError),
}

/// A configured span of time that is safe to use for expiry arithmetic.
///
/// The bounds are enforced once, at construction: at least one second (cookie `Max-Age`
/// has whole-second resolution, so anything shorter would round to an instant expiry) and
/// at most 1000 years, far beyond any sensible policy yet small enough that adding it to
/// the current time cannot overflow a timestamp. Consumers therefore need no further
/// checks. Parsed from and displayed as a human duration such as `30d` or `1h 30m`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NonZeroDuration(Duration);

impl NonZeroDuration {
    const MIN: Duration = Duration::from_secs(1);
    const MAX: Duration = Duration::from_secs(1000 * 365 * 24 * 60 * 60);

    pub fn get(self) -> Duration {
        self.0
    }
}

impl fmt::Display for NonZeroDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&humantime::format_duration(self.0), f)
    }
}

impl TryFrom<Duration> for NonZeroDuration {
    type Error = DurationRangeError;

    fn try_from(duration: Duration) -> Result<Self, Self::Error> {
        if !(Self::MIN..=Self::MAX).contains(&duration) {
            return Err(DurationRangeError);
        }

        Ok(Self(duration))
    }
}

impl FromStr for NonZeroDuration {
    type Err = ParseNonZeroDurationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(humantime::parse_duration(value)?.try_into()?)
    }
}

impl TryFrom<String> for NonZeroDuration {
    type Error = ParseNonZeroDurationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<NonZeroDuration> for String {
    fn from(duration: NonZeroDuration) -> Self {
        duration.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_human_durations() {
        assert_eq!(
            "1h 30m".parse::<NonZeroDuration>().unwrap().get(),
            Duration::from_secs(5400)
        );
        assert_eq!(
            "1500ms".parse::<NonZeroDuration>().unwrap().get(),
            Duration::from_millis(1500)
        );
    }

    #[test]
    fn rejects_unusable_durations() {
        for value in ["0s", "500ms", "999999999999999s", "-1s", "nonsense", ""] {
            assert!(value.parse::<NonZeroDuration>().is_err(), "{value}");
        }
    }

    #[test]
    fn displays_in_a_form_that_parses_back() {
        let duration: NonZeroDuration = "30d".parse().unwrap();

        assert_eq!(
            duration.to_string().parse::<NonZeroDuration>().unwrap(),
            duration
        );
    }
}
