use crate::store::{
    AccountLimitWindow, CredentialState, ProviderAccount, ProviderAccountId, RoutingState,
};

fn account() -> ProviderAccount {
    ProviderAccount {
        provider: "claude".into(),
        account_id: ProviderAccountId::parse("primary").unwrap(),
        home: None,
        login_email: None,
        observed_email: None,
        observed_subject: None,
        observed_credential_digest: None,
        observed_plan: None,
        credential_state: CredentialState::Connected,
        routing_state: RoutingState::Disabled,
        plan: Some("configured".into()),
        paid_through: None,
        utilization_percent: Some(98),
        cooldown_until: Some(1900000000),
        cooldown_reason: Some("weekly".into()),
        last_selected_at: Some(7),
        created_at: 1,
        updated_at: 1,
    }
}

fn window(name: &str, used: u8, model: Option<&str>) -> AccountLimitWindow {
    AccountLimitWindow {
        window: name.into(),
        used_percent: used,
        model: model.map(str::to_owned),
        resets_at: Some(1_900_000_000),
        plan: None,
        limited_until: (used == 100).then_some(1_900_000_005),
    }
}

#[test]
fn claude_capacity_historical_names_reconcile_and_refresh() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        crate::store::opening::open_ephemeral(&directory.path().join("registry.db")).unwrap();
    let mut account = account();
    account.cooldown_until = None;
    crate::provider_account::catalog::upsert_provider_account(&store, &account).unwrap();
    for (name, model, canonical, scope) in [
        ("seven_day_overage_included", Some("fable"), "seven_day_fable", Some("fable")),
        ("rejected: seven_day_overage_included", Some("fable"), "seven_day_fable", Some("fable")),
        ("blocked: seven_day_fable", Some("fable"), "seven_day_fable", Some("fable")),
        ("limited: seven_day_overage_included", Some("fable"), "seven_day_fable", Some("fable")),
        ("rate_limit_reached: seven_day_fable", Some("fable"), "seven_day_fable", Some("fable")),
        ("You've hit your Fable limit · resets Nov 18 at 4am (America/Los_Angeles)", Some("fable"), "seven_day_fable", Some("fable")),
        ("You've reached your Fable limit. Run /usage-credits to continue or switch models with /model.", Some("fable"), "seven_day_fable", Some("fable")),
        ("weekly:fable", None, "seven_day_fable", Some("fable")),
        ("weekly", None, "seven_day", None),
        ("session", None, "five_hour", None),
    ] {
        store.upsert_provider_account_limits("claude", &account.account_id, &[window(name, 100, model)], "stream").unwrap();
        let rows = crate::provider_account::capacity::limits(&store, None).unwrap();
        assert_eq!(rows.len(), 1, "{name}");
        assert_eq!(rows[0].window, canonical);
        assert_eq!(rows[0].model.as_deref(), scope);
        assert!(rows[0].limited_until.is_some());
        let revision = store.provider_account_capacity_revision("claude", &account.account_id).unwrap();
        crate::provider_account::capacity::reconcile(&store, &account, revision, &[window(canonical, 16, scope)]).unwrap();
        let rows = crate::provider_account::capacity::limits(&store, None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].used_percent, 16);
        assert_eq!(rows[0].limited_until, None);
        let conn = store.conn.lock().unwrap();
        let raw: String = conn.query_row("SELECT window FROM provider_account_limits", [], |row| row.get(0)).unwrap();
        assert_eq!(raw, canonical);
        conn.execute("DELETE FROM provider_account_limits", []).unwrap();
    }
}

#[test]
fn claude_capacity_historical_unknown_credit_and_mismatched_rows_stay() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        crate::store::opening::open_ephemeral(&directory.path().join("registry.db")).unwrap();
    let account = account();
    crate::provider_account::catalog::upsert_provider_account(&store, &account).unwrap();
    for (name, model) in [
        ("rejected: overage", Some("fable")),
        ("You're out of usage credits. Run /usage-credits to keep using Fable 5.2 or /model to switch models.", Some("fable")),
        ("rejected: seven_day_overage_included", Some("future")),
        ("rejected: seven_day_fable", Some("sonnet")),
        ("unknown", None),
    ] {
        store.upsert_provider_account_limits("claude", &account.account_id, &[window(name, 100, model)], "stream").unwrap();
    }
    let before = crate::provider_account::capacity::limits(&store, None).unwrap();
    let revision = store
        .provider_account_capacity_revision("claude", &account.account_id)
        .unwrap();
    crate::provider_account::capacity::reconcile(
        &store,
        &account,
        revision,
        &[window("seven_day_fable", 16, Some("fable"))],
    )
    .unwrap();
    let after = crate::provider_account::capacity::limits(&store, None).unwrap();
    for row in before {
        assert!(after.contains(&row), "{row:?}");
    }
}

