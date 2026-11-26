//! Git carries a plan without Linear or Asana. Each Machine pushes its whole
//! plan to a ref of its own on the repository's remote
//! (`refs/loopflow/plan/<machine id>`, one stamped record per file) and reads
//! the others' from `refs/loopflow/plan-seen/*`. It decides nothing: arriving
//! records go through the one import, where the newest stamp wins.
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};

use loopflow_model::durable::RepositoryId;
use loopflow_storage::sqlite::plan_carriers::{Carrier, NotTaken};
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::edit::StampedRecord;

/// This Machine's ref, here and on the remote.
const OWN: &str = "refs/loopflow/plan/";
/// The remote's refs as last fetched, this Machine's own among them.
const SEEN: &str = "refs/loopflow/plan-seen/";
/// The id its Machine knows the repository by, which every other clone
/// records as naming its own checkout.
const REPOSITORY: &str = "repository";
/// Bytes of records read from the store at a time while a commit is built.
const PAGE: usize = 1 << 20;
/// Base retry wait after a repository's background exchange fails; doubles up to an hour.
const BACKOFF_BASE: Duration = Duration::from_secs(120);
const BACKOFF_MAX: Duration = Duration::from_secs(60 * 60);
/// Cap on cached verified `(commit_oid, repository_id)` pairs.
const MAX_VERIFIED_TIPS: usize = 4096;

#[derive(Debug, Clone, Copy)]
struct RepoBackoff {
    failures: u32,
    next_attempt: Instant,
}

impl RepoBackoff {
    fn delay(failures: u32) -> Duration {
        let shift = failures.saturating_sub(1).min(16);
        BACKOFF_BASE.saturating_mul(1 << shift).min(BACKOFF_MAX)
    }
}

static BACKOFFS: LazyLock<Mutex<HashMap<String, RepoBackoff>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Immutable `(commit_oid, repository_id)` pairs whose `repository` blob has
/// already been verified to match `repository_id`.
static VERIFIED_TIP_REPOS: LazyLock<Mutex<HashSet<(String, String)>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

pub(super) fn is_backed_off(repo: &str, now: Instant) -> bool {
    let backoffs = BACKOFFS.lock().expect("plan-git backoff lock");
    is_backed_off_in(&backoffs, repo, now)
}

fn is_backed_off_in(backoffs: &HashMap<String, RepoBackoff>, repo: &str, now: Instant) -> bool {
    backoffs
        .get(repo)
        .is_some_and(|state| now < state.next_attempt)
}

fn clear_backoff(repo: &str) {
    BACKOFFS.lock().expect("plan-git backoff lock").remove(repo);
}

fn record_backoff_in(
    backoffs: &mut HashMap<String, RepoBackoff>,
    repo: &str,
    now: Instant,
    result: &Result<()>,
) {
    match result {
        Ok(()) => {
            backoffs.remove(repo);
        }
        Err(error) => {
            let failures = backoffs
                .get(repo)
                .map_or(1, |state| state.failures.saturating_add(1));
            let wait = RepoBackoff::delay(failures);
            backoffs.insert(
                repo.to_owned(),
                RepoBackoff {
                    failures,
                    next_attempt: now + wait,
                },
            );
            tracing::debug!(
                %repo,
                failures,
                retry_in_secs = wait.as_secs(),
                error = %format!("{error:#}"),
                "Git plan exchange failed"
            );
        }
    }
}

/// The remote a repository's plan travels by: `origin`, else the first by
/// name. `None` for a repository with no remote. Parses `.git/config` directly
/// via `observe::derived` so ordinary repositories never fork `git remote`,
/// falling back to `git remote` only for `[include]`/`[includeIf]`,
/// `extensions.worktreeConfig`, or legacy `.git/remotes`/`.git/branches`.
pub fn remote(repo: &Path) -> Option<String> {
    if !repo.is_dir() {
        return None;
    }
    if crate::git::read_layout(repo).is_none() {
        crate::observe::record_spawn();
        return remote_by_git(crate::git::git_stdout(repo, &["remote"]).ok());
    }
    let key = crate::observe::Derived::GitRemote {
        repo: repo.to_owned(),
    };
    crate::observe::derived(key, || {
        if let Some(layout) = crate::git::read_layout(repo) {
            let config = crate::observe::read_to_string(&layout.common_dir.join("config"));
            let has_remotes = has_legacy_remote_dir(&layout.common_dir.join("remotes"));
            let has_branches = has_legacy_remote_dir(&layout.common_dir.join("branches"));
            if !has_remotes && !has_branches {
                if let Ok(config) = config {
                    if let ConfigRemotes::Parsed(remote) = parse_config_remotes(&config) {
                        return remote;
                    }
                }
            }
        }
        crate::observe::record_spawn();
        remote_by_git(crate::git::git_stdout_inheriting(repo, &["remote"], &|_| {}).ok())
    })
    .as_ref()
    .clone()
}

fn has_legacy_remote_dir(dir: &Path) -> bool {
    crate::observe::read_dir(dir).is_ok_and(|entries| !entries.is_empty())
}

#[derive(Debug, PartialEq, Eq)]
enum ConfigRemotes {
    Parsed(Option<String>),
    Fallback,
}

fn parse_config_remotes(config: &str) -> ConfigRemotes {
    let mut remotes = BTreeSet::new();
    let mut current_remote: Option<String> = None;
    let mut in_extensions = false;
    let mut in_continuation = false;

    for raw_line in config.lines() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if in_continuation {
            in_continuation = ends_with_odd_backslashes(strip_unquoted_comment(line));
            continue;
        }
        let mut rest = line.trim_start_matches([' ', '\t']);
        if rest.is_empty() || rest.starts_with('#') || rest.starts_with(';') {
            continue;
        }
        if rest.starts_with('[') {
            let Some((section, subsection, trailing)) = parse_section_header(rest) else {
                return ConfigRemotes::Fallback;
            };
            let section = section.to_ascii_lowercase();
            if section == "include" || section == "includeif" {
                return ConfigRemotes::Fallback;
            }
            in_extensions = section == "extensions";
            if section == "remote" {
                match subsection {
                    Some(name) if !name.is_empty() => current_remote = Some(name),
                    Some(_) => return ConfigRemotes::Fallback,
                    None => current_remote = None,
                }
            } else {
                current_remote = None;
            }
            rest = trailing.trim_start_matches([' ', '\t']);
            if rest.is_empty() || rest.starts_with('#') || rest.starts_with(';') {
                continue;
            }
        }
        let effective = strip_unquoted_comment(rest).trim_end_matches([' ', '\t']);
        if !effective.is_empty() {
            let key = effective.split(['=', ' ', '\t']).next().unwrap_or_default();
            if !key.as_bytes().first().is_some_and(u8::is_ascii_alphabetic) {
                return ConfigRemotes::Fallback;
            }
            if in_extensions && key.eq_ignore_ascii_case("worktreeconfig") {
                return ConfigRemotes::Fallback;
            }
            if let Some(name) = &current_remote {
                remotes.insert(name.clone());
            }
        }
        in_continuation = ends_with_odd_backslashes(strip_unquoted_comment(rest));
    }

    let selected = if remotes.contains("origin") {
        Some("origin".to_owned())
    } else {
        remotes.into_iter().next()
    };
    ConfigRemotes::Parsed(selected)
}

