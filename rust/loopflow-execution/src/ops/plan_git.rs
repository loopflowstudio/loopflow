//! Git carries a plan without Linear or Asana. Each Machine pushes its whole
//! plan to a ref of its own on the repository's remote
//! (`refs/loopflow/plan/<machine id>`, one stamped record per file) and reads
//! the others' from `refs/loopflow/plan-seen/*`. It decides nothing: arriving
//! records go through the one import, where the newest stamp wins.
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::Receiver;
use std::time::Instant;

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

/// The remote a repository's plan travels by: `origin`, else the first by
/// name. `None` for a repository with no remote.
pub fn remote(repo: &Path) -> Option<String> {
    let remotes = crate::git::git_stdout(repo, &["remote"]).ok()?;
    let mut remotes: Vec<&str> = remotes.lines().filter(|name| !name.is_empty()).collect();
    remotes.sort_unstable();
    remotes
        .iter()
        .find(|name| **name == "origin")
        .or(remotes.first())
        .map(|name| (*name).to_owned())
}

/// One Git exchange for every repository this Machine plans without Linear
/// or Asana. The Machine server runs this on its timer.
pub fn exchange_all(store: &SqliteStore) -> Result<()> {
    for path in store.repository_paths()? {
        let repo = path.to_string_lossy();
        if !path.is_dir() || crate::ops::tracker::connected(&repo) {
            continue;
        }
        if let Err(error) = exchange(store, &repo, None) {
            tracing::debug!(%repo, error = %format!("{error:#}"), "Git plan exchange failed");
        }
    }
    Ok(())
}

/// [`exchange`] for a clone this Machine has saved nothing in and never read
/// a plan for, which registers it for [`exchange_all`]. `false` when the
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
        publish(
            store,
            path,
            &remote,
            me.as_str(),
            &repository,
            sent_seq,
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
    let seen = seen_refs(path)?;
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
                .filter(|base| commit_exists(path, base))
                .or(mine);
            let (records, theirs) = changed_records(path, base.map(String::as_str), tip)?;
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
        until,
    )?;
    failed.map_or(Ok(()), Err)?;
    NotTaken::of(&not_taken).map_or(Ok(()), |not_taken| Err(not_taken.into()))
}

/// Each fetched plan ref's Machine and commit.
fn seen_refs(repo: &Path) -> Result<HashMap<String, String>> {
    let listed = git(
        repo,
        &["for-each-ref", "--format=%(refname) %(objectname)", SEEN],
        None,
    )?;
    Ok(String::from_utf8_lossy(&listed)
        .lines()
        .filter_map(|line| line.strip_prefix(SEEN)?.split_once(' '))
        .map(|(machine, tip)| (machine.to_owned(), tip.to_owned()))
        .collect())
}

fn commit_exists(repo: &Path, commit: &str) -> bool {
    git(
        repo,
        &["cat-file", "-e", &format!("{commit}^{{commit}}")],
        None,
    )
    .is_ok()
}

/// The records whose files differ between `base` (nothing, when `None`) and
/// `tip`, and the repository id `tip` names when that changed too.
fn changed_records(
    repo: &Path,
    base: Option<&str>,
    tip: &str,
) -> Result<(Vec<StampedRecord>, Option<RepositoryId>)> {
    let empty;
    let base = match base {
        Some(base) => base,
        None => {
            let hashed = git(repo, &["hash-object", "-t", "tree", "--stdin"], Some(b""))?;
            empty = String::from_utf8_lossy(&hashed).trim().to_owned();
            &empty
        }
    };
    let diff = git(
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
    let blobs = git(repo, &["cat-file", "--batch"], Some(wanted.as_bytes()))?;
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

/// Commit every record changed here since `sent_seq` onto this Machine's ref
/// as last fetched from the remote, and push, giving up at `until`. A remote
/// without this Machine's ref is given the whole plan. Nothing is forced: a
/// push the remote refuses is the error.
fn publish(
    store: &SqliteStore,
    repo: &Path,
    remote: &str,
    me: &str,
    repository: &RepositoryId,
    sent_seq: i64,
    until: Option<Instant>,
) -> Result<()> {
    let key = repo.to_string_lossy();
    let mine = git(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{SEEN}{me}^{{commit}}"),
        ],
        None,
    )
    .ok()
    .map(|tip| String::from_utf8_lossy(&tip).trim().to_owned());
    let mine = mine.as_ref();
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
    let named = mine.is_some_and(|mine| {
        git(
            repo,
            &["cat-file", "-p", &format!("{mine}:{REPOSITORY}")],
            None,
        )
        .is_ok_and(|held| String::from_utf8_lossy(&held).trim() == repository.as_str())
    });
    if files.is_empty() && (named || mine.is_none()) {
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
    git(repo, &["fast-import", "--quiet", "--force"], Some(&stream))?;
    let tree = |commit: &str| git(repo, &["rev-parse", &format!("{commit}^{{tree}}")], None);
    if mine.is_none_or(|mine| tree(mine).ok() != tree(&own).ok()) {
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
        git(repo, &["update-ref", &format!("{SEEN}{me}"), &own], None)?;
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

/// Run Git in `repo` with no terminal to ask on, giving it `input`. A failure
/// is Git's own reason.
fn git(repo: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>> {
    git_until(repo, args, input, None)
}

/// [`git`], stopped at `until` when it has not answered by then.
fn git_until(
    repo: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    until: Option<Instant>,
) -> Result<Vec<u8>> {
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
    let mut child = command.spawn().context("start git")?;
    let stdin = child.stdin.take();
    let stdout = read_in_background(child.stdout.take().expect("stdout is piped"));
    let stderr = read_in_background(child.stderr.take().expect("stderr is piped"));
    let answered = |pipe: &Receiver<Vec<u8>>| match until {
        Some(until) => pipe
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .ok(),
        None => pipe.recv().ok(),
    };
    let output = std::thread::scope(|scope| {
        if let (Some(mut stdin), Some(input)) = (stdin, input) {
            // Git may answer before it has read everything it is given.
            scope.spawn(move || {
                let _ = stdin.write_all(input);
            });
        }
        let output = (answered(&stdout), answered(&stderr));
        if output.0.is_none() || output.1.is_none() {
            // SAFETY: the child is unreaped, so its id still names the
            // group it leads; signalling a group has no memory effects.
            unsafe { libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL) };
        }
        output
    });
    let status = child.wait()?;
    let (Some(stdout), Some(stderr)) = output else {
        bail!("git {} did not answer in time", args[0]);
    };
    if status.success() {
        return Ok(stdout);
    }
    let stderr = String::from_utf8_lossy(&stderr);
    let reason = stderr
        .lines()
        .find(|line| line.starts_with("fatal:") || line.starts_with("error:"))
        .or_else(|| stderr.lines().rfind(|line| !line.trim().is_empty()));
    match reason {
        Some(reason) => bail!("{}", reason.trim()),
        None => bail!("git {} failed with {}", args[0], status),
    }
}

fn read_in_background(mut pipe: impl Read + Send + 'static) -> Receiver<Vec<u8>> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        let _ = sender.send(bytes);
    });
    receiver
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
        // A push that has not answered by the deadline is stopped.
        let error = send(&laptop.store, &laptop.repo, std::time::Instant::now()).unwrap_err();
        assert_eq!(format!("{error:#}"), "git push did not answer in time");
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
}
