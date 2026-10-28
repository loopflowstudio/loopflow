//! Task owners' emails and the cached GitHub user behind each.
use std::collections::BTreeMap;

use rusqlite::{params, OptionalExtension};

use super::SqliteStore;
use crate::ops::people::{CachedGithubPerson, GithubPerson};
use crate::store::StoreResult;

impl SqliteStore {
    /// Every email that has owned one of `repo`'s Tasks, with the latest name
    /// its claims carried.
    pub(crate) fn task_owner_emails(
        &self,
        repo: &str,
    ) -> StoreResult<Vec<(String, Option<String>)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT o.person_key,o.person FROM task_ownership o
             JOIN tasks t ON t.id=o.task_id
             JOIN repository_plans w ON w.id=t.repository_id
             WHERE w.repo=?1 AND o.person_key IS NOT NULL ORDER BY o.at",
        )?;
        let rows = query.query_map([repo], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        let mut owners: BTreeMap<String, Option<String>> = BTreeMap::new();
        for row in rows {
            let (email, name) = row?;
            let known = owners.entry(email).or_default();
            if name.is_some() {
                *known = name;
            }
        }
        Ok(owners.into_iter().collect())
    }

    pub(crate) fn github_person(&self, email: &str) -> StoreResult<Option<CachedGithubPerson>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT login,name,avatar_url,checked_at FROM github_people WHERE email=?1",
            [email],
            |row| {
                let login: Option<String> = row.get(0)?;
                let name: Option<String> = row.get(1)?;
                let avatar_url: Option<String> = row.get(2)?;
                Ok(CachedGithubPerson {
                    person: login
                        .zip(avatar_url)
                        .map(|(login, avatar_url)| GithubPerson {
                            login,
                            name,
                            avatar_url,
                        }),
                    checked_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub(crate) fn record_github_person(
        &self,
        email: &str,
        person: Option<&GithubPerson>,
        checked_at: i64,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO github_people(email,login,name,avatar_url,checked_at)
             VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(email) DO UPDATE SET login=excluded.login,name=excluded.name,
             avatar_url=excluded.avatar_url,checked_at=excluded.checked_at",
            params![
                email,
                person.map(|person| &person.login),
                person.and_then(|person| person.name.as_ref()),
                person.map(|person| &person.avatar_url),
                checked_at
            ],
        )?;
        Ok(())
    }
}