/// Parse a Git config section header (`[section]`, `[section "subsection"]`, or
/// legacy `[section.subsection]`), returning `(section, subsection, trailing)`.
fn parse_section_header(line: &str) -> Option<(&str, Option<String>, &str)> {
    let body = line.strip_prefix('[')?;
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
        i += 1;
    }
    if i == 0 || i >= bytes.len() {
        return None;
    }
    let section = &body[..i];
    match bytes[i] {
        b']' => Some((section, None, &body[i + 1..])),
        b'.' => {
            let rest = &body[i + 1..];
            let close = rest.find(']')?;
            let sub = &rest[..close];
            if sub.is_empty() {
                return None;
            }
            Some((section, Some(sub.to_ascii_lowercase()), &rest[close + 1..]))
        }
        b' ' | b'\t' => {
            while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') {
                i += 1;
            }
            if i >= bytes.len() {
                return None;
            }
            if bytes[i] == b']' {
                return Some((section, None, &body[i + 1..]));
            }
            if bytes[i] != b'"' {
                return None;
            }
            i += 1;
            let mut sub = String::new();
            let mut chars = body[i..].char_indices();
            let mut closed_at = None;
            while let Some((offset, ch)) = chars.next() {
                match ch {
                    '\\' => {
                        let (_, escaped) = chars.next()?;
                        sub.push(escaped);
                    }
                    '"' => {
                        closed_at = Some(i + offset + 1);
                        break;
                    }
                    _ => sub.push(ch),
                }
            }
            let mut after = closed_at?;
            while after < bytes.len() && matches!(bytes[after], b' ' | b'\t') {
                after += 1;
            }
            if bytes.get(after) != Some(&b']') {
                return None;
            }
            Some((section, Some(sub), &body[after + 1..]))
        }
        _ => None,
    }
}

fn strip_unquoted_comment(line: &str) -> &str {
    let mut in_quotes = false;
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => in_quotes = !in_quotes,
            '#' | ';' if !in_quotes => return &line[..idx],
            _ => {}
        }
    }
    line
}

fn ends_with_odd_backslashes(line: &str) -> bool {
    line.bytes().rev().take_while(|b| *b == b'\\').count() % 2 == 1
}

fn remote_by_git(remotes: Option<String>) -> Option<String> {
    let remotes = remotes?;
    let mut remotes: Vec<&str> = remotes.lines().filter(|name| !name.is_empty()).collect();
    remotes.sort_unstable();
    remotes
        .iter()
        .find(|name| **name == "origin")
        .or(remotes.first())
        .map(|name| (*name).to_owned())
}

/// The minute exchange covers existing repositories that have an unfinished
/// Task or a non-retired Wave on this Machine (and no active local claim,
/// which the shared owner poll already covers). A repository whose exchange
/// fails doubles its wait up to an hour.
pub fn exchange_unowned(store: &SqliteStore) -> Result<()> {
    exchange_unowned_at(store, Instant::now(), &BACKOFFS, |repo| {
        exchange(store, repo, None)
    })
}

/// Background exchange for an owned repository: shares `BACKOFFS` with
/// [`exchange_unowned`] so unreachable remotes back off exponentially.
pub(super) fn exchange_background(
    store: &SqliteStore,
    repo: &str,
    until: Option<Instant>,
) -> Result<()> {
    exchange_background_at(store, repo, until, Instant::now())
}

pub(super) fn exchange_background_at(
    store: &SqliteStore,
    repo: &str,
    until: Option<Instant>,
    now: Instant,
) -> Result<()> {
    if is_backed_off(repo, now) {
        return Ok(());
    }
    let result = exchange(store, repo, until);
    record_backoff_in(
        &mut BACKOFFS.lock().expect("plan-git backoff lock"),
        repo,
        now,
        &result,
    );
    result
}

fn exchange_unowned_at(
    store: &SqliteStore,
    now: Instant,
    backoffs: &Mutex<HashMap<String, RepoBackoff>>,
    mut run_exchange: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    let owned = super::plan_carriers::owned_repositories(store)?;
    for path in store.active_plan_repository_paths()? {
        let repo = path.to_string_lossy();
        if owned.contains(repo.as_ref()) || !path.is_dir() || crate::ops::tracker::connected(&repo)
        {
            continue;
        }
        if is_backed_off_in(&backoffs.lock().expect("plan-git backoff lock"), &repo, now) {
            continue;
        }
        let result = run_exchange(&repo);
        record_backoff_in(
            &mut backoffs.lock().expect("plan-git backoff lock"),
            &repo,
            now,
            &result,
        );
    }
    Ok(())
}

/// [`exchange`] for a clone this Machine has saved nothing in and never read
/// a plan for, which registers it for [`exchange_unowned`]. `false` when the
/// Machine already knows the clone, or it has no remote.
pub fn first_exchange(store: &SqliteStore, repo: &str, until: Instant) -> Result<bool> {
    if store.repository_id(repo)?.is_some() || remote(Path::new(repo)).is_none() {
        return Ok(false);
    }
    exchange(store, repo, Some(until)).map(|()| true)
}

/// Fetch every Machine's plan ref, take in what changed in the others', then
/// commit the records changed here onto this Machine's ref and push it. The
/// outcome is recorded for `lf repo tracker`. A repository with no remote
/// does nothing. With `until`, Git is stopped then and the next exchange
/// carries what this one did not. A ref holding a record this Machine could
/// not write is read from the same place next time, and that is the error.
pub fn exchange(store: &SqliteStore, repo: &str, until: Option<Instant>) -> Result<()> {
    let Some(remote) = remote(Path::new(repo)) else {
        return Ok(());
    };
    // A clone that has saved nothing still reads the plan its remote carries.
    let repository = crate::repository::location::ensure_repository(store, repo)?;
    turn(store, repo, &remote, &repository, until, |sent_seq| {
        exchange_with(store, repo, &remote, &repository, sent_seq, until)
    })
}

/// Push the records changed here onto this Machine's ref as last fetched,
/// fetching nothing, and give up at `until`. A push the remote refuses (its
/// copy of the ref is ahead of the one fetched) is the error, and the next
/// [`exchange`] sends what this did not. While something is not taken the
/// save may be what lets it arrive (a Wave renamed), so this reads as well;
/// what is still not taken stays on the line and is no error of the save.
pub fn send(store: &SqliteStore, repo: &str, until: Instant) -> Result<()> {
    let path = Path::new(repo);
    let Some(remote) = remote(path) else {
        return Ok(());
    };
    let Some(repository) = store.repository_id(repo)? else {
        return Ok(());
    };
    turn(store, repo, &remote, &repository, Some(until), |sent_seq| {
        let waiting = store
            .plan_carrier(repo, Carrier::Git)?
            .is_some_and(|held| held.not_taken.is_some());
        if waiting {
            return exchange_with(store, repo, &remote, &repository, sent_seq, Some(until))
                .or_else(|error| error.downcast::<NotTaken>().map(drop));
        }
        let me = crate::store::machines::local_machine(store)?.id;
        let mine = seen_own_ref(path, me.as_str(), Some(until))?;
        publish(
            store,
            path,
            &remote,
            me.as_str(),
            &repository,
            sent_seq,
            mine.as_deref(),
            Some(until),
        )
    })
}

