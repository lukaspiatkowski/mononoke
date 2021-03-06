// Copyright (c) 2018-present, Facebook, Inc.
// All Rights Reserved.
//
// This software may be used and distributed according to the terms of the
// GNU General Public License version 2 or any later version.

use std::fmt::{self, Display};

use chrono::{DateTime as ChronoDateTime, FixedOffset, LocalResult, TimeZone};
use quickcheck::{empty_shrinker, Arbitrary, Gen};

use errors::*;
use thrift;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DateTime(ChronoDateTime<FixedOffset>);

impl DateTime {
    #[inline]
    pub fn new(dt: ChronoDateTime<FixedOffset>) -> Self {
        DateTime(dt)
    }

    pub fn from_timestamp(secs: i64, tz_offset_secs: i32) -> Result<Self> {
        let tz = FixedOffset::west_opt(tz_offset_secs).ok_or_else(|| {
            ErrorKind::InvalidDateTime(format!("timezone offset out of range: {}", tz_offset_secs))
        })?;
        let dt = match tz.timestamp_opt(secs, 0) {
            LocalResult::Single(dt) => dt,
            _ => bail_err!(ErrorKind::InvalidDateTime(format!(
                "seconds out of range: {}",
                secs
            ))),
        };
        Ok(Self::new(dt))
    }

    pub(crate) fn from_thrift(dt: thrift::DateTime) -> Result<Self> {
        Self::from_timestamp(dt.timestamp_secs, dt.tz_offset_secs)
    }

    /// Retrieves the Unix timestamp in UTC.
    #[inline]
    pub fn timestamp_secs(&self) -> i64 {
        self.0.timestamp()
    }

    /// Retrieves the timezone offset, as represented by the number of seconds to
    /// add to convert local time to UTC.
    #[inline]
    pub fn tz_offset_secs(&self) -> i32 {
        // This is the same as the way Mercurial stores timezone offsets.
        self.0.offset().utc_minus_local()
    }

    #[inline]
    pub fn as_chrono(&self) -> &ChronoDateTime<FixedOffset> {
        &self.0
    }

    #[inline]
    pub fn into_chrono(self) -> ChronoDateTime<FixedOffset> {
        self.0
    }

    pub(crate) fn into_thrift(self) -> thrift::DateTime {
        thrift::DateTime {
            timestamp_secs: self.timestamp_secs(),
            tz_offset_secs: self.tz_offset_secs(),
        }
    }
}

impl Display for DateTime {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.0)
    }
}

impl Arbitrary for DateTime {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        // Ensure a large domain from which to get second values.
        let secs = g.gen_range(i32::min_value(), i32::max_value()) as i64;
        // Timezone offsets in the range [-86399, 86399] (both inclusive) are valid.
        // gen_range generates a value in the range [low, high).
        let tz_offset_secs = g.gen_range(-86_399, 86_400);
        DateTime::from_timestamp(secs, tz_offset_secs)
            .expect("Arbitrary instances should always be valid")
    }

    fn shrink(&self) -> Box<Iterator<Item = Self>> {
        empty_shrinker()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    quickcheck! {
        fn thrift_roundtrip(dt: DateTime) -> bool {
            let thrift_dt = dt.into_thrift();
            let dt2 = DateTime::from_thrift(thrift_dt)
                .expect("roundtrip instances should always be valid");
            // Equality on DateTime structs doesn't pay attention to the time zone,
            // in order to be consistent with Ord.
            dt == dt2 && dt.tz_offset_secs() == dt2.tz_offset_secs()
        }
    }

    #[test]
    fn bad_inputs() {
        DateTime::from_timestamp(0, 86_400)
            .expect_err("unexpected OK - tz_offset_secs out of bounds");
        DateTime::from_timestamp(0, -86_400)
            .expect_err("unexpected OK - tz_offset_secs out of bounds");
        DateTime::from_timestamp(i64::min_value(), 0)
            .expect_err("unexpected OK - timestamp_secs out of bounds");
        DateTime::from_timestamp(i64::max_value(), 0)
            .expect_err("unexpected OK - timestamp_secs out of bounds");
    }

    #[test]
    fn bad_thrift() {
        DateTime::from_thrift(thrift::DateTime {
            timestamp_secs: 0,
            tz_offset_secs: 86_400,
        }).expect_err("unexpected OK - tz_offset_secs out of bounds");
        DateTime::from_thrift(thrift::DateTime {
            timestamp_secs: 0,
            tz_offset_secs: -86_400,
        }).expect_err("unexpected OK - tz_offset_secs out of bounds");
        DateTime::from_thrift(thrift::DateTime {
            timestamp_secs: i64::min_value(),
            tz_offset_secs: 0,
        }).expect_err("unexpected OK - timestamp_secs out of bounds");
        DateTime::from_thrift(thrift::DateTime {
            timestamp_secs: i64::max_value(),
            tz_offset_secs: 0,
        }).expect_err("unexpected OK - timestamp_secs out of bounds");
    }
}