#[test]
fn claude_capacity_clear_releases_all_holds_and_supersedes_polls() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        crate::store::opening::open_ephemeral(&directory.path().join("registry.db")).unwrap();
    let account = account();
    crate::provider_account::catalog::upsert_provider_account(&store, &account).unwrap();
    store
        .upsert_provider_account_limits(
            "claude",
            &account.account_id,
            &[
                window("seven_day", 100, None),
                window("seven_day_fable", 100, Some("fable")),
                window("unknown", 100, Some("opus")),
            ],
            "stream",
        )
        .unwrap();
    let before = crate::provider_account::capacity::limits(&store, None).unwrap();
    let revision = store
        .provider_account_capacity_revision("claude", &account.account_id)
        .unwrap();
    store
        .clear_provider_account_cooldown("claude", &account.account_id)
        .unwrap();
    let current = crate::provider_account::catalog::get_provider_account(
        &store,
        "claude",
        &account.account_id,
    )
    .unwrap()
    .unwrap();
    assert_eq!(current.cooldown_until, None);
    assert_eq!(current.cooldown_reason, None);
    let after = crate::provider_account::capacity::limits(&store, None).unwrap();
    for (mut before, after) in before.into_iter().zip(after) {
        before.limited_until = None;
        assert_eq!(before, after);
    }
    assert!(
        store
            .provider_account_capacity_revision("claude", &account.account_id)
            .unwrap()
            > revision
    );
    assert!(!crate::provider_account::capacity::reconcile(
        &store,
        &account,
        revision,
        &[window("seven_day", 100, None)]
    )
    .unwrap());
}

#[test]
fn claude_usage_reconciliation_keeps_only_matching_aliases_and_scopes() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        crate::store::opening::open_ephemeral(&directory.path().join("registry.db")).unwrap();
    for (id, percent) in [("first", 11), ("second", 100)] {
        let mut account = account();
        account.account_id = ProviderAccountId::parse(id).unwrap();
        account.cooldown_until = None;
        account.cooldown_reason = None;
        crate::provider_account::catalog::upsert_provider_account(&store, &account).unwrap();
        let old = [
            window("seven_day_overage_included", 100, Some("fable")),
            window("usage credits", 100, Some("opus")),
            window("unknown", 100, None),
        ];
        store
            .upsert_provider_account_limits("claude", &account.account_id, &old, "stream")
            .unwrap();
        let revision = store
            .provider_account_capacity_revision("claude", &account.account_id)
            .unwrap();
        let polled = [
            window("seven_day", 66, None),
            window("seven_day_fable", percent, Some("fable")),
        ];
        assert!(
            crate::provider_account::capacity::reconcile(&store, &account, revision, &polled)
                .unwrap()
        );
        let limits = crate::provider_account::capacity::limits(&store, Some("claude")).unwrap();
        let own: Vec<_> = limits
            .iter()
            .filter(|row| row.account_id == account.account_id)
            .cloned()
            .collect();
        assert_eq!(own.len(), 4);
        assert!(!own
            .iter()
            .any(|row| row.window == "seven_day_overage_included"));
        assert!(own.iter().any(|row| row.window == "usage credits"));
        // Remove unrelated holds only from this local assertion's input.
        let included: Vec<_> = own
            .into_iter()
            .filter(|row| row.window.starts_with("seven_day"))
            .collect();
        for model in ["opus", "sonnet", "haiku"] {
            assert_eq!(
                crate::provider_account::account_held_until(
                    &account,
                    &included,
                    Some(model),
                    1_800_000_000
                ),
                None
            );
        }
        assert_eq!(
            crate::provider_account::account_held_until(
                &account,
                &included,
                Some("fable"),
                1_800_000_000
            ),
            (percent == 100).then_some(1_900_000_005)
        );
        assert_eq!(
            crate::provider_account::account_held_until(
                &account,
                &included,
                Some("fable"),
                1_900_000_005
            ),
            None
        );
    }
}