/// One Git exchange per repository at a time, its outcome recorded for
/// `lf repo tracker`; the next reads what this one wrote.
fn turn(
    store: &SqliteStore,
    repo: &str,
    remote: &str,
    repository: &RepositoryId,
    until: Option<Instant>,
    carry: impl FnOnce(i64) -> Result<()>,
) -> Result<()> {
    let _turn = super::plan_carriers::lock(store, "plan-git", repository, until)?;
    let sent_seq = store.select_carrier(repo, Carrier::Git, remote)?.sent_seq;
    let result = carry(sent_seq);
    if result.is_ok() {
        clear_backoff(repo);
    }
    let error = super::plan_carriers::carrier_error(&result);
    store.record_carrier_exchange(repo, Carrier::Git, error.as_deref())?;
    result
}

fn exchange_with(
    store: &SqliteStore,
    repo: &str,
    remote: &str,
    repository: &RepositoryId,
    sent_seq: i64,
    until: Option<Instant>,
) -> Result<()> {
    let path = Path::new(repo);
    git_until(
        path,
        &[
            "fetch",
            "--quiet",
            "--no-tags",
            "--prune",
            "--no-write-fetch-head",
            "--no-recurse-submodules",
            remote,
            &format!("+{OWN}*:{SEEN}*"),
        ],
        None,
        until,
    )?;
    let me = crate::store::machines::local_machine(store)?.id;
    let seen = seen_refs(path, until)?;
    let mine = seen.get(me.as_str());

    // A ref that cannot be read does not stop the others or this Machine's push.
    let mut failed = None;
    let mut not_taken = Vec::new();
    let taken = store.plan_ref_tips(repo)?;
    let names = crate::store::ownership::machine_names(store)?;
    for (machine, tip) in &seen {
        if machine == me.as_str() || taken.get(machine) == Some(tip) {
            continue;
        }
        let read = (|| {
            let base = taken
                .get(machine)
                .filter(|base| commit_exists(path, base, until))
                .or(mine);
            let (records, theirs) = changed_records(path, base.map(String::as_str), tip, until)?;
            let skipped = store.apply_plan_records(repo, &records, None, &names)?;
            // A ref with anything not taken is read from the same place again.
            if skipped.is_empty() {
                store.record_plan_ref(repo, machine, tip)?;
            }
            Ok::<_, anyhow::Error>((theirs, skipped))
        })();
        match read {
            Ok((theirs, skipped)) => {
                not_taken.extend(skipped);
                // A hub or a move that names the repository by that Machine's
                // id finds this checkout. Tasks keep the id they were saved under.
                if let Some(theirs) = theirs {
                    if !store.locate_repository(repo, &theirs)? {
                        tracing::warn!(%repo, %theirs, "another checkout here already answers to that repository id")
                    }
                }
            }
            Err(error) => {
                failed.get_or_insert(error.context(format!("read the plan ref of {machine}")));
            }
        }
    }
    store.record_not_taken(repo, Carrier::Git, &not_taken)?;
    publish(
        store,
        path,
        remote,
        me.as_str(),
        repository,
        sent_seq,
        mine.map(String::as_str),
        until,
    )?;
    failed.map_or(Ok(()), Err)?;
    NotTaken::of(&not_taken).map_or(Ok(()), |not_taken| Err(not_taken.into()))
}

/// This Machine's own fetched plan ref tip, when `send` runs without `seen_refs`.
fn seen_own_ref(repo: &Path, me: &str, until: Option<Instant>) -> Result<Option<String>> {
    let tip = git_until(
        repo,
        &[
            "for-each-ref",
            "--format=%(objectname)",
            &format!("{SEEN}{me}"),
        ],
        None,
        until,
    )?;
    let tip = String::from_utf8_lossy(&tip);
    let tip = tip.trim();
    Ok((!tip.is_empty()).then(|| tip.to_owned()))
}

/// Each fetched plan ref's Machine and commit.
fn seen_refs(repo: &Path, until: Option<Instant>) -> Result<HashMap<String, String>> {
    let listed = git_until(
        repo,
        &["for-each-ref", "--format=%(refname) %(objectname)", SEEN],
        None,
        until,
    )?;
    Ok(String::from_utf8_lossy(&listed)
        .lines()
        .filter_map(|line| line.strip_prefix(SEEN)?.split_once(' '))
        .map(|(machine, tip)| (machine.to_owned(), tip.to_owned()))
        .collect())
}

fn commit_exists(repo: &Path, commit: &str, until: Option<Instant>) -> bool {
    git_until(
        repo,
        &["cat-file", "-e", &format!("{commit}^{{commit}}")],
        None,
        until,
    )
    .is_ok()
}

/// The records whose files differ between `base` (nothing, when `None`) and
/// `tip`, and the repository id `tip` names when that changed too.
fn changed_records(
    repo: &Path,
    base: Option<&str>,
    tip: &str,
    until: Option<Instant>,
) -> Result<(Vec<StampedRecord>, Option<RepositoryId>)> {
    let empty;
    let base = match base {
        Some(base) => base,
        None => {
            let hashed = git_until(
                repo,
                &["hash-object", "-t", "tree", "--stdin"],
                Some(b""),
                until,
            )?;
            empty = String::from_utf8_lossy(&hashed).trim().to_owned();
            &empty
        }
    };
    let diff = git_until(
        repo,
        &[
            "diff-tree",
            "-r",
            "-z",
            "--raw",
            "--no-abbrev",
            "--no-renames",
            "--diff-filter=AM",
            base,
            tip,
        ],
        None,
        until,
    )?;
    // `:<mode> <mode> <old> <new> <status>\0<path>\0`, repeated.
    let mut parts = diff.split(|byte| *byte == 0);
    let mut files = Vec::new();
    while let (Some(change), Some(file)) = (parts.next(), parts.next()) {
        let change = String::from_utf8_lossy(change);
        let Some(blob) = change.split(' ').nth(3) else {
            continue;
        };
        files.push((blob.to_owned(), String::from_utf8_lossy(file).into_owned()));
    }
    let wanted = files
        .iter()
        .map(|(blob, _)| format!("{blob}\n"))
        .collect::<String>();
    let blobs = git_until(
        repo,
        &["cat-file", "--batch"],
        Some(wanted.as_bytes()),
        until,
    )?;
    let mut rest = blobs.as_slice();
    let (mut records, mut repository) = (Vec::new(), None);
    for (_, file) in &files {
        let content = next_blob(&mut rest).context("read a plan ref's files")?;
        if file == REPOSITORY {
            repository = RepositoryId::parse(String::from_utf8_lossy(content).trim()).ok();
        } else if let Some(record) = parse_record(file, content) {
            records.push(record);
        }
    }
    Ok((records, repository))
}

/// One `<object> <type> <size>\n<content>\n` of `git cat-file --batch`.
fn next_blob<'a>(rest: &mut &'a [u8]) -> Result<&'a [u8]> {
    let end = rest
        .iter()
        .position(|byte| *byte == b'\n')
        .ok_or_else(|| anyhow!("Git answered fewer objects than asked for"))?;
    let header = String::from_utf8_lossy(&rest[..end]).into_owned();
    let size: usize = header
        .rsplit(' ')
        .next()
        .and_then(|size| size.parse().ok())
        .ok_or_else(|| anyhow!("Git could not read an object: {header}"))?;
    let content = rest
        .get(end + 1..end + 1 + size)
        .ok_or_else(|| anyhow!("Git cut an object short"))?;
    *rest = rest.get(end + 2 + size..).unwrap_or_default();
    Ok(content)
}

