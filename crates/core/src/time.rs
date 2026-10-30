use std::{fmt, str::FromStr, time::Duration};

use derive_more::{Display, Error, From};
use humantime::DurationError;
use serde::{Deserialize, Serialize, Serializer, ser::Error as _};

/// The duration is outside the supported range.
#[derive(Debug, Display, Error)]
#[display(
    "duration must be between {} and {}",
    humantime::format_duration(*min),
    humantime::format_duration(*max)
)]
pub struct DurationRangeError {
    min: Duration,
    max: Duration,
}

#[derive(Debug, Display, Error, From)]
pub enum ParseBoundedDurationError {
    #[display("invalid duration: {_0}")]
    Syntax(DurationError),
    #[display("{_0}")]
    Range(DurationRangeError),
}

/// Longest [`CookieDuration`]: far beyond any sensible policy, yet small enough that adding it
/// to the current time cannot overflow a timestamp.
const MAX_COOKIE_SECS: u64 = 1000 * 365 * 24 * 60 * 60;

/// A span of time that lasts at least `MIN` and at most `MAX` whole seconds.
///
/// The bounds are enforced once, at construction, so consumers need no further checks. Parsed
/// from and displayed as a human duration such as `30d` or `1h 30m`. Const generics cannot be
/// [`Duration`]s, which is why the bounds are expressed in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BoundedDuration<const MIN: u64, const MAX: u64>(Duration);

/// How long a cookie may live: at least one second, because `Max-Age` has whole-second
/// resolution and anything shorter would round to an instant expiry.
pub type CookieDuration = BoundedDuration<1, MAX_COOKIE_SECS>;

impl<const MIN: u64, const MAX: u64> BoundedDuration<MIN, MAX> {
    const MIN_DURATION: Duration = Duration::from_secs(MIN);
    const MAX_DURATION: Duration = Duration::from_secs(MAX);

    pub fn get(self) -> Duration {
        self.0
    }
}

impl<const MIN: u64, const MAX: u64> fmt::Display for BoundedDuration<MIN, MAX> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&humantime::format_duration(self.0), f)
    }
}

impl<const MIN: u64, const MAX: u64> TryFrom<Duration> for BoundedDuration<MIN, MAX> {
    type Error = DurationRangeError;

    fn try_from(duration: Duration) -> Result<Self, Self::Error> {
        if !(Self::MIN_DURATION..=Self::MAX_DURATION).contains(&duration) {
            return Err(DurationRangeError {
                min: Self::MIN_DURATION,
                max: Self::MAX_DURATION,
            });
        }

        Ok(Self(duration))
    }
}

impl<const MIN: u64, const MAX: u64> FromStr for BoundedDuration<MIN, MAX> {
    type Err = ParseBoundedDurationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(humantime::parse_duration(value)?.try_into()?)
    }
}

impl<const MIN: u64, const MAX: u64> TryFrom<String> for BoundedDuration<MIN, MAX> {
    type Error = ParseBoundedDurationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl<const MIN: u64, const MAX: u64> From<BoundedDuration<MIN, MAX>> for String {
    fn from(duration: BoundedDuration<MIN, MAX>) -> Self {
        duration.to_string()
    }
}

/// A duration that is serialized as an integer number of milliseconds, for clients that
/// consume durations as plain numbers. The conversion happens only on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, From)]
pub struct Millis(Duration);

impl Serialize for Millis {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let millis = u64::try_from(self.0.as_millis()).map_err(S::Error::custom)?;

        serializer.serialize_u64(millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_durations_as_milliseconds() {
        let json = serde_json::to_string(&Millis(Duration::from_millis(1500))).unwrap();

        assert_eq!(json, "1500");
    }

    #[test]
    fn parses_human_durations() {
        assert_eq!(
            "1h 30m".parse::<CookieDuration>().unwrap().get(),
            Duration::from_secs(5400)
        );
        assert_eq!(
            "1500ms".parse::<CookieDuration>().unwrap().get(),
            Duration::from_millis(1500)
        );
    }

    #[test]
    fn rejects_unusable_durations() {
        for value in ["0s", "500ms", "999999999999999s", "-1s", "nonsense", ""] {
            assert!(value.parse::<CookieDuration>().is_err(), "{value}");
        }
    }

    #[test]
    fn bounds_come_from_the_type_parameters() {
        type Short = BoundedDuration<2, 3>;

        assert!("2s".parse::<Short>().is_ok());
        assert!("3s".parse::<Short>().is_ok());
        assert!("1s".parse::<Short>().is_err());
        assert!("4s".parse::<Short>().is_err());
        assert_eq!(
            "4s".parse::<Short>().unwrap_err().to_string(),
            "duration must be between 2s and 3s"
        );
    }

    #[test]
    fn displays_in_a_form_that_parses_back() {
        let duration: CookieDuration = "30d".parse().unwrap();

        assert_eq!(
            duration.to_string().parse::<CookieDuration>().unwrap(),
            duration
        );
    }
}
