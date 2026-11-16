//! Const-generic bounded integer and trimmed-text value types.
//!
//! [`BoundedU8`] validates an unsigned byte in `MIN..=MAX` on construction and JSON
//! deserialization while encoding as PostgreSQL `SMALLINT` (`i16`) when the `sqlx` feature is
//! enabled. [`BoundedText`] validates character length in `MIN..=MAX` after trimming surrounding
//! whitespace, rejecting NUL bytes and enforcing a sealed [`LinePolicy`] ([`SingleLine`] or
//! [`MultiLine`]).

use std::{fmt, marker::PhantomData, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An integer outside `MIN..=MAX`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("{value} must be between {min} and {max}")]
pub struct BoundedU8Error {
    pub value: u8,
    pub min: u8,
    pub max: u8,
}

/// Text that contains forbidden control characters or whose trimmed length lies outside
/// `MIN..=MAX` Unicode scalar values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("text must be between {min} and {max} characters")]
pub struct BoundedTextError {
    pub min: usize,
    pub max: usize,
}

/// Unsigned integer validated into `MIN..=MAX`, stored as `i16` for PostgreSQL `SMALLINT`
/// compatibility.
///
/// ```
/// use pp_core::types::bounded::BoundedU8;
///
/// type Step = BoundedU8<0, 9>;
///
/// assert_eq!(Step::constant::<4>().get(), 4);
/// assert_eq!(Step::try_from(9).unwrap().get(), 9);
/// assert!(Step::try_from(10).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type), sqlx(transparent))]
pub struct BoundedU8<const MIN: u8, const MAX: u8>(i16);

impl<const MIN: u8, const MAX: u8> BoundedU8<MIN, MAX> {
    pub const MIN: Self = {
        assert!(MIN <= MAX);
        Self(MIN as i16)
    };

    pub const MAX: Self = {
        assert!(MIN <= MAX);
        Self(MAX as i16)
    };

    /// Constructs a compile-time verified constant within `MIN..=MAX`.
    pub const fn constant<const VALUE: u8>() -> Self {
        const {
            assert!(MIN <= MAX);
            assert!(VALUE >= MIN && VALUE <= MAX);
        }

        Self(VALUE as i16)
    }

    /// Validates that `value` lies within `MIN..=MAX`.
    pub const fn new(value: u8) -> Result<Self, BoundedU8Error> {
        const {
            assert!(MIN <= MAX);
        }

        if value < MIN || value > MAX {
            return Err(BoundedU8Error {
                value,
                min: MIN,
                max: MAX,
            });
        }

        Ok(Self(value as i16))
    }

    /// Returns the validated integer in `MIN..=MAX`.
    pub const fn get(self) -> u8 {
        self.0 as u8
    }
}

impl<const MIN: u8, const MAX: u8> fmt::Display for BoundedU8<MIN, MAX> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.get(), formatter)
    }
}

impl<const MIN: u8, const MAX: u8> TryFrom<u8> for BoundedU8<MIN, MAX> {
    type Error = BoundedU8Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<const MIN: u8, const MAX: u8> From<BoundedU8<MIN, MAX>> for u8 {
    fn from(bounded: BoundedU8<MIN, MAX>) -> Self {
        bounded.get()
    }
}

/// Line policy that rejects `\r`, `\n`, and `\0` before trimming surrounding whitespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SingleLine;

/// Line policy that rejects `\0` while preserving internal line breaks after trimming surrounding
/// whitespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiLine;

/// Trimmed UTF-8 text bounded to `MIN..=MAX` Unicode scalar values under [`LinePolicy`] `L`.
///
/// ```
/// use pp_core::types::bounded::{BoundedText, MultiLine, SingleLine};
///
/// type Subject = BoundedText<1, 20, SingleLine>;
/// type Notes = BoundedText<0, 20, MultiLine>;
///
/// assert_eq!("  Hello  ".parse::<Subject>().unwrap().as_str(), "Hello");
/// assert!("Hello\n".parse::<Subject>().is_err());
/// assert_eq!("   ".parse::<Notes>().unwrap().as_str(), "");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String", bound = "")]
pub struct BoundedText<const MIN: usize, const MAX: usize, L: LinePolicy = MultiLine>(
    String,
    PhantomData<fn() -> L>,
);

impl<const MIN: usize, const MAX: usize, L: LinePolicy> BoundedText<MIN, MAX, L> {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<const MIN: usize, const MAX: usize, L: LinePolicy> fmt::Display for BoundedText<MIN, MAX, L> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<const MIN: usize, const MAX: usize, L: LinePolicy> AsRef<str> for BoundedText<MIN, MAX, L> {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<const MIN: usize, const MAX: usize, L: LinePolicy> FromStr for BoundedText<MIN, MAX, L> {
    type Err = BoundedTextError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        const {
            assert!(MIN <= MAX);
        }

        if value.contains(L::FORBIDDEN) {
            return Err(BoundedTextError { min: MIN, max: MAX });
        }

        let trimmed = value.trim();

        if !(MIN..=MAX).contains(&trimmed.chars().count()) {
            return Err(BoundedTextError { min: MIN, max: MAX });
        }