/// A plan ref file as a record. A file that does not parse is skipped and
/// logged.
fn parse_record(file: &str, content: &[u8]) -> Option<StampedRecord> {
    serde_json::from_slice(content)
        .inspect_err(|error| {
            tracing::warn!(%file, %error, "skipped a plan ref file that does not parse");
        })
        .ok()
}

/// Where a record's file lives in a plan ref; `None` for an id that cannot
/// name a file.
fn record_path(record: &StampedRecord) -> Option<String> {
    let id = &record.id;
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return None;
    }
    // Ids are `<kind>_<hex>`: spread a directory by the first hex byte.
    let unique = id.rsplit('_').next().unwrap_or(id);
    let shard = unique.get(..2).unwrap_or("00");
    Some(match record.table.as_str() {
        "waves" => format!("waves/{id}.json"),
        "projects" => format!("projects/{id}.json"),
        "tasks" => format!("tasks/{shard}/{id}.json"),
        "task_comments" => format!("comments/{shard}/{id}.json"),
        _ => return None,
    })
}

fn tip_names_repository(
    repo: &Path,
    commit: &str,
    repository: &RepositoryId,
    until: Option<Instant>,
) -> bool {
    let key = (commit.to_owned(), repository.as_str().to_owned());
    if VERIFIED_TIP_REPOS
        .lock()
        .expect("verified tip cache lock")
        .contains(&key)
    {
        return true;
    }
    let matches = git_until(
        repo,
        &["cat-file", "-p", &format!("{commit}:{REPOSITORY}")],
        None,
        until,
    )
    .is_ok_and(|held| String::from_utf8_lossy(&held).trim() == repository.as_str());
    if matches {
        let mut cache = VERIFIED_TIP_REPOS.lock().expect("verified tip cache lock");
        if cache.len() >= MAX_VERIFIED_TIPS {
            cache.clear();
        }
        cache.insert(key);
    }
    matches
}

/// Commit every record changed here since `sent_seq` onto this Machine's ref
/// as last fetched from the remote, and push, giving up at `until`. A remote
/// without this Machine's ref is given the whole plan. Nothing is forced: a
/// push the remote refuses is the error.
#[allow(clippy::too_many_arguments)]
fn publish(
    store: &SqliteStore,
    repo: &Path,
    remote: &str,
    me: &str,
    repository: &RepositoryId,
    sent_seq: i64,
    mine: Option<&str>,
    until: Option<Instant>,
) -> Result<()> {
    let key = repo.to_string_lossy();
    let mut since = if mine.is_some() { sent_seq } else { 0 };
    let mut files = Vec::new();
    loop {
        let page = store.plan_records_since(&key, since, PAGE)?;
        for record in &page.records {
            let Some(path) = record_path(record) else {
                tracing::warn!(id = %record.id, "a record whose id cannot name a file stays out of the plan ref");
                continue;
            };
            file(&mut files, &path, &serde_json::to_vec(record)?);
        }
        since = page.seq;
        if !page.more {
            break;
        }
    }
    if files.is_empty() && mine.is_none_or(|tip| tip_names_repository(repo, tip, repository, until))
    {
        store.plan_sent(&key, Carrier::Git, since)?;
        return Ok(());
    }
    let own = format!("{OWN}{me}");
    let mut stream = format!(
        "commit {own}\ncommitter {me} <loopflow@localhost> {} +0000\ndata 4\nplan\n",
        time::OffsetDateTime::now_utc().unix_timestamp()
    )
    .into_bytes();
    if let Some(mine) = mine {
        stream.extend_from_slice(format!("from {mine}\n").as_bytes());
    }
    file(&mut stream, REPOSITORY, repository.as_str().as_bytes());
    stream.extend_from_slice(&files);
    // A commit left by a push that failed is replaced, never built on.
    git_until(
        repo,
        &["fast-import", "--quiet", "--force"],
        Some(&stream),
        until,
    )?;
    let tree = |commit: &str| {
        git_until(
            repo,
            &["rev-parse", &format!("{commit}^{{tree}}")],
            None,
            until,
        )
    };
    let changed = match mine {
        Some(mine) => tree(mine)? != tree(&own)?,
        None => true,
    };
    if changed {
        git_until(
            repo,
            &[
                "push",
                "--quiet",
                "--no-verify",
                remote,
                &format!("{own}:{own}"),
            ],
            None,
            until,
        )?;
        git_until(
            repo,
            &["update-ref", &format!("{SEEN}{me}"), &own],
            None,
            until,
        )?;
    }
    store.plan_sent(&key, Carrier::Git, since)?;
    Ok(())
}

/// One file of a `git fast-import` commit.
fn file(stream: &mut Vec<u8>, path: &str, content: &[u8]) {
    stream
        .extend_from_slice(format!("M 100644 inline {path}\ndata {}\n", content.len()).as_bytes());
    stream.extend_from_slice(content);
    stream.push(b'\n');
}

/// Set `O_NONBLOCK` on a pipe descriptor so `poll` can multiplex stdin, stdout,
/// and stderr on the calling thread without spawning per-pipe reader threads.
fn set_nonblocking(fd: std::os::fd::RawFd) -> std::io::Result<()> {
    // SAFETY: `fd` is an open pipe descriptor owned by the caller.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: setting `O_NONBLOCK` on an open pipe fd is safe.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Drain a non-blocking pipe until `WouldBlock` or EOF, dropping it on EOF/error.
fn drain_pipe(pipe: &mut Option<impl Read>, out: &mut Vec<u8>, buf: &mut [u8]) {
    let Some(reader) = pipe.as_mut() else {
        return;
    };
    loop {
        match reader.read(buf) {
            Ok(0) => {
                *pipe = None;
                break;
            }
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                *pipe = None;
                break;
            }
        }
    }
}

