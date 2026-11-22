mod canonical_tests {
    use loopflow_execution::linear::LinearIssueId;
    use loopflow_model::durable::{MachineId, ProjectId, TaskId};
    use loopflow_model::id::{
        AgentProcessId, AgentSessionId, AttachmentToken, LfSessionId, TaskSelector, WaveId,
    };
    use loopflow_storage::pr_landing::PrLandingId;
    use loopflow_storage::work::task::TaskPrId;
    use std::collections::HashMap;

    #[test]
    fn canonical_ids_constructors_serde_hash_and_sql_agree() {
        let uuid = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA";
        for original in [
            uuid.to_owned(),
            uuid.replace('-', ""),
            format!("{{{uuid}}}"),
            format!("URN:UUID:{uuid}"),
        ] {
            let canonical = original.to_ascii_lowercase();
            let id = WaveId::parse(&original).unwrap();
            let decoded: WaveId = serde_json::from_value(original.clone().into()).unwrap();
            assert_eq!(id, decoded);
            assert_eq!(id.as_str(), canonical);
            assert_eq!(
                HashMap::from([(id.clone(), 1)]).get(&WaveId::parse(&canonical).unwrap()),
                Some(&1)
            );
            assert_eq!(serde_json::to_value(&id).unwrap(), canonical);
            let conn = rusqlite::Connection::open_in_memory().unwrap();
            let stored: WaveId = conn.query_row("SELECT ?1", [&id], |r| r.get(0)).unwrap();
            assert_eq!(stored, id);
            assert_eq!(
                AttachmentToken::parse(&original).unwrap().as_str(),
                canonical
            );
            assert_eq!(
                AgentProcessId::parse(&original).unwrap().as_str(),
                canonical
            );
            assert_eq!(LinearIssueId::new(&original).unwrap().as_str(), canonical);
        }
        macro_rules! prefixed {
            ($ty:ty,$prefix:literal) => {{
                let value = format!("{}{}", $prefix, uuid);
                let id = <$ty>::parse(&value).unwrap();
                assert_eq!(id.as_str(), value.to_ascii_lowercase());
                assert_eq!(serde_json::from_value::<$ty>(value.into()).unwrap(), id);
            }};
        }
        prefixed!(TaskId, "TASK_");
        prefixed!(ProjectId, "PROJ_");
        prefixed!(MachineId, "HOME_");
        prefixed!(TaskPrId, "PR_");
        prefixed!(PrLandingId, "LANDING_");
        assert_eq!(
            TaskId::from_raw("TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").as_str(),
            "task_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(
            LinearIssueId::from_raw("Case-Sensitive-History").as_str(),
            "Case-Sensitive-History"
        );
    }

    #[test]
    fn canonical_ids_session_is_distinct_from_native_conversation() {
        for value in [
            "SESSION_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ] {
            let id: LfSessionId = serde_json::from_value(value.into()).unwrap();
            assert_eq!(id.as_str(), value.to_ascii_lowercase());
            assert_ne!(
                AgentSessionId::from(value),
                AgentSessionId::from(value.to_ascii_lowercase())
            );
        }
        for value in [
            "Historical-Conversation",
            "run_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
        ] {
            assert_eq!(LfSessionId::from(value).as_str(), value);
        }
    }

    #[test]
    fn canonical_ids_task_selector_keeps_each_domain_separate() {
        let ticket = TaskSelector::parse("lOo-506");
        assert_eq!(ticket.label, "LOO-506");
        assert_eq!(ticket.external, "lOo-506");
        let opaque = TaskSelector::parse("issue-1");
        assert!(opaque.matches("issue-1"));
        assert_eq!(opaque.external, "issue-1");
        let task = TaskSelector::parse("TASK_ABCDEF");
        assert_eq!(task.local_prefix.as_deref(), Some("abcdef"));
        let linear = TaskSelector::parse("FFFFFFFF-FFFF-4FFF-8FFF-FFFFFFFFFFFF");
        assert!(linear.matches("ffffffff-ffff-4fff-8fff-ffffffffffff"));
        assert_eq!(TaskSelector::parse("lf-ABCD").label, "lf-abcd");
    }
}
