//! Bitemporal clocks: valid time is not record time.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use thiserror::Error;
use time::OffsetDateTime;
use time::UtcOffset;
use time::format_description::well_known::Rfc3339;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TimeError {
    #[error("invalid RFC3339 timestamp: {0}")]
    InvalidRfc3339(String),
    #[error("record time may not precede system receipt")]
    RecordBeforeReceipt,
    #[error("interval lower bound is not <= upper bound")]
    InvertedInterval,
}

/// An instant on the legal timeline. Stored and serialized as RFC3339 UTC (`Z`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(OffsetDateTime);

fn as_utc(dt: OffsetDateTime) -> OffsetDateTime {
    dt.to_offset(UtcOffset::UTC)
}

impl Instant {
    pub fn from_offset(dt: OffsetDateTime) -> Self {
        Self(as_utc(dt))
    }

    pub fn parse(text: &str) -> Result<Self, TimeError> {
        OffsetDateTime::parse(text, &Rfc3339)
            .map(Self::from_offset)
            .map_err(|_| TimeError::InvalidRfc3339(text.to_owned()))
    }

    pub fn as_offset(self) -> OffsetDateTime {
        self.0
    }

    pub fn to_rfc3339(self) -> String {
        self.0.format(&Rfc3339).expect("OffsetDateTime formats")
    }
}

impl Serialize for Instant {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_rfc3339())
    }
}

impl<'de> Deserialize<'de> for Instant {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Instant::parse(&text).map_err(serde::de::Error::custom)
    }
}

impl fmt::Debug for Instant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_rfc3339())
    }
}

impl fmt::Display for Instant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_rfc3339())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Bound {
    Inclusive(Instant),
    Exclusive(Instant),
    PosInf,
    NegInf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Interval {
    pub start: Bound,
    pub end: Bound,
}

impl Interval {
    pub fn always() -> Self {
        Self {
            start: Bound::NegInf,
            end: Bound::PosInf,
        }
    }

    pub fn from_instants(start: Instant, end: Option<Instant>) -> Result<Self, TimeError> {
        if let Some(end) = end
            && start > end
        {
            return Err(TimeError::InvertedInterval);
        }
        Ok(Self {
            start: Bound::Inclusive(start),
            end: match end {
                Some(e) => Bound::Exclusive(e),
                None => Bound::PosInf,
            },
        })
    }

    pub fn contains(self, t: Instant) -> bool {
        let after_start = match self.start {
            Bound::Inclusive(s) => t >= s,
            Bound::Exclusive(s) => t > s,
            Bound::NegInf => true,
            Bound::PosInf => false,
        };
        let before_end = match self.end {
            Bound::Inclusive(e) => t <= e,
            Bound::Exclusive(e) => t < e,
            Bound::PosInf => true,
            Bound::NegInf => false,
        };
        after_start && before_end
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalendarKind {
    Days,
    WorkingDays,
    CountedDays,
    CalendarDays,
    Hours,
    Minutes,
    Seconds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FidrynDuration {
    pub amount: i64,
    pub kind: CalendarKind,
}

/// Both clocks every rule, handler, and trace node observes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunContext {
    pub valid_time: Instant,
    pub record_time: Instant,
}

impl RunContext {
    pub fn new(valid_time: Instant, record_time: Instant) -> Self {
        Self {
            valid_time,
            record_time,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TemporalLens {
    pub valid_time: Instant,
    pub record_time: Instant,
}

impl From<RunContext> for TemporalLens {
    fn from(ctx: RunContext) -> Self {
        Self {
            valid_time: ctx.valid_time,
            record_time: ctx.record_time,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_contains_start() {
        let start = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let end = Instant::parse("2034-01-01T00:00:00Z").unwrap();
        let iv = Interval::from_instants(start, Some(end)).unwrap();
        assert!(iv.contains(start));
        assert!(!iv.contains(end));
    }

    #[test]
    fn working_days_are_not_days() {
        let a = FidrynDuration {
            amount: 20,
            kind: CalendarKind::CountedDays,
        };
        let b = FidrynDuration {
            amount: 20,
            kind: CalendarKind::WorkingDays,
        };
        assert_ne!(a, b);
    }

    #[test]
    fn equal_instants_have_one_canonical_wire_form() {
        let utc = Instant::parse("2020-01-01T00:00:00Z").unwrap();
        let offset = Instant::parse("2020-01-01T01:00:00+01:00").unwrap();
        let plus_zero = Instant::parse("2020-01-01T00:00:00+00:00").unwrap();
        assert_eq!(utc, offset);
        assert_eq!(utc, plus_zero);
        let expected = "\"2020-01-01T00:00:00Z\"";
        assert_eq!(serde_json::to_string(&utc).unwrap(), expected);
        assert_eq!(serde_json::to_string(&offset).unwrap(), expected);
        assert_eq!(serde_json::to_string(&plus_zero).unwrap(), expected);
        assert_eq!(utc.to_rfc3339(), "2020-01-01T00:00:00Z");
        assert_eq!(offset.to_rfc3339(), "2020-01-01T00:00:00Z");
    }
}