/// Run Git without a terminal, stopping its process group at the deadline.
/// Multiplexes stdin/stdout/stderr with `poll(2)` so no extra threads are spawned.
fn git_until(
    repo: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    until: Option<Instant>,
) -> Result<Vec<u8>> {
    if until.is_some_and(|deadline| Instant::now() >= deadline) {
        bail!("git {} did not answer in time", args[0]);
    }
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if until.is_some() {
        // Its own group, so stopping it stops its transport helpers too.
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
    }
    crate::observe::record_spawn();
    let mut child = command.spawn().context("start git")?;
    let mut stdin = child.stdin.take();
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    if let Some(ref pipe) = stdin {
        set_nonblocking(pipe.as_raw_fd()).context("configure git stdin nonblocking")?;
    }
    if let Some(ref pipe) = stdout {
        set_nonblocking(pipe.as_raw_fd()).context("configure git stdout nonblocking")?;
    }
    if let Some(ref pipe) = stderr {
        set_nonblocking(pipe.as_raw_fd()).context("configure git stderr nonblocking")?;
    }

    let input_bytes = input.unwrap_or(&[]);
    let mut written = 0usize;
    if written >= input_bytes.len() {
        stdin = None;
    }
    let mut out_bytes = Vec::new();
    let mut err_bytes = Vec::new();
    let mut buf = [0u8; 8192];
    let mut timed_out = false;

    while stdin.is_some() || stdout.is_some() || stderr.is_some() {
        let timeout_ms = match until {
            Some(deadline) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    timed_out = true;
                    break;
                }
                remaining.as_millis().min(i32::MAX as u128) as libc::c_int
            }
            None => -1,
        };
        let mut fds = [libc::pollfd {
            fd: -1,
            events: 0,
            revents: 0,
        }; 3];
        let mut nfds = 0usize;
        let mut push_fd = |fd: std::os::fd::RawFd, events: libc::c_short| {
            let idx = nfds;
            fds[idx] = libc::pollfd {
                fd,
                events,
                revents: 0,
            };
            nfds += 1;
            idx
        };
        let stdin_idx = stdin
            .as_ref()
            .map(|p| push_fd(p.as_raw_fd(), libc::POLLOUT));
        let stdout_idx = stdout
            .as_ref()
            .map(|p| push_fd(p.as_raw_fd(), libc::POLLIN));
        let stderr_idx = stderr
            .as_ref()
            .map(|p| push_fd(p.as_raw_fd(), libc::POLLIN));

        // SAFETY: `fds[..nfds]` contains valid open file descriptors.
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), nfds as libc::nfds_t, timeout_ms) };
        if ready < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err.into());
        }
        if ready == 0 {
            timed_out = true;
            break;
        }

        if let (Some(idx), Some(pipe)) = (stdin_idx, stdin.as_mut()) {
            let revents = fds[idx].revents;
            if revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                stdin = None;
            } else if revents & libc::POLLOUT != 0 {
                loop {
                    match pipe.write(&input_bytes[written..]) {
                        Ok(0) => {
                            stdin = None;
                            break;
                        }
                        Ok(n) => {
                            written += n;
                            if written >= input_bytes.len() {
                                stdin = None;
                                break;
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => {
                            stdin = None;
                            break;
                        }
                    }
                }
            }
        }
        if stdout_idx.is_some_and(|idx| fds[idx].revents != 0) {
            drain_pipe(&mut stdout, &mut out_bytes, &mut buf);
        }
        if stderr_idx.is_some_and(|idx| fds[idx].revents != 0) {
            drain_pipe(&mut stderr, &mut err_bytes, &mut buf);
        }
    }

    if timed_out {
        drop(stdin);
        drop(stdout);
        drop(stderr);
        // SAFETY: the child is unreaped, so its id still names the group it
        // leads; signalling a group has no memory effects.
        unsafe { libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL) };
        let _ = child.wait();
        bail!("git {} did not answer in time", args[0]);
    }
    let status = child.wait()?;
    if status.success() {
        return Ok(out_bytes);
    }
    let stderr = String::from_utf8_lossy(&err_bytes);
    let reason = stderr
        .lines()
        .find(|line| line.starts_with("fatal:") || line.starts_with("error:"))
        .or_else(|| stderr.lines().rfind(|line| !line.trim().is_empty()));
    match reason {
        Some(reason) => bail!("{}", reason.trim()),
        None => bail!("git {} failed with {}", args[0], status),
    }
}

#[cfg(test)]
mod tests {
    use super::{exchange, first_exchange, remote, send};
    use loopflow_model::durable::RepositoryId;
    use loopflow_storage::sqlite::plan_carriers::Carrier;
    use loopflow_storage::sqlite::SqliteStore;
    use loopflow_storage::work::task::{NewTask, TaskUpdate};
    use loopflow_test_support::TestRepo;
    use std::path::{Path, PathBuf};