#[test]
fn claude_usage_reconciliation_revision_rejects_delayed_polls_and_rolls_back() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("registry.db");
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    let account = account();
    crate::provider_account::catalog::upsert_provider_account(&store, &account).unwrap();
    let revision = store
        .provider_account_capacity_revision("claude", &account.account_id)
        .unwrap();
    // Two independent writers can observe and write in the same second.
    let writer = crate::store::opening::open_ephemeral(&path).unwrap();
    writer
        .record_provider_account_capacity(
            "claude",
            &account.account_id,
            &crate::store::AccountCapacity {
                utilization_percent: Some(100),
                cooldown_until: Some(1_900_000_005),
                cooldown_reason: Some("rejected: seven_day".into()),
                windows: vec![window("seven_day", 100, None)],
            },
        )
        .unwrap();
    assert!(!crate::provider_account::capacity::reconcile(
        &store,
        &account,
        revision,
        &[window("seven_day", 10, None)]
    )
    .unwrap());
    assert_eq!(
        crate::provider_account::capacity::limits(&store, None).unwrap()[0].used_percent,
        100
    );
    let revision = store
        .provider_account_capacity_revision("claude", &account.account_id)
        .unwrap();
    assert!(crate::provider_account::capacity::reconcile(
        &store,
        &account,
        revision,
        &[window("seven_day", 20, None)]
    )
    .unwrap());
    assert!(!crate::provider_account::capacity::reconcile(
        &writer,
        &account,
        revision,
        &[window("seven_day", 90, None)]
    )
    .unwrap());
    assert_eq!(
        crate::provider_account::capacity::limits(&store, None).unwrap()[0].used_percent,
        20
    );
    assert_eq!(
        crate::provider_account::catalog::get_provider_account(
            &store,
            "claude",
            &account.account_id
        )
        .unwrap()
        .unwrap()
        .cooldown_until,
        None
    );

    let revision = store
        .provider_account_capacity_revision("claude", &account.account_id)
        .unwrap();
    store.conn.lock().unwrap().execute_batch("CREATE TRIGGER fail_window BEFORE INSERT ON provider_account_limits WHEN NEW.window='fail' BEGIN SELECT RAISE(ABORT, 'fixture failure'); END;").unwrap();
    assert!(crate::provider_account::capacity::reconcile(
        &store,
        &account,
        revision,
        &[window("seven_day", 5, None), window("fail", 5, None)]
    )
    .is_err());
    assert_eq!(
        store
            .provider_account_capacity_revision("claude", &account.account_id)
            .unwrap(),
        revision
    );
    assert_eq!(
        crate::provider_account::capacity::limits(&store, None).unwrap()[0].used_percent,
        20
    );
}

#[test]
fn claude_usage_reconciliation_requires_the_cause_and_preserves_global_rejection() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        crate::store::opening::open_ephemeral(&directory.path().join("registry.db")).unwrap();
    for (cause, polled, cleared) in [
        (
            "rejected: seven_day",
            vec![window("seven_day_fable", 11, Some("fable"))],
            false,
        ),
        (
            "rejected: seven_day",
            vec![window("seven_day", 66, None)],
            true,
        ),
        (
            "rejected: five_hour",
            vec![window("seven_day", 66, None)],
            false,
        ),
        (
            "rejected: five_hour",
            vec![window("five_hour", 10, None)],
            true,
        ),
        (
            "rejected: seven_day",
            vec![window("seven_day", 100, None)],
            false,
        ),
        (
            "rejected: seven_day_fable",
            vec![window("seven_day_fable", 100, Some("fable"))],
            true,
        ),
        (
            "rejected: seven_day_overage_included",
            vec![window("seven_day", 66, None)],
            false,
        ),
        ("usage credits", vec![window("seven_day", 66, None)], false),
        ("rejected", vec![window("seven_day", 66, None)], false),
        (
            "unknown: seven_day",
            vec![window("seven_day", 66, None)],
            false,
        ),
        ("rejected: seven_day", vec![], false),
    ] {
        let mut account = account();
        account.cooldown_reason = Some(cause.into());
        crate::provider_account::catalog::upsert_provider_account(&store, &account).unwrap();
        let revision = store
            .provider_account_capacity_revision("claude", &account.account_id)
            .unwrap();
        assert!(
            crate::provider_account::capacity::reconcile(&store, &account, revision, &polled)
                .unwrap()
        );
        let current = crate::provider_account::catalog::get_provider_account(
            &store,
            "claude",
            &account.account_id,
        )
        .unwrap()
        .unwrap();
        assert_eq!(current.cooldown_until.is_none(), cleared, "{cause}");
    }
    store
        .record_provider_account_health(
            "claude",
            &account().account_id,
            Some(100),
            Some(1_900_000_000),
            Some("rejected: seven_day"),
        )
        .unwrap();
    store
        .record_provider_account_capacity(
            "claude",
            &account().account_id,
            &crate::store::AccountCapacity {
                utilization_percent: Some(50),
                cooldown_until: None,
                cooldown_reason: None,
                windows: vec![window("five_hour", 50, None)],
            },
        )
        .unwrap();
    let current = crate::provider_account::catalog::get_provider_account(
        &store,
        "claude",
        &account().account_id,
    )
    .unwrap()
    .unwrap();
    assert_eq!(current.cooldown_until, Some(1_900_000_000));
    assert_eq!(
        current.cooldown_reason.as_deref(),
        Some("rejected: seven_day")
    );
}

