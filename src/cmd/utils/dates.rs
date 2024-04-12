use std::time::SystemTime;

use chrono::{DateTime, Utc};

pub fn to_datetime_utc(time: SystemTime) -> DateTime<Utc> {
    DateTime::from(time)
}
