// The 50-Session boundary measurement (scratch/loo-821-hooks.md). Paste into
// server/boundary.rs tests with `SqliteStore::open_shared_for_bench`
// (`#[cfg(test)]` over `open_with(path, home, Forbidden, SQLITE_WRITE_BUSY_TIMEOUT)`),
// then: LF821_STORE=<copy of loopflow.db> cargo test -p loopflow --lib zz_fifty -- --ignored --nocapture


    /// 50 Sessions on Tasks of a real store take boundaries at once, each
    /// request on its own connection as the server opens them. Set
    /// `LF821_STORE` to a copy of the store; it is written.
    #[test]
    #[ignore]
    fn zz_fifty_concurrent_sessions() {
        use std::time::Instant;
        let copy = std::path::PathBuf::from(std::env::var("LF821_STORE").unwrap());
        drop(SqliteStore::new(&copy).unwrap());
        let shared_home = tempfile::tempdir().unwrap();
        let path = shared_home.path().join(".lf/loopflow.db");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::rename(&copy, &path).unwrap();
        let open = |path: &std::path::Path| SqliteStore::open_shared_for_bench(path, shared_home.path()).unwrap();
        let home = tempfile::tempdir().unwrap();
        let setup = open(&path);
        let tasks: Vec<(String, String, Option<String>)> = setup.conn.lock().unwrap()
            .prepare("SELECT t.id,t.worktree,p.wave_id FROM tasks t LEFT JOIN projects p ON p.id=t.project_id WHERE t.worktree!='' ORDER BY t.updated_at DESC LIMIT 50").unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().collect::<Result<_, _>>().unwrap();
        let sessions: Vec<(SteerAdapter, std::path::PathBuf)> = tasks.iter().enumerate().map(|(n, (task, worktree, wave))| {
            let id = uuid::Uuid::new_v4().simple().to_string();
            setup.test_session(&id, &crate::session_record::new_artifact_key());
            setup.conn.lock().unwrap().execute(
                "UPDATE agent_sessions SET cwd=?2,task_id=?3,wave_id=?4,created_at=unixepoch() WHERE id=?1",
                rusqlite::params![id, worktree, task, wave]).unwrap();
            let process = setup.test_lf_process();
            let claim = setup.claim_session_attachment(&id, None, &process, false).unwrap();
            setup.record_agent_process_identity(&id, &claim, 100_000 + n as u32, 1).unwrap();
            let adapter = setup.reserve_steer_adapter(&id, &claim).unwrap();
            setup.conn.lock().unwrap().execute(
                "UPDATE processes SET agent_interactive=1 WHERE id=?1", [&adapter.agent_process_id]).unwrap();
            setup.with_provider_context(&adapter, "native", NativeBoundary::Activate { source: "startup" }, || Ok(())).unwrap();
            let delivery = home.path().join(format!("{id}.json"));
            ContextDelivery { repository: None, repo: worktree.into(), wave: None, skill_file: None,
                submitted_docs: None, references: Vec::new(), home: home.path().to_owned() }
                .save(&delivery).unwrap();
            (adapter, delivery)
        }).collect();
        let path = path.as_path();
        let run = |label: &str, width: usize, rounds: usize, make: &(dyn Fn(&SteerAdapter, &std::path::Path, usize, &[crate::id::SteerBatchId]) -> SessionBoundary + Sync), steer: bool| {
            if steer {
                for (adapter, _) in &sessions[..width] {
                    setup.save_session_steer(&adapter.session, "Bench", "STEER").unwrap();
                }
            }
            let started = Instant::now();
            let mut all: Vec<(f64, f64)> = std::thread::scope(|scope| {
                sessions[..width].iter().map(|(adapter, delivery)| scope.spawn(move || {
                    let mut printed = Vec::new();
                    (0..rounds).map(|round| {
                        let t = Instant::now();
                        let store = open(path);
                        let opened = t.elapsed().as_secs_f64() * 1000.0;
                        let (answered, _) = answer(&store, &make(adapter, delivery, round, &printed)).unwrap();
                        printed = answered.steers.into_iter().map(|s| s.batch).collect();
                        (t.elapsed().as_secs_f64() * 1000.0, opened)
                    }).collect::<Vec<_>>()
                })).collect::<Vec<_>>().into_iter().flat_map(|h| h.join().unwrap()).collect()
            });
            let wall = started.elapsed().as_secs_f64();
            all.sort_by(|a, b| a.0.total_cmp(&b.0));
            let at = |q: f64| all[((all.len() as f64 * q) as usize).min(all.len() - 1)].0;
            let open = all.iter().map(|x| x.1).sum::<f64>() / all.len() as f64;
            eprintln!("{label}: {} boundaries in {wall:.2}s; ms p50 {:.1} p95 {:.1} p99 {:.1} max {:.1}; open mean {open:.1}",
                all.len(), at(0.5), at(0.95), at(0.99), all[all.len() - 1].0);
        };
        let tool = |adapter: &SteerAdapter, delivery: &std::path::Path, round: usize, printed: &[crate::id::SteerBatchId]| {
            let mut b = boundary(BoundaryMoment::Tool, adapter, delivery,
                serde_json::json!({"session_id":"native","hook_event_name":"PostToolUse","tool_use_id":format!("t{round}"),"turn_id":"x"}));
            b.printed = printed.to_vec();
            b
        };
        let prompt = |adapter: &SteerAdapter, delivery: &std::path::Path, round: usize, printed: &[crate::id::SteerBatchId]| {
            let mut b = boundary(BoundaryMoment::Tool, adapter, delivery,
                serde_json::json!({"session_id":"native","hook_event_name":"UserPromptSubmit","prompt_id":format!("p{round}"),"prompt":"go"}));
            b.printed = printed.to_vec();
            b
        };
        let stop = |adapter: &SteerAdapter, delivery: &std::path::Path, round: usize, printed: &[crate::id::SteerBatchId]| {
            let mut b = boundary(BoundaryMoment::Stop, adapter, delivery,
                serde_json::json!({"session_id":"native","hook_event_name":"Stop","prompt_id":format!("p{round}")}));
            b.printed = printed.to_vec();
            b
        };
        for width in [1, 50] {
            eprintln!("{width} Sessions at once");
            run("  tool, nothing pending", width, 20, &tool, false);
            run("  prompt with a steer, then confirm", width, 2, &prompt, true);
            run("  turn end", width, 1, &stop, false);
        }
    }