#[test]
fn claude_usage_reconciliation_missing_reset_keeps_rejection_deadline() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        crate::store::opening::open_ephemeral(&directory.path().join("registry.db")).unwrap();
    let mut account = account();
    account.cooldown_until = None;
    crate::provider_account::catalog::upsert_provider_account(&store, &account).unwrap();
    store
        .upsert_provider_account_limits(
            "claude",
            &account.account_id,
            &[window("seven_day_overage_included", 100, Some("fable"))],
            "stream",
        )
        .unwrap();
    let mut spent = window("seven_day_fable", 100, Some("fable"));
    spent.resets_at = None;
    spent.limited_until = None;
    let revision = store
        .provider_account_capacity_revision("claude", &account.account_id)
        .unwrap();
    assert!(
        crate::provider_account::capacity::reconcile(&store, &account, revision, &[spent]).unwrap()
    );
    let current = crate::provider_account::capacity::limits(&store, None).unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].resets_at, None);
    assert_eq!(current[0].limited_until, Some(1_900_000_005));
    let revision = store
        .provider_account_capacity_revision("claude", &account.account_id)
        .unwrap();
    store
        .clear_provider_account_cooldown("claude", &account.account_id)
        .unwrap();
    assert!(
        !crate::provider_account::capacity::reconcile(&store, &account, revision, &[]).unwrap()
    );
    let revision = store
        .provider_account_capacity_revision("claude", &account.account_id)
        .unwrap();
    store
        .record_provider_account_credential_invalidated("claude", &account.account_id, "fixture")
        .unwrap();
    assert!(
        !crate::provider_account::capacity::reconcile(&store, &account, revision, &[]).unwrap()
    );
}