        Ok(Self(trimmed.to_owned(), PhantomData))
    }
}

impl<const MIN: usize, const MAX: usize, L: LinePolicy> TryFrom<String>
    for BoundedText<MIN, MAX, L>
{
    type Error = BoundedTextError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl<const MIN: usize, const MAX: usize, L: LinePolicy> From<BoundedText<MIN, MAX, L>> for String {
    fn from(text: BoundedText<MIN, MAX, L>) -> Self {
        text.0
    }
}

#[cfg(feature = "sqlx")]
impl<const MIN: usize, const MAX: usize, L: LinePolicy, DB: sqlx::Database> sqlx::Type<DB>
    for BoundedText<MIN, MAX, L>
where
    String: sqlx::Type<DB>,
{
    fn type_info() -> DB::TypeInfo {
        <String as sqlx::Type<DB>>::type_info()
    }

    fn compatible(ty: &DB::TypeInfo) -> bool {
        <String as sqlx::Type<DB>>::compatible(ty)
    }
}

#[cfg(feature = "sqlx")]
impl<'q, const MIN: usize, const MAX: usize, L: LinePolicy, DB: sqlx::Database> sqlx::Encode<'q, DB>
    for BoundedText<MIN, MAX, L>
where
    String: sqlx::Encode<'q, DB>,
{
    fn encode_by_ref(
        &self,
        buf: &mut <DB as sqlx::Database>::ArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <String as sqlx::Encode<'q, DB>>::encode_by_ref(&self.0, buf)
    }

    fn size_hint(&self) -> usize {
        <String as sqlx::Encode<'q, DB>>::size_hint(&self.0)
    }
}

#[cfg(feature = "sqlx")]
impl<'r, const MIN: usize, const MAX: usize, L: LinePolicy, DB: sqlx::Database> sqlx::Decode<'r, DB>
    for BoundedText<MIN, MAX, L>
where
    String: sqlx::Decode<'r, DB>,
{
    fn decode(
        value: <DB as sqlx::Database>::ValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let raw = <String as sqlx::Decode<'r, DB>>::decode(value)?;

        Ok(Self::try_from(raw)?)
    }
}

mod private {
    pub trait Sealed {}
}

/// Sealed control-character policy for [`BoundedText`].
pub trait LinePolicy: private::Sealed + fmt::Debug + Clone + Copy + PartialEq + Eq {
    /// Characters rejected in the raw input before surrounding whitespace is trimmed.
    const FORBIDDEN: &'static [char];
}

impl private::Sealed for SingleLine {}

impl LinePolicy for SingleLine {
    const FORBIDDEN: &'static [char] = &['\r', '\n', '\0'];
}

impl private::Sealed for MultiLine {}

impl LinePolicy for MultiLine {
    const FORBIDDEN: &'static [char] = &['\0'];
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{BoundedText, BoundedTextError, BoundedU8, BoundedU8Error, MultiLine, SingleLine};

    type Digit = BoundedU8<1, 9>;
    type Subject = BoundedText<1, 5, SingleLine>;
    type Body = BoundedText<1, 5, MultiLine>;
    type OptionalNotes = BoundedText<0, 5, MultiLine>;

    #[test]
    fn bounded_u8_enforces_range_constants_and_json_round_trip() {
        assert_eq!(Digit::MIN.get(), 1);
        assert_eq!(Digit::MAX.get(), 9);
        assert_eq!(Digit::constant::<4>().get(), 4);

        assert_eq!(
            Digit::new(0),
            Err(BoundedU8Error {
                value: 0,
                min: 1,
                max: 9,
            })
        );
        assert_eq!(Digit::try_from(1).unwrap().get(), 1);
        assert_eq!(Digit::try_from(9).unwrap().get(), 9);
        assert_eq!(u8::from(Digit::constant::<7>()), 7);
        assert_eq!(
            Digit::try_from(10),
            Err(BoundedU8Error {
                value: 10,
                min: 1,
                max: 9,
            })
        );

        let parsed: Digit = serde_json::from_value(json!(5)).unwrap();

        assert_eq!(parsed.get(), 5);
        assert_eq!(serde_json::to_value(parsed).unwrap(), json!(5));
        assert!(serde_json::from_value::<Digit>(json!(0)).is_err());
        assert!(serde_json::from_value::<Digit>(json!(10)).is_err());
    }

    #[test]
    fn single_line_bounded_text_rejects_newlines_before_trim_and_nul() {
        let subject: Subject = "  élan  ".parse().unwrap();

        assert_eq!(subject.as_str(), "élan");
        assert_eq!(subject.as_ref(), "élan");
        assert_eq!(subject.to_string(), "élan");
        assert_eq!(String::from(subject.clone()), "élan");
        assert_eq!(serde_json::to_value(&subject).unwrap(), json!("élan"));
        assert_eq!(
            serde_json::from_value::<Subject>(json!("  élan  ")).unwrap(),
            subject
        );

        for invalid in ["", "   ", "hi\n", "\nhi", "hi\r", "h\ni", "hi\0", "abcdef"] {
            assert_eq!(
                invalid.parse::<Subject>(),
                Err(BoundedTextError { min: 1, max: 5 }),
                "{invalid:?}"
            );
        }
    }

    #[test]
    fn multi_line_bounded_text_trims_allows_internal_newlines_and_supports_zero_min() {
        let body: Body = "  hi\nmé  ".parse().unwrap();

        assert_eq!(body.as_str(), "hi\nmé");
        assert!("   \n\t  ".parse::<Body>().is_err());
        assert!("hi\0".parse::<Body>().is_err());
        assert!("123456".parse::<Body>().is_err());

        let blank_notes: OptionalNotes = "   \n  ".parse().unwrap();

        assert_eq!(blank_notes.as_str(), "");
        assert!("a\0b".parse::<OptionalNotes>().is_err());
        assert!("123456".parse::<OptionalNotes>().is_err());
    }
}
