use chrono::{Datelike, FixedOffset, Local, TimeZone, Timelike, Utc};
use once_cell::sync::OnceCell;

use crate::lsp_typst_boundary::TypstDatetime;

#[derive(Debug, Default)]
pub struct Now {
    now: OnceCell<chrono::DateTime<Utc>>,
}

impl Now {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn date_with_typst_offset(&self, offset: Option<i64>) -> Option<TypstDatetime> {
        let tz = TypstTz::from_typst_offset(offset)?;
        let now = self.chrono_now();
        let datetime = now.with_timezone(&tz).naive_local();
        chrono_to_typst_datetime_only_date(datetime)
    }

    pub fn datetime(&self) -> Option<TypstDatetime> {
        let now = self.chrono_now();
        let datetime = now.naive_utc();
        chrono_to_typst_datetime(datetime)
    }

    fn chrono_now(&self) -> &chrono::DateTime<Utc> {
        self.now.get_or_init(Utc::now)
    }
}

fn chrono_to_typst_datetime_only_date(
    chrono_datetime: chrono::NaiveDateTime,
) -> Option<TypstDatetime> {
    TypstDatetime::from_ymd(
        chrono_datetime.year(),
        chrono_datetime.month() as u8,
        chrono_datetime.day() as u8,
    )
}

fn chrono_to_typst_datetime(chrono_datetime: chrono::NaiveDateTime) -> Option<TypstDatetime> {
    TypstDatetime::from_ymd_hms(
        chrono_datetime.year(),
        chrono_datetime.month() as u8,
        chrono_datetime.day() as u8,
        chrono_datetime.hour() as u8,
        chrono_datetime.minute() as u8,
        chrono_datetime.second() as u8,
    )
}

/// Could be the local timezone (whatever it happens to be on the user's system) or a timezone with
/// a known, fixed offset from UTC
#[derive(Debug, Clone, Copy)]
enum TypstTz {
    Local(Local),
    FixedOffset(FixedOffset),
}

impl TypstTz {
    pub fn from_typst_offset(offset: Option<i64>) -> Option<Self> {
        match offset {
            Some(offset) => Self::from_offset(offset),
            None => Some(Self::local()),
        }
    }

    /// Create a timezone with given UTC offset in hours, if the offset is within bounds
    pub fn from_offset(offset: i64) -> Option<Self> {
        const SECS_PER_HOUR: i32 = 60 * 60;
        FixedOffset::east_opt(offset as i32 * SECS_PER_HOUR).map(Self::FixedOffset)
    }

    pub fn local() -> Self {
        Self::Local(Local)
    }
}

impl TimeZone for TypstTz {
    type Offset = FixedOffset;

    fn from_offset(offset: &Self::Offset) -> Self {
        Self::FixedOffset(*offset)
    }

    fn offset_from_local_date(
        &self,
        local: &chrono::NaiveDate,
    ) -> chrono::LocalResult<Self::Offset> {
        match self {
            Self::Local(inner) => inner.offset_from_local_date(local),
            Self::FixedOffset(inner) => inner.offset_from_local_date(local),
        }
    }

    fn offset_from_local_datetime(
        &self,
        local: &chrono::NaiveDateTime,
    ) -> chrono::LocalResult<Self::Offset> {
        match self {
            Self::Local(inner) => inner.offset_from_local_datetime(local),
            Self::FixedOffset(inner) => inner.offset_from_local_datetime(local),
        }
    }

    fn offset_from_utc_date(&self, utc: &chrono::NaiveDate) -> Self::Offset {
        match self {
            Self::Local(inner) => inner.offset_from_utc_date(utc),
            Self::FixedOffset(inner) => inner.offset_from_utc_date(utc),
        }
    }

    fn offset_from_utc_datetime(&self, utc: &chrono::NaiveDateTime) -> Self::Offset {
        match self {
            Self::Local(inner) => inner.offset_from_utc_datetime(utc),
            Self::FixedOffset(inner) => inner.offset_from_utc_datetime(utc),
        }
    }
}