#[test]
fn claude_usage_reconciliation_legacy_precedence_and_indexed_lookups() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        crate::store::opening::open_ephemeral(&directory.path().join("registry.db")).unwrap();
    let account = account();
    crate::provider_account::catalog::upsert_provider_account(&store, &account).unwrap();
    store
        .upsert_provider_account_limits(
            "claude",
            &account.account_id,
            &[
                window("rejected: seven_day_overage_included", 100, Some("fable")),
                window("seven_day_fable", 11, Some("fable")),
            ],
            "stream",
        )
        .unwrap();
    // Equal timestamps keep the rejection; a strictly newer canonical row wins.
    assert_eq!(
        crate::provider_account::capacity::limits(&store, None).unwrap()[0].used_percent,
        100
    );
    let conn = store.conn.lock().unwrap();
    conn.execute("UPDATE provider_account_limits SET observed_at=observed_at+1 WHERE window='seven_day_fable'", []).unwrap();
    drop(conn);
    assert_eq!(
        crate::provider_account::capacity::limits(&store, None).unwrap()[0].used_percent,
        11
    );
    let conn = store.conn.lock().unwrap();
    conn.execute("UPDATE provider_account_limits SET observed_at=observed_at+2 WHERE window='rejected: seven_day_overage_included'", []).unwrap();
    drop(conn);
    let rows = crate::provider_account::capacity::limits(&store, None).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].window, "seven_day_fable");
    assert_eq!(rows[0].used_percent, 100);
    assert_eq!(rows[0].limited_until, Some(1_900_000_005));
    let conn = store.conn.lock().unwrap();
    conn.execute_batch(
        "WITH RECURSIVE accounts(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM accounts WHERE n<10000)
        INSERT INTO provider_accounts(provider,account_id,credential_state,created_at,updated_at)
        SELECT 'claude', 'fixture-'||n, 'connected', 1, 1 FROM accounts;",
    )
    .unwrap();
    drop(conn);
    let revision = store
        .provider_account_capacity_revision("claude", &account.account_id)
        .unwrap();
    let statements = loopflow_storage::query_plans::statements(&store, || {
        assert!(crate::provider_account::capacity::reconcile(
            &store,
            &account,
            revision,
            &[window("seven_day_fable", 10, Some("fable"))]
        )
        .unwrap());
    });
    let conn = store.conn.lock().unwrap();
    for sql in statements.iter().filter(|sql| {
        ["SELECT", "UPDATE", "DELETE"]
            .iter()
            .any(|keyword| sql.starts_with(keyword))
            && (sql.contains("provider_accounts") || sql.contains("provider_account_limits"))
    }) {
        let mut explain = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
        let parameters = vec![rusqlite::types::Value::Null; explain.parameter_count()];
        let plan: Vec<String> = explain
            .query_map(rusqlite::params_from_iter(parameters), |row| row.get(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            plan.iter()
                .any(|line| line.contains("SEARCH") && line.contains("INDEX")),
            "{plan:?}"
        );
        assert!(!plan.iter().any(|line| line.contains("SCAN")), "{plan:?}");
    }
}

#[test]
fn a_capacity_replacement_rechecks_the_snapshot_after_a_stream_write() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        crate::store::opening::open_ephemeral(&directory.path().join("registry.db")).unwrap();
    let account = account();
    crate::provider_account::catalog::upsert_provider_account(&store, &account).unwrap();
    store
        .upsert_provider_account_limits(
            "claude",
            &account.account_id,
            &[window("weekly", 100, None)],
            "stream",
        )
        .unwrap();
    let reading = store
        .provider_account_capacity("claude", &account.account_id)
        .unwrap();
    let update = loopflow_storage::capacity::ProviderCapacityUpdate {
        windows: vec![window("seven_day", 10, None)],
        superseded_windows: vec!["weekly".into()],
        recovered: true,
    };
    store
        .record_provider_account_capacity(
            "claude",
            &account.account_id,
            &crate::store::AccountCapacity {
                utilization_percent: Some(100),
                cooldown_until: Some(1_900_000_100),
                cooldown_reason: Some("new rejection".into()),
                windows: vec![window("five_hour", 100, None)],
            },
        )
        .unwrap();
    let before = store.provider_account_limit_rows(None).unwrap();
    assert!(!store
        .apply_provider_account_capacity("claude", &account.account_id, reading.revision, &update)
        .unwrap());
    assert_eq!(store.provider_account_limit_rows(None).unwrap(), before);
    assert_eq!(
        crate::provider_account::catalog::get_provider_account(
            &store,
            "claude",
            &account.account_id
        )
        .unwrap()
        .unwrap()
        .cooldown_reason
        .as_deref(),
        Some("new rejection")
    );
}

