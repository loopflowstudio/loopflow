use crate::store::StoreResult;
use crate::wave::Wave;

pub fn unix_to_datetime(seconds: i64) -> time::OffsetDateTime {
    time::OffsetDateTime::from_unix_timestamp(seconds).unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
}

pub fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

/// SELECT id, name, repo, created_at, parent_wave_id
pub fn map_wave_row(row: &rusqlite::Row<'_>) -> StoreResult<Wave> {
    Ok(Wave {
        id: row.get(0)?,
        name: row.get(1)?,
        repo: row.get(2)?,
        created_at: Some(unix_to_datetime(row.get(3)?)),
        parent_wave_id: row.get(4)?,
    })
}
