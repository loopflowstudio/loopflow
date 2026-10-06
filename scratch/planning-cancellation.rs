// Exploratory counterexample, 2026-10-05. Insert into store/mod.rs tests beside
// planning_store to reproduce with the real run_sqlite and a disposable database.
// This models the proposed async-scope lock; it is not a final production test.
// Required replacement coverage must call the cancellation-safe planning API.
    #[tokio::test]
    async fn cancelled_planning_writer_retains_acquisition_lock_until_ingestion_finishes() {
        let (directory, store, wave) = planning_store().await;
        let project = make_project(&wave);
        let task = make_task(&wave, &project);
        let snapshot = task_planning_snapshot(&wave, &project, &task);
        let lock_path = directory.path().join("planning.lock");
        let lock = std::fs::File::create(&lock_path).unwrap();
        fs2::FileExt::lock_exclusive(&lock).unwrap();
        let sqlite = store.sqlite.clone();
        let (entered, started) = tokio::sync::oneshot::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        let (finished, completion) = tokio::sync::oneshot::channel();
        let writer = tokio::spawn(async move {
            let _lock = lock;
            super::run_sqlite(&sqlite, move |store| {
                entered.send(()).unwrap();
                blocked.recv().unwrap();
                let result = store.put_pm_snapshot(&snapshot);
                finished.send(result.is_ok()).unwrap();
                result
            })
            .await
        });
        started.await.unwrap();
        writer.abort();
        assert!(writer.await.unwrap_err().is_cancelled());
        let next = std::fs::File::open(&lock_path).unwrap();
        let excluded = fs2::FileExt::try_lock_exclusive(&next).is_err();
        release.send(()).unwrap();
        assert!(completion.await.unwrap());
        assert!(store.pm_snapshot(wave.id()).await.unwrap().is_some());
        assert!(
            excluded,
            "cancelled caller released its lock before accepted planning was written"
        );
    }