#[test]
fn account_file_edits_apply_to_single_and_filtered_reads() {
    let _env = loopflow_test_support::env_lock().lock().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let store =
        crate::store::opening::open_ephemeral(&directory.path().join("registry.db")).unwrap();
    let mut expected = account();
    expected.login_email = Some(crate::profile::EmailAddress::parse("work@example.com").unwrap());
    expected.paid_through =
        Some(time::Date::from_calendar_date(2026, time::Month::December, 1).unwrap());
    expected.observed_email = Some("observed@example.com".into());
    expected.observed_subject = Some("subject".into());
    expected.observed_plan = Some("observed plan".into());
    expected.observed_credential_digest = Some("digest".into());
    crate::provider_account::catalog::upsert_provider_account(&store, &expected).unwrap();

    let assert_reads = |expected: &ProviderAccount| {
        assert_eq!(
            crate::provider_account::catalog::get_provider_account(
                &store,
                "claude",
                &expected.account_id
            )
            .unwrap()
            .as_ref(),
            Some(expected)
        );
        for provider in [None, Some("claude")] {
            assert_eq!(
                crate::provider_account::catalog::list_provider_accounts(&store, provider).unwrap(),
                std::slice::from_ref(expected)
            );
        }
        assert!(
            crate::provider_account::catalog::list_provider_accounts(&store, Some("codex"))
                .unwrap()
                .is_empty()
        );
    };
    assert_reads(&expected);
    let path = directory.path().join("config.yaml");
    let mut declarations = crate::config::files::read(&path).unwrap();
    declarations["accounts"]["claude"]["primary"]["plan"] = "edited".into();
    std::fs::write(&path, serde_yaml_ng::to_string(&declarations).unwrap()).unwrap();
    expected.plan = Some("edited".into());
    assert_reads(&expected);

    std::fs::write(path, "{}\n").unwrap();
    expected.login_email = None;
    expected.routing_state = RoutingState::Automatic;
    expected.plan = None;
    expected.paid_through = None;
    assert_reads(&expected);
}

#[test]
fn account_observations_preserve_policy_and_omitted_windows() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        crate::store::opening::open_ephemeral(&directory.path().join("registry.db")).unwrap();
    let expected = account();
    crate::provider_account::catalog::upsert_provider_account(&store, &expected).unwrap();
    let weekly = AccountLimitWindow {
        window: "weekly".into(),
        used_percent: 98,
        resets_at: Some(1900000000),
        plan: Some("max".into()),
        model: None,
        limited_until: None,
    };
    store
        .upsert_provider_account_limits("claude", &expected.account_id, &[weekly], "stream")
        .unwrap();
    let previous = crate::provider_account::capacity::limits(&store, None)
        .unwrap()
        .remove(0);
    for state in [CredentialState::Missing, CredentialState::Connected] {
        store
            .update_provider_account_credential_state("claude", &expected.account_id, state)
            .unwrap();
        let mut actual = crate::provider_account::catalog::get_provider_account(
            &store,
            "claude",
            &expected.account_id,
        )
        .unwrap()
        .unwrap();
        assert_eq!(actual.credential_state, state);
        actual.credential_state = expected.credential_state;
        actual.updated_at = expected.updated_at;
        assert_eq!(actual, expected);
    }
    let session = AccountLimitWindow {
        window: "session".into(),
        used_percent: 2,
        resets_at: None,
        plan: None,
        model: None,
        limited_until: None,
    };
    store
        .upsert_provider_account_limits("claude", &expected.account_id, &[session], "poll")
        .unwrap();
    let rows = crate::provider_account::capacity::limits(&store, None).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter().find(|row| row.window == "seven_day"),
        Some(&previous)
    );
}

#[test]
fn account_observation_batch_rolls_back_on_later_window_failure() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        crate::store::opening::open_ephemeral(&directory.path().join("registry.db")).unwrap();
    let account = account();
    crate::provider_account::catalog::upsert_provider_account(&store, &account).unwrap();
    let session = AccountLimitWindow {
        window: "session".into(),
        used_percent: 22,
        resets_at: None,
        plan: None,
        model: None,
        limited_until: None,
    };
    store
        .upsert_provider_account_limits(
            "claude",
            &account.account_id,
            std::slice::from_ref(&session),
            "stream",
        )
        .unwrap();
    let previous = crate::provider_account::capacity::limits(&store, None).unwrap();
    store.conn.lock().unwrap().execute_batch("CREATE TRIGGER reject_weekly BEFORE INSERT ON provider_account_limits WHEN NEW.window = 'weekly' BEGIN SELECT RAISE(ABORT, 'simulated disk failure'); END;").unwrap();
    let windows = [
        AccountLimitWindow {
            used_percent: 40,
            ..session.clone()
        },
        AccountLimitWindow {
            window: "weekly".into(),
            ..session
        },
    ];
    assert!(store
        .upsert_provider_account_limits("claude", &account.account_id, &windows, "poll")
        .is_err());
    assert_eq!(
        crate::provider_account::capacity::limits(&store, None).unwrap(),
        previous
    );
}
