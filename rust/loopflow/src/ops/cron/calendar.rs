use chrono::{Datelike, LocalResult, NaiveDate, TimeZone};

/// Calendar arithmetic shared by continuity and opportunity accounting.
pub(crate) fn at_or_before<T: TimeZone>(zone: &T, now: i64, hour: u32, minute: u32) -> Option<i64> {
    let mut date = zone.timestamp_opt(now, 0).single()?.date_naive();
    for _ in 0..370 {
        if let Some(timestamp) = on(zone, date, hour, minute) {
            if timestamp <= now {
                return Some(timestamp);
            }
        }
        date = date.pred_opt()?;
    }
    None
}

pub(crate) fn after<T: TimeZone>(zone: &T, timestamp: i64, hour: u32, minute: u32) -> Option<i64> {
    let mut date = zone.timestamp_opt(timestamp, 0).single()?.date_naive();
    for _ in 0..370 {
        date = date.succ_opt()?;
        if let Some(next) = on(zone, date, hour, minute) {
            if next > timestamp {
                return Some(next);
            }
        }
    }
    None
}

fn on<T: TimeZone>(zone: &T, date: NaiveDate, hour: u32, minute: u32) -> Option<i64> {
    match zone.with_ymd_and_hms(date.year(), date.month(), date.day(), hour, minute, 0) {
        LocalResult::Single(value) => Some(value.timestamp()),
        LocalResult::Ambiguous(first, _) => Some(first.timestamp()),
        LocalResult::None => None,
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use chrono_tz::America::Los_Angeles;

    use super::{after, at_or_before};

    #[test]
    fn daily_opportunities_follow_dst_without_duplicate_or_nonexistent_times() {
        let zone = Los_Angeles;
        let spring = zone
            .with_ymd_and_hms(2026, 3, 7, 2, 30, 0)
            .unwrap()
            .timestamp();
        let next = after(&zone, spring, 2, 30).unwrap();
        assert_eq!(
            next,
            zone.with_ymd_and_hms(2026, 3, 9, 2, 30, 0)
                .unwrap()
                .timestamp()
        );
        let fall = zone.with_ymd_and_hms(2026, 11, 1, 1, 30, 0);
        let first = fall.earliest().unwrap().timestamp();
        let second = fall.latest().unwrap().timestamp();
        assert_eq!(at_or_before(&zone, second, 1, 30), Some(first));
        assert!(after(&zone, first, 1, 30).unwrap() > second);
    }
}