    fn git(dir: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// A Machine: its own store and its own clone of `origin`.
    struct Machine {
        store: SqliteStore,
        repo: String,
    }

    impl Machine {
        fn new(root: &Path, name: &str, origin: &Path) -> Self {
            let clone = root.join(name).join("clone");
            std::fs::create_dir_all(clone.parent().unwrap()).unwrap();
            git(
                root,
                &[
                    "clone",
                    "-q",
                    origin.to_str().unwrap(),
                    clone.to_str().unwrap(),
                ],
            );
            Self::at(root, name, clone)
        }

        fn at(root: &Path, name: &str, clone: PathBuf) -> Self {
            let store = crate::store::opening::open_ephemeral(&root.join(name).join("loopflow.db"))
                .unwrap();
            Self {
                store,
                repo: clone.canonicalize().unwrap().to_str().unwrap().to_owned(),
            }
        }

        fn task(&self, title: &str) -> loopflow_model::durable::TaskId {
            let project =
                crate::test_support::seed_wave_project(&self.store, &self.repo, "plan").unwrap();
            self.store
                .create_task(&NewTask {
                    parent_task_id: None,
                    repository_id: crate::repository::location::ensure_repository(
                        &self.store,
                        &self.repo,
                    )
                    .unwrap(),
                    id: loopflow_model::durable::TaskId::new(),
                    project_id: Some(project.id),
                    title: title.into(),
                    description: String::new(),
                    due_date: None,
                })
                .unwrap()
                .id
        }

        fn retitle(&self, task: &loopflow_model::durable::TaskId, title: &str) {
            let revision = self.store.task(task).unwrap().unwrap().revision;
            self.store
                .edit_task(
                    task,
                    revision,
                    &TaskUpdate {
                        title: Some(title.into()),
                        ..Default::default()
                    },
                )
                .unwrap();
        }

        fn title(&self, task: &loopflow_model::durable::TaskId) -> Option<String> {
            self.store.task(task).unwrap().map(|task| task.title)
        }

        fn exchange(&self) {
            exchange(&self.store, &self.repo, None).unwrap();
        }

        /// What follows a save: a push, no fetch.
        fn send(&self) -> anyhow::Result<()> {
            let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
            send(&self.store, &self.repo, until)
        }

        /// Returns once the wall clock is past every stamp this Machine holds.
        fn wait_past_stamps(&self) {
            let held = self
                .store
                .plan_records_since(&self.repo, 0, usize::MAX)
                .unwrap()
                .records
                .iter()
                .flat_map(|record| record.fields.iter().map(|field| field.at))
                .max()
                .unwrap();
            let now = || time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
            while now() <= i128::from(held) {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }

        fn unsent(&self) -> usize {
            self.store
                .plan_unsent(&self.repo, Carrier::Git, 0)
                .unwrap()
                .count
        }

        fn repository(&self) -> RepositoryId {
            self.store.repository_id(&self.repo).unwrap().unwrap()
        }

        /// The titles `lf task list` shows in this clone.
        fn listed(&self) -> Vec<String> {
            self.store
                .open_tasks_matching(&self.repository(), "", None, None)
                .unwrap()
                .into_iter()
                .map(|task| task.title)
                .collect()
        }

        /// The checkout this Machine finds for a repository id.
        fn checkout_of(&self, repository: &RepositoryId) -> Option<String> {
            self.store.repository_path(repository).unwrap()
        }
    }

    fn machines() -> (tempfile::TempDir, TestRepo, Machine, Machine) {
        let root = tempfile::tempdir().unwrap();
        let origin = TestRepo::new();
        let laptop = Machine::new(root.path(), "laptop", origin.bare_path());
        let mini = Machine::new(root.path(), "mini", origin.bare_path());
        (root, origin, laptop, mini)
    }

    #[tokio::test]
    async fn the_minute_git_exchange_yields_claimed_work_to_the_shared_owner_poll() {
        let (_root, _origin, sender, owner) = machines();
        let task = sender.task("Before the claim");
        sender.exchange();
        // Register the checkout with an active Wave before its first minute tick.
        crate::repository::location::ensure_repository(&owner.store, &owner.repo).unwrap();
        crate::test_support::seed_wave_project(&owner.store, &owner.repo, "other").unwrap();
        super::exchange_unowned(&owner.store).unwrap();
        assert_eq!(owner.title(&task).as_deref(), Some("Before the claim"));
        git(
            Path::new(&owner.repo),
            &["config", "user.email", "owner@example.test"],
        );
        crate::store::ownership::claim_task(&owner.store, &task).unwrap();

        sender.retitle(&task, "First edit");
        sender.exchange();
        super::exchange_unowned(&owner.store).unwrap();
        assert_eq!(owner.title(&task).as_deref(), Some("Before the claim"));
        let store = crate::store::Store {
            sqlite: owner.store.clone(),
        };
        crate::ops::tracker_events::check_owned_work(&store, Some(&task))
            .await
            .unwrap();
        assert_eq!(owner.title(&task).as_deref(), Some("First edit"));
        sender.retitle(&task, "Second edit");
        sender.exchange();
        // The server shares the Graph's attempt; the next due read catches up.
        crate::ops::tracker_events::check_owned_work(&store, None)
            .await
            .unwrap();
        assert_eq!(owner.title(&task).as_deref(), Some("First edit"));
        crate::ops::plan_carriers::poll_owned(&store, &owner.repo, 0)
            .await
            .unwrap();
        assert_eq!(owner.title(&task).as_deref(), Some("Second edit"));
    }

    #[test]
    fn a_task_made_on_one_clone_is_on_another_and_the_later_title_wins_on_both() {
        let (_root, origin, laptop, mini) = machines();
        let task = laptop.task("x");
        assert!(laptop.unsent() > 0);
        laptop.exchange();
        assert_eq!(laptop.unsent(), 0);
        // The plan stays out of what a person sees of the repository.
        assert!(!git(origin.bare_path(), &["branch", "-a"]).contains("loopflow"));
        assert!(!git(Path::new(&laptop.repo), &["branch", "-a"]).contains("loopflow"));

        // mini was never told about the laptop: the remote is what they share,
        // and the first read of a fresh clone fetches it, once.
        let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
        assert!(first_exchange(&mini.store, &mini.repo, until).unwrap());
        assert!(!first_exchange(&mini.store, &mini.repo, until).unwrap());
        assert_eq!(mini.title(&task).as_deref(), Some("x"));
        assert_eq!(mini.listed(), ["x"]);
        // Each clone keeps its own repository id and answers to the other's,
        // so a hub or a move finds the checkout and no saved Task changes hands.
        mini.task("made on mini");
        mini.exchange();
        laptop.exchange();
        assert_eq!(laptop.listed().len(), 2);
        assert_eq!(mini.listed().len(), 2);
        assert_eq!(
            laptop.checkout_of(&mini.repository()).as_deref(),
            Some(laptop.repo.as_str())
        );
        assert_eq!(
            mini.checkout_of(&laptop.repository()).as_deref(),
            Some(mini.repo.as_str())
        );
        assert_eq!(laptop.title(&task).as_deref(), Some("x"));

        mini.retitle(&task, "one");
        // Later means a later millisecond: within one, the Machine id decides.
        mini.wait_past_stamps();
        laptop.retitle(&task, "two");
        mini.exchange();
        laptop.exchange();
        mini.exchange();
        assert_eq!(laptop.title(&task).as_deref(), Some("two"));
        assert_eq!(mini.title(&task).as_deref(), Some("two"));

        // Once both hold the same plan an exchange commits nothing more.
        laptop.exchange();
        let refs = || git(origin.bare_path(), &["for-each-ref", "refs/loopflow/plan"]);
        let settled = refs();
        assert_eq!(settled.lines().count(), 2, "{settled}");
        mini.exchange();
        laptop.exchange();
        assert_eq!(refs(), settled);
        assert_eq!((laptop.unsent(), mini.unsent()), (0, 0));
    }

    #[test]
    fn an_exchange_with_an_end_is_stopped_at_it() {
        let (_root, _origin, laptop, mini) = machines();
        let task = laptop.task("before a claim");
        let unsent = laptop.unsent();
        let now = Some(std::time::Instant::now());
        let error = exchange(&laptop.store, &laptop.repo, now).unwrap_err();
        assert_eq!(format!("{error:#}"), "git fetch did not answer in time");
        assert_eq!(laptop.unsent(), unsent);
        // The next exchange carries what that one did not.
        laptop.exchange();
        mini.exchange();
        assert_eq!(mini.title(&task).as_deref(), Some("before a claim"));
    }

    #[test]
    fn a_save_is_pushed_at_once_without_a_fetch() {
        let (_root, origin, laptop, mini) = machines();
        let refs = || git(origin.bare_path(), &["for-each-ref", "refs/loopflow/plan"]);
        let first = laptop.task("first");
        laptop.send().unwrap();
        assert_eq!(refs().lines().count(), 1);
        assert_eq!(laptop.unsent(), 0);
        // The next save builds on the ref as pushed; nothing was fetched.
        let second = laptop.task("second");
        laptop.send().unwrap();
        mini.exchange();
        assert_eq!(mini.title(&first).as_deref(), Some("first"));
        assert_eq!(mini.title(&second).as_deref(), Some("second"));
        // A send reads nothing: mini's Task waits for an exchange.
        let theirs = mini.task("made on mini");
        mini.send().unwrap();
        laptop.task("third");
        laptop.send().unwrap();
        assert_eq!(laptop.title(&theirs), None);

        // With the remote gone the save stays unsent, with Git's reason.
        let away = origin.bare_path().with_extension("away");
        std::fs::rename(origin.bare_path(), &away).unwrap();
        laptop.task("made offline");
        let error = laptop.send().unwrap_err();
        assert!(format!("{error:#}").starts_with("fatal:"), "{error:#}");
        assert_eq!(laptop.unsent(), 1);
        std::fs::rename(&away, origin.bare_path()).unwrap();
        // An expired send leaves the cursor where it was.
        let error = send(&laptop.store, &laptop.repo, std::time::Instant::now()).unwrap_err();
        assert!(
            format!("{error:#}").ends_with("did not answer in time"),
            "{error:#}"
        );
        assert_eq!(laptop.unsent(), 1);

        // A push the remote refuses, its copy of the ref being ahead of the
        // one this clone holds, is left for the next exchange.
        let me = crate::store::machines::local_machine(&laptop.store)
            .unwrap()
            .id;
        git(
            Path::new(&laptop.repo),
            &["update-ref", "-d", &format!("refs/loopflow/plan-seen/{me}")],
        );
        laptop.send().unwrap_err();
        assert_eq!(laptop.unsent(), 1);
        laptop.exchange();
        assert_eq!(laptop.unsent(), 0);
        assert_eq!(laptop.title(&theirs).as_deref(), Some("made on mini"));
    }

    #[test]
    fn an_unreachable_remote_keeps_the_change_unsent_with_gits_reason() {
        let (_root, origin, laptop, mini) = machines();
        laptop.task("first");
        laptop.exchange();
        let away = origin.bare_path().with_extension("away");
        std::fs::rename(origin.bare_path(), &away).unwrap();
        let task = laptop.task("made offline");
        let error = exchange(&laptop.store, &laptop.repo, None).unwrap_err();
        assert!(format!("{error:#}").starts_with("fatal:"), "{error:#}");
        let held = laptop
            .store
            .plan_carrier(&laptop.repo, Carrier::Git)
            .unwrap()
            .unwrap();
        assert_eq!(held.peer, "origin");
        assert!(held.failing_since.is_some());
        assert_eq!(held.error.as_deref(), Some(format!("{error:#}").as_str()));
        assert_eq!(laptop.unsent(), 1);

        std::fs::rename(&away, origin.bare_path()).unwrap();
        laptop.exchange();
        let held = laptop
            .store
            .plan_carrier(&laptop.repo, Carrier::Git)
            .unwrap()
            .unwrap();
        assert_eq!((held.error, held.failing_since), (None, None));
        assert_eq!(laptop.unsent(), 0);
        mini.exchange();
        assert_eq!(mini.title(&task).as_deref(), Some("made offline"));
    }

    #[test]
    fn a_file_or_field_this_build_cannot_read_is_skipped_and_the_rest_arrives() {
        let (root, origin, laptop, mini) = machines();
        let task = laptop.task("readable");
        laptop.exchange();
        // Another Machine's ref: the laptop's records, one with a field from a
        // later build, beside a file that is not a record.
        let me = git(
            origin.bare_path(),
            &["for-each-ref", "--format=%(refname)", "refs/loopflow/plan"],
        );
        let scratch = root.path().join("scratch");
        git(
            root.path(),
            &[
                "clone",
                "-q",
                origin.bare_path().to_str().unwrap(),
                scratch.to_str().unwrap(),
            ],
        );
        git(&scratch, &["fetch", "-q", "origin", me.trim()]);
        git(&scratch, &["checkout", "-q", "FETCH_HEAD"]);
        let file = scratch
            .join("tasks")
            .join(&task.as_str()["task_".len()..][..2])
            .join(format!("{task}.json"));
        let mut record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        for field in record["fields"].as_array_mut().unwrap() {
            if field["field"] == "task_title" {
                field["value"] = "from a later build".into();
                field["at"] = serde_json::json!(field["at"].as_i64().unwrap() + 1);
            }
        }
        record["fields"].as_array_mut().unwrap().push(
            serde_json::json!({"field":"task_mood","value":"sunny","at":1,"machine":"home_later"}),
        );
        std::fs::write(&file, serde_json::to_vec(&record).unwrap()).unwrap();
        std::fs::write(scratch.join("waves/notes.json"), "not a record").unwrap();
        git(&scratch, &["add", "-A"]);
        git(
            &scratch,
            &[
                "-c",
                "user.name=later",
                "-c",
                "user.email=later@localhost",
                "commit",
                "-qm",
                "plan",
            ],
        );
        git(
            &scratch,
            &["push", "-q", "origin", "HEAD:refs/loopflow/plan/home_later"],
        );

        mini.exchange();
        assert_eq!(mini.title(&task).as_deref(), Some("from a later build"));
    }

    #[test]
    fn a_wave_made_on_two_machines_is_not_taken_until_one_is_renamed() {
        let (_root, _origin, laptop, mini) = machines();
        // Each made a Wave called "plan" before reading the other's.
        let mine = laptop.task("on the laptop");
        let theirs = mini.task("on mini");
        laptop.exchange();
        let not_taken = |machine: &Machine| {
            let held = machine
                .store
                .plan_carrier(&machine.repo, Carrier::Git)
                .unwrap()
                .unwrap();
            // The remote itself worked.
            assert_eq!((held.error, held.failing_since), (None, None));
            held.not_taken
        };
        // The laptop's Wave, its Project and its Task, on the line and as
        // the exchange's error; mini's own plan still went out.
        let error = exchange(&mini.store, &mini.repo, None).unwrap_err();
        let held = not_taken(&mini).unwrap();
        assert_eq!(error.to_string(), held.to_string());
        assert_eq!(held.count, 3);
        assert!(
            held.reason.starts_with("wave plan from ")
                && held.reason.contains("`lf wave rename plan <new name>`"),
            "{held}"
        );
        assert_eq!(mini.unsent(), 0);
        exchange(&laptop.store, &laptop.repo, None).unwrap_err();
        assert!(not_taken(&laptop).is_some());
        assert_eq!(laptop.title(&theirs), None);

        // One is renamed: each reads the other's ref again and writes it.
        let project = laptop.store.task(&mine).unwrap().unwrap().project_id;
        let wave = laptop
            .store
            .project(&project.unwrap())
            .unwrap()
            .unwrap()
            .wave_id;
        loopflow_storage::test_fixture::arrive(
            &laptop.store,
            &laptop.repo,
            &loopflow_storage::work::edit::WorkEdit::for_target(
                wave.as_str(),
                &[loopflow_storage::work::edit::WorkValue::WaveName(
                    "laptop-plan".into(),
                )],
            ),
        )
        .unwrap();
        // The save that renamed it reads too, so the line is right at once.
        laptop.send().unwrap();
        assert_eq!(laptop.title(&theirs).as_deref(), Some("on mini"));
        assert_eq!(not_taken(&laptop), None);
        mini.exchange();
        assert_eq!(mini.title(&mine).as_deref(), Some("on the laptop"));
        assert_eq!(not_taken(&mini), None);
    }

    #[test]
    fn a_repository_with_no_remote_writes_no_ref() {
        let root = tempfile::tempdir().unwrap();
        let clone = root.path().join("alone");
        std::fs::create_dir_all(&clone).unwrap();
        git(&clone, &["init", "-q"]);
        let alone = Machine::at(root.path(), "store", clone);
        alone.task("here");
        assert_eq!(remote(Path::new(&alone.repo)), None);
        alone.exchange();
        assert_eq!(git(Path::new(&alone.repo), &["for-each-ref"]), "");
        assert!(alone
            .store
            .plan_carrier(&alone.repo, Carrier::Git)
            .unwrap()
            .is_none());
    }

    #[test]
    fn a_remote_added_after_the_first_lookup_is_seen_at_once() {
        let root = tempfile::tempdir().unwrap();
        let clone = root.path().join("clone");
        std::fs::create_dir_all(&clone).unwrap();
        git(&clone, &["init", "-q"]);
        assert_eq!(remote(&clone), None);
        assert_eq!(remote(&clone), None);
        git(&clone, &["remote", "add", "upstream", "/nowhere"]);
        assert_eq!(remote(&clone).as_deref(), Some("upstream"));
        git(&clone, &["remote", "add", "origin", "/nowhere"]);
        assert_eq!(remote(&clone).as_deref(), Some("origin"));
    }

    #[test]
    fn minute_exchange_skips_repos_without_unfinished_tasks_or_active_waves() {
        let (_root, _origin, sender, idle) = machines();
        let task = sender.task("Remote task");
        sender.exchange();
        // Register idle's checkout with no unfinished Tasks and no non-retired Waves.
        crate::repository::location::ensure_repository(&idle.store, &idle.repo).unwrap();
        let backoffs = std::sync::Mutex::new(std::collections::HashMap::new());
        let mut attempted = Vec::new();
        super::exchange_unowned_at(&idle.store, std::time::Instant::now(), &backoffs, |repo| {
            attempted.push(repo.to_owned());
            Ok(())
        })
        .unwrap();
        assert!(
            attempted.is_empty(),
            "idle repo with no unfinished Task or active Wave must be skipped: {attempted:?}"
        );
        assert_eq!(idle.title(&task), None);

        // Adding an unfinished Task makes the repo eligible.
        let local_task = idle.task("Active work");
        super::exchange_unowned_at(&idle.store, std::time::Instant::now(), &backoffs, |repo| {
            attempted.push(repo.to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(attempted, std::slice::from_ref(&idle.repo));

        // Completing the Task and retiring its Wave skips the repo again.
        {
            let conn = idle.store.conn.lock().unwrap();
            conn.execute(
                "UPDATE tasks SET planning_state='completed' WHERE id=?1",
                [local_task.as_str()],
            )
            .unwrap();
            conn.execute("UPDATE waves SET retired_at=1", []).unwrap();
        }
        attempted.clear();
        super::exchange_unowned_at(&idle.store, std::time::Instant::now(), &backoffs, |repo| {
            attempted.push(repo.to_owned());
            Ok(())
        })
        .unwrap();
        assert!(
            attempted.is_empty(),
            "repo with only completed Tasks and retired Waves must be skipped: {attempted:?}"
        );
    }

    #[test]
    fn minute_exchange_doubles_backoff_on_failure_up_to_one_hour() {
        use std::time::{Duration, Instant};
        let (_root, _origin, _sender, repo) = machines();
        repo.task("Work");
        let backoffs = std::sync::Mutex::new(std::collections::HashMap::new());
        let t0 = Instant::now();
        let mut attempts = 0usize;

        // First failure sets 2-minute backoff.
        super::exchange_unowned_at(&repo.store, t0, &backoffs, |_| {
            attempts += 1;
            anyhow::bail!("offline")
        })
        .unwrap();
        assert_eq!(attempts, 1);

        // 119 s later: still backed off.
        super::exchange_unowned_at(
            &repo.store,
            t0 + Duration::from_secs(119),
            &backoffs,
            |_| {
                attempts += 1;
                anyhow::bail!("offline")
            },
        )
        .unwrap();
        assert_eq!(attempts, 1);

        // 120 s later: retries and fails -> 4-minute backoff.
        let t1 = t0 + Duration::from_secs(120);
        super::exchange_unowned_at(&repo.store, t1, &backoffs, |_| {
            attempts += 1;
            anyhow::bail!("offline")
        })
        .unwrap();
        assert_eq!(attempts, 2);

        // 239 s later: still backed off; 240 s later: retries.
        super::exchange_unowned_at(
            &repo.store,
            t1 + Duration::from_secs(239),
            &backoffs,
            |_| {
                attempts += 1;
                anyhow::bail!("offline")
            },
        )
        .unwrap();
        assert_eq!(attempts, 2);

        // Caps at 1 hour regardless of failure count.
        assert_eq!(super::RepoBackoff::delay(1), Duration::from_secs(120));
        assert_eq!(super::RepoBackoff::delay(2), Duration::from_secs(240));
        assert_eq!(super::RepoBackoff::delay(10), Duration::from_secs(3600));

        // Success clears the backoff.
        let t2 = t1 + Duration::from_secs(240);
        super::exchange_unowned_at(&repo.store, t2, &backoffs, |_| {
            attempts += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(attempts, 3);
        assert!(backoffs.lock().unwrap().is_empty());
    }

    #[test]
    fn remote_parses_git_config_without_spawning_and_falls_back_when_needed() {
        let root = tempfile::tempdir().unwrap();

        // Plain repo with no remote: 0 spawns.
        let plain = root.path().join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        git(&plain, &["init", "-q"]);
        let before = crate::observe::thread_spawns();
        assert_eq!(remote(&plain), None);
        assert_eq!(
            crate::observe::thread_spawns() - before,
            0,
            "cold lookup of plain repo without remotes must spawn nothing"
        );

        // Adding remotes in .git/config resolves without spawning.
        git(&plain, &["remote", "add", "upstream", "/nowhere"]);
        let before = crate::observe::thread_spawns();
        assert_eq!(remote(&plain).as_deref(), Some("upstream"));
        assert_eq!(crate::observe::thread_spawns() - before, 0);

        // Empty [remote "origin"] section without keys is ignored (matches git remote).
        let cfg_path = plain.join(".git/config");
        let mut cfg = std::fs::read_to_string(&cfg_path).unwrap();
        cfg.push_str("\n[remote \"origin\"] # comment only\n[core]\n\tbare = false\n");
        std::fs::write(&cfg_path, cfg).unwrap();
        let before = crate::observe::thread_spawns();
        assert_eq!(remote(&plain).as_deref(), Some("upstream"));
        assert_eq!(crate::observe::thread_spawns() - before, 0);

        // Adding a key under [remote "origin"] selects origin with 0 spawns.
        git(&plain, &["remote", "add", "origin", "/nowhere"]);
        let before = crate::observe::thread_spawns();
        assert_eq!(remote(&plain).as_deref(), Some("origin"));
        assert_eq!(crate::observe::thread_spawns() - before, 0);

        // [include] falls back to `git remote`.
        let inc_repo = root.path().join("with-include");
        std::fs::create_dir_all(&inc_repo).unwrap();
        git(&inc_repo, &["init", "-q"]);
        let extra = root.path().join("extra.gitconfig");
        std::fs::write(&extra, "[remote \"included\"]\n\turl = /nowhere\n").unwrap();
        let mut inc_cfg = std::fs::read_to_string(inc_repo.join(".git/config")).unwrap();
        inc_cfg.push_str(&format!("\n[include]\n\tpath = {}\n", extra.display()));
        std::fs::write(inc_repo.join(".git/config"), inc_cfg).unwrap();
        let before = crate::observe::thread_spawns();
        assert_eq!(remote(&inc_repo).as_deref(), Some("included"));
        assert_eq!(
            crate::observe::thread_spawns() - before,
            1,
            "[include] must fall back to git remote"
        );

        // Legacy .git/remotes/ falls back to `git remote` and matches its answer.
        let legacy_repo = root.path().join("with-legacy");
        std::fs::create_dir_all(&legacy_repo).unwrap();
        git(&legacy_repo, &["init", "-q"]);
        let remotes_dir = legacy_repo.join(".git/remotes");
        std::fs::create_dir_all(&remotes_dir).unwrap();
        std::fs::write(remotes_dir.join("legacy"), "URL: /nowhere\n").unwrap();
        let expected_legacy =
            super::remote_by_git(crate::git::git_stdout(&legacy_repo, &["remote"]).ok());
        let before = crate::observe::thread_spawns();
        assert_eq!(remote(&legacy_repo), expected_legacy);
        assert_eq!(
            crate::observe::thread_spawns() - before,
            1,
            "legacy .git/remotes must fall back to git remote"
        );
    }

    #[test]
    fn quiet_exchange_reuses_seen_refs_and_skips_repeated_cat_file() {
        let (_root, _origin, laptop, mini) = machines();
        let task = laptop.task("quiet poll");
        laptop.exchange();
        assert_eq!(laptop.unsent(), 0);

        // First quiet exchange: fetch + for-each-ref + one-time cat-file verification = 3 forks.
        let before = crate::observe::thread_spawns();
        laptop.exchange();
        assert_eq!(
            crate::observe::thread_spawns() - before,
            3,
            "first quiet exchange should only run fetch, for-each-ref, and one cat-file"
        );

        // Subsequent quiet exchanges reuse seen_refs's mine and the verified tip cache = 2 forks.
        let before = crate::observe::thread_spawns();
        laptop.exchange();
        assert_eq!(
            crate::observe::thread_spawns() - before,
            2,
            "subsequent quiet exchange must only run fetch + for-each-ref"
        );

        // Verify the plan still round-trips to another clone.
        mini.exchange();
        assert_eq!(mini.title(&task).as_deref(), Some("quiet poll"));
    }
}
