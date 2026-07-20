use crate::store::StoreResult;
use crate::wave::Wave;

pub fn unix_to_datetime(seconds: i64) -> time::OffsetDateTime {
    time::OffsetDateTime::from_unix_timestamp(seconds).unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
}

pub fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

/// SELECT id, name, repo, created_at, parent_wave_id, promoted_at
pub fn map_wave_row(row: &rusqlite::Row<'_>) -> StoreResult<Wave> {
    Ok(Wave::from_stored_parts(
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        unix_to_datetime(row.get(3)?),
        row.get(4)?,
        row.get::<_, Option<i64>>(5)?.map(unix_to_datetime),
    ))
}
