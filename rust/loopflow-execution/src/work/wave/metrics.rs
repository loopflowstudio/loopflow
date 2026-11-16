pub use loopflow_storage::metric::*;
use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Load the reviewed Markdown contracts in one Wave's `metrics/` directory.
///
/// The caller supplies the current PM Project ids. Instrument binding belongs
/// to observation storage, where it can be checked against this exact metric
/// identity rather than a misleading global set of names.
pub fn discover_metric_contracts(
    metrics_dir: &Path,
    wave_id: &str,
) -> Result<MetricContractDiscovery, MetricError> {
    let metadata = match fs::symlink_metadata(metrics_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(MetricContractDiscovery {
                contracts: Vec::new(),
                contract_issues: Vec::new(),
            });
        }
        Err(error) => {
            return Err(MetricError::ContractDirectoryRead {
                path: metrics_dir.display().to_string(),
                message: error.to_string(),
            });
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(MetricError::SymlinkedContractDirectory(
            metrics_dir.display().to_string(),
        ));
    }
    if !metadata.is_dir() {
        return Err(MetricError::ContractDirectoryNotDirectory(
            metrics_dir.display().to_string(),
        ));
    }
    if let Some(repository_root) = contract_repository_root(metrics_dir) {
        let canonical_root =
            repository_root
                .canonicalize()
                .map_err(|error| MetricError::ContractDirectoryRead {
                    path: repository_root.display().to_string(),
                    message: error.to_string(),
                })?;
        let canonical_metrics =
            metrics_dir
                .canonicalize()
                .map_err(|error| MetricError::ContractDirectoryRead {
                    path: metrics_dir.display().to_string(),
                    message: error.to_string(),
                })?;
        if !canonical_metrics.starts_with(&canonical_root) {
            return Err(MetricError::ContractDirectoryOutsideRepository {
                path: metrics_dir.display().to_string(),
                repository: repository_root.display().to_string(),
            });
        }
    }

    let entries =
        fs::read_dir(metrics_dir).map_err(|error| MetricError::ContractDirectoryRead {
            path: metrics_dir.display().to_string(),
            message: error.to_string(),
        })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| MetricError::ContractDirectoryRead {
            path: metrics_dir.display().to_string(),
            message: error.to_string(),
        })?;
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "md") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut sources = Vec::new();
    let mut contract_issues = Vec::new();
    for path in paths {
        match load_metric_contract(&path, wave_id) {
            Ok(contract) => sources.push(LoadedMetricContract { path, contract }),
            Err(error) => contract_issues.push(MetricContractIssueDto::MalformedContract {
                path: contract_issue_path(metrics_dir, &path),
                message: error.to_string(),
            }),
        }
    }

    let contracts = sources;

    contract_issues.sort_by_key(contract_issue_sort_key);
    Ok(MetricContractDiscovery {
        contracts,
        contract_issues,
    })
}

fn contract_issue_path(metrics_dir: &Path, path: &Path) -> String {
    let repository_root = contract_repository_root(metrics_dir);
    repository_root
        .and_then(|root| path.strip_prefix(root).ok())
        .or_else(|| {
            metrics_dir
                .parent()
                .and_then(|parent| path.strip_prefix(parent).ok())
        })
        .unwrap_or(path)
        .display()
        .to_string()
}

fn contract_repository_root(metrics_dir: &Path) -> Option<&Path> {
    metrics_dir
        .ancestors()
        .find(|ancestor| ancestor.ends_with(".lf/wave"))
        .and_then(Path::parent)
        .and_then(Path::parent)
}

/// Parse one reviewed metric contract. Contract discovery turns failures into
/// a per-file issue so one malformed Markdown file cannot hide its siblings.
pub fn load_metric_contract(path: &Path, wave_id: &str) -> Result<MetricContract, MetricError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| MetricError::ContractRead {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    if metadata.file_type().is_symlink() {
        return Err(MetricError::SymlinkedContract(path.display().to_string()));
    }
    if !metadata.is_file() {
        return Err(MetricError::ContractNotFile(path.display().to_string()));
    }
    let content = fs::read_to_string(path).map_err(|error| MetricError::ContractRead {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    let (frontmatter, markdown) = loopflow_definitions::text::split_frontmatter(&content)
        .ok_or(MetricError::MissingContractFrontmatter)?;
    let frontmatter: MetricContractFrontmatter = serde_yaml_ng::from_str(frontmatter)
        .map_err(|error| MetricError::InvalidContractFrontmatter(error.to_string()))?;
    if frontmatter.schema != 1 {
        return Err(MetricError::UnsupportedContractSchema(frontmatter.schema));
    }
    if wave_id.trim().is_empty() {
        return Err(MetricError::EmptyWaveId);
    }
    require_contract_field("id", &frontmatter.id)?;
    require_contract_field("instrument", &frontmatter.instrument)?;
    require_contract_field("unit", &frontmatter.unit)?;

    let file_name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| MetricError::InvalidContractFilename(path.display().to_string()))?;
    if file_name != frontmatter.id {
        return Err(MetricError::ContractIdDoesNotMatchFilename {
            id: frontmatter.id,
            filename: file_name.to_string(),
        });
    }
    let (name, body) = parse_metric_markdown(markdown)?;

    MetricContract::new(MetricContractDefinition {
        identity: MetricIdentity::new(wave_id, frontmatter.id),
        name,
        stage: frontmatter.stage,
        instrument: frontmatter.instrument,
        unit: frontmatter.unit,
        window: MetricDuration::parse(&frontmatter.window)?,
        freshness_policy: MetricDuration::parse(&frontmatter.freshness)?,
        body,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetricContractFrontmatter {
    schema: u8,
    id: String,
    stage: MetricStage,
    instrument: String,
    unit: String,
    window: String,
    freshness: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopflow_storage::metric::MAX_METRIC_DURATION_SECONDS;
    use time::{Duration, OffsetDateTime};

    #[test]
    fn canonical_ids_metric_keys_preserve_punctuation_and_metric_names() {
        let uuid = "ABCDEF12-ABCD-4ABC-8ABC-ABCDEF123456";
        for spelling in [
            uuid.to_string(),
            uuid.replace('-', ""),
            format!("{{{uuid}}}"),
            format!("URN:UUID:{uuid}"),
        ] {
            let identity = MetricIdentity::new(&spelling, "Case-Sensitive");
            let decoded: MetricIdentity = serde_json::from_value(
                serde_json::json!({"wave_id": spelling, "metric_id": "Case-Sensitive"}),
            )
            .unwrap();
            assert_eq!(identity, decoded);
            let map = std::collections::HashMap::from([(identity, 1)]);
            assert_eq!(
                map.get(&MetricIdentity::new(
                    &spelling.to_ascii_lowercase(),
                    "Case-Sensitive"
                )),
                Some(&1)
            );
            assert!(!map.contains_key(&MetricIdentity::new(&spelling, "case-sensitive")));
            assert_eq!(decoded.wave_id(), spelling.to_ascii_lowercase());
        }
        assert_ne!(
            MetricIdentity::new(uuid, "metric"),
            MetricIdentity::new(&uuid.replace('-', ""), "metric")
        );
        assert_eq!(
            MetricIdentity::new("Historical-Wave", "metric").wave_id(),
            "Historical-Wave"
        );
    }

    fn at(hour: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::hours(hour)
    }

    fn definition() -> MetricContractDefinition {
        MetricContractDefinition {
            identity: MetricIdentity::new("product", "task-loop-trust"),
            name: "Task loops earn trust".to_string(),
            stage: MetricStage::Installed,
            instrument: "lifecycle-scorecard".to_string(),
            unit: "ratio".to_string(),

            window: MetricDuration::parse("7d").unwrap(),
            freshness_policy: MetricDuration::parse("6h").unwrap(),
            body: "Count only settled review loops.\n".to_string(),
        }
    }

    fn contract() -> MetricContract {
        MetricContract::new(definition()).unwrap()
    }

    fn contract_markdown(id: &str, instrument: &str) -> String {
        format!(
            "---\nschema: 1\nid: {id}\nstage: installed\ninstrument: {instrument}\nunit: ratio\nwindow: 7d\nfreshness: 6h\n---\n\n# Task loops earn trust\n\nCount only settled review loops.\n"
        )
    }

    fn observed(contract: &MetricContract, value: f64, end: OffsetDateTime) -> MetricObservation {
        let mut observation = MetricObservation::Observed {
            identity: contract.identity.clone().into(),
            contract_revision: contract.contract_revision.clone(),
            instrument: "lifecycle-scorecard".to_string(),
            observation_id: String::new(),
            value,
            source_window_start: end - Duration::days(7),
            source_window_end: end,
            complete: true,
        };
        recompute_observation_id(&mut observation);
        observation
    }

    fn unavailable(contract: &MetricContract, as_of: OffsetDateTime) -> MetricObservation {
        let mut observation = MetricObservation::Unavailable {
            identity: contract.identity.clone().into(),
            contract_revision: contract.contract_revision.clone(),
            instrument: contract.instrument.clone(),
            observation_id: String::new(),
            source_as_of: as_of,
            reason: "source timeout".to_string(),
        };
        recompute_observation_id(&mut observation);
        observation
    }

    fn recompute_observation_id(observation: &mut MetricObservation) {
        let observation_id = observation.expected_observation_id().unwrap();
        match observation {
            MetricObservation::Observed {
                observation_id: id, ..
            }
            | MetricObservation::Unavailable {
                observation_id: id, ..
            } => *id = observation_id,
        }
    }

    #[test]
    fn contract_revision_tracks_measurement_meaning_not_presentation_or_ownership() {
        let original = contract();
        let mut moved = definition();
        moved.name = "Renamed metric".to_string();
        moved.stage = MetricStage::Graduated;
        assert_eq!(
            MetricContract::new(moved).unwrap().contract_revision,
            original.contract_revision
        );

        let mut changed_rule = definition();
        changed_rule.body = "Count all review loops.".to_string();
        assert_ne!(
            MetricContract::new(changed_rule).unwrap().contract_revision,
            original.contract_revision
        );
    }

    #[test]
    fn duration_overflow_is_rejected_for_every_supported_unit() {
        for (suffix, seconds_per_unit) in [("m", 60_i64), ("h", 3_600), ("d", 86_400)] {
            let largest = MAX_METRIC_DURATION_SECONDS / seconds_per_unit;
            assert!(MetricDuration::parse(&format!("{largest}{suffix}")).is_ok());
            assert!(matches!(
                MetricDuration::parse(&format!("{}{suffix}", largest + 1)),
                Err(MetricError::InvalidDuration(_))
            ));
            assert!(matches!(
                MetricDuration::parse(&format!("{}{suffix}", i64::MAX)),
                Err(MetricError::InvalidDuration(_))
            ));
        }
        for malformed in ["", "7", "1s", "1💥", "💥"] {
            assert!(matches!(
                MetricDuration::parse(malformed),
                Err(MetricError::InvalidDuration(_))
            ));
        }
    }

    #[test]
    fn target_and_freshness_boundaries_are_inclusive_and_exact() {
        let contract = contract();
        let evidence = observed(&contract, 1.0, at(200));
        let fresh = derive_metric_reading(
            &contract,
            std::slice::from_ref(&evidence),
            Some(&Threshold::AtLeast { value: 1.0 }),
            at(206) - Duration::SECOND,
        )
        .unwrap();
        assert!(fresh.instrumented);
        assert!(matches!(fresh.freshness, MetricFreshnessDto::Fresh { .. }));
        assert!(matches!(fresh.evidence, MetricEvidenceDto::Met { .. }));

        let stale = derive_metric_reading(
            &contract,
            &[evidence],
            Some(&Threshold::AtLeast { value: 1.0 }),
            at(206),
        )
        .unwrap();
        assert!(matches!(stale.freshness, MetricFreshnessDto::Stale { .. }));
        assert!(matches!(
            stale.evidence,
            MetricEvidenceDto::Unknown {
                cause: MetricUnknownCauseDto::StaleObservation { .. }
            }
        ));
    }

    #[test]
    fn prior_revision_and_wrong_windows_are_unknown_without_reinterpretation() {
        let contract = contract();
        let mut prior = observed(&contract, 1.0, at(200));
        let MetricObservation::Observed {
            contract_revision, ..
        } = &mut prior
        else {
            unreachable!();
        };
        *contract_revision = "0".repeat(64);
        recompute_observation_id(&mut prior);
        let reading = derive_metric_reading(
            &contract,
            &[prior],
            Some(&Threshold::AtLeast { value: 1.0 }),
            at(201),
        )
        .unwrap();
        assert!(matches!(
            reading.evidence,
            MetricEvidenceDto::Unknown {
                cause: MetricUnknownCauseDto::RevisionMismatch { .. }
            }
        ));
        assert!(!reading.instrumented);

        let mut prior_definition = definition();
        prior_definition.freshness_policy = MetricDuration::parse("1h").unwrap();
        let prior_contract = MetricContract::new(prior_definition).unwrap();
        let prior = observed(&prior_contract, 1.0, at(200));
        let reading = derive_metric_reading(
            &contract,
            &[prior],
            Some(&Threshold::AtLeast { value: 1.0 }),
            at(206),
        )
        .unwrap();
        assert!(matches!(reading.freshness, MetricFreshnessDto::Never));
        assert!(matches!(
            reading.evidence,
            MetricEvidenceDto::Unknown {
                cause: MetricUnknownCauseDto::RevisionMismatch { .. }
            }
        ));

        let mut wrong_window = observed(&contract, 1.0, at(200));
        let MetricObservation::Observed {
            source_window_start,
            ..
        } = &mut wrong_window
        else {
            unreachable!();
        };
        *source_window_start = at(200) - Duration::days(6);
        recompute_observation_id(&mut wrong_window);
        let reading = derive_metric_reading(
            &contract,
            &[wrong_window],
            Some(&Threshold::AtLeast { value: 1.0 }),
            at(201),
        )
        .unwrap();
        assert!(matches!(
            reading.evidence,
            MetricEvidenceDto::Unknown {
                cause: MetricUnknownCauseDto::WindowMismatch { .. }
            }
        ));
    }

    #[test]
    fn validation_rejects_bad_time_and_tampered_content_ids() {
        let contract = contract();
        let first = observed(&contract, 1.0, at(200));
        first.validate(&contract, at(200)).unwrap();
        let distinct = observed(&contract, 0.5, at(200));
        distinct.validate(&contract, at(200)).unwrap();

        let mut tampered = observed(&contract, 0.5, at(201));
        let MetricObservation::Observed { observation_id, .. } = &mut tampered else {
            unreachable!();
        };
        *observation_id = "producer-chosen-id".to_string();
        assert!(matches!(
            tampered.validate(&contract, at(201)),
            Err(MetricError::InvalidObservationId { .. })
        ));

        let mut other_metric = observed(&contract, 0.5, at(201));
        let MetricObservation::Observed { identity, .. } = &mut other_metric else {
            unreachable!();
        };
        identity.metric_id = "other-metric".to_string();
        recompute_observation_id(&mut other_metric);
        assert!(matches!(
            other_metric.validate(&contract, at(201)),
            Err(MetricError::ObservationIdentityMismatch { .. })
        ));

        let future = observed(
            &contract,
            1.0,
            at(200) + Duration::minutes(5) + Duration::SECOND,
        );
        assert!(matches!(
            future.validate(&contract, at(200)),
            Err(MetricError::FutureSourceTime { .. })
        ));
    }

    #[test]
    fn source_time_and_content_digest_select_current_evidence() {
        let contract = contract();
        let missed = observed(&contract, 0.5, at(200));
        let met = observed(&contract, 1.0, at(200));
        let expected_value = if missed.observation_id() > met.observation_id() {
            0.5
        } else {
            1.0
        };
        let forward = derive_metric_reading(
            &contract,
            &[missed.clone(), met.clone()],
            Some(&Threshold::AtLeast { value: 1.0 }),
            at(201),
        )
        .unwrap();
        let reverse = derive_metric_reading(
            &contract,
            &[met, missed],
            Some(&Threshold::AtLeast { value: 1.0 }),
            at(201),
        )
        .unwrap();
        for reading in [forward, reverse] {
            let value = match reading.evidence {
                MetricEvidenceDto::Met { value, .. } | MetricEvidenceDto::Missed { value, .. } => {
                    value
                }
                evidence => panic!("expected current numeric evidence, got {evidence:?}"),
            };
            assert_eq!(value, expected_value);
        }
    }

    #[test]
    fn projection_ignores_observations_addressed_to_other_metrics() {
        let contract = contract();
        let mut other_metric = observed(&contract, 1.0, at(200));
        let MetricObservation::Observed { identity, .. } = &mut other_metric else {
            unreachable!();
        };
        identity.metric_id = "other-metric".to_string();
        recompute_observation_id(&mut other_metric);

        let reading = derive_metric_reading(
            &contract,
            &[other_metric],
            Some(&Threshold::AtLeast { value: 1.0 }),
            at(201),
        )
        .unwrap();
        assert!(!reading.instrumented);
        assert!(matches!(
            reading.evidence,
            MetricEvidenceDto::Unknown {
                cause: MetricUnknownCauseDto::Never
            }
        ));
    }

    #[test]
    fn graduation_without_qualifying_evidence_is_a_contract_issue() {
        let mut definition = definition();
        definition.stage = MetricStage::Graduated;
        let contract = MetricContract::new(definition).unwrap();
        let portfolio = derive_metric_portfolio(&[(&contract, &[])], &[], at(200)).unwrap();
        assert!(portfolio.metrics.is_empty());
        assert!(matches!(
            portfolio.contract_issues.as_slice(),
            [MetricContractIssueDto::InvalidGraduation { .. }]
        ));
    }

    #[test]
    fn graduated_metric_retains_official_state_when_evidence_later_goes_stale() {
        let mut definition = definition();
        definition.stage = MetricStage::Graduated;
        let contract = MetricContract::new(definition).unwrap();
        let evidence = observed(&contract, 1.0, at(200));

        let portfolio = derive_metric_portfolio(&[(&contract, &[evidence])], &[], at(206)).unwrap();

        assert!(portfolio.contract_issues.is_empty());
        assert!(matches!(
            portfolio.metrics.as_slice(),
            [MetricReadingDto {
                freshness: MetricFreshnessDto::Stale { .. },
                evidence: MetricEvidenceDto::Unknown {
                    cause: MetricUnknownCauseDto::StaleObservation { .. }
                },
                ..
            }]
        ));
    }

    #[test]
    fn graduated_metric_retains_official_state_during_a_current_source_failure() {
        let mut definition = definition();
        definition.stage = MetricStage::Graduated;
        let contract = MetricContract::new(definition).unwrap();
        let observations = [
            observed(&contract, 1.0, at(200)),
            unavailable(&contract, at(201)),
        ];

        let portfolio =
            derive_metric_portfolio(&[(&contract, &observations)], &[], at(202)).unwrap();

        assert!(portfolio.contract_issues.is_empty());
        assert!(matches!(
            portfolio.metrics.as_slice(),
            [MetricReadingDto {
                evidence: MetricEvidenceDto::Unavailable { reason, .. },
                ..
            }] if reason == "source timeout"
        ));
    }

    #[test]
    fn wire_uses_kind_discriminators_and_requires_variant_fields() {
        let contract = contract();
        let reading = derive_metric_reading(
            &contract,
            &[observed(&contract, 1.0, at(200))],
            Some(&Threshold::AtLeast { value: 1.0 }),
            at(201),
        )
        .unwrap();
        let json = serde_json::to_value(reading).unwrap();
        assert_eq!(json["description"], "Count only settled review loops.");
        assert_eq!(json["target"]["kind"], "at_least");
        assert_eq!(json["instrumented"], true);
        assert_eq!(json["freshness"]["kind"], "fresh");
        assert_eq!(json["evidence"]["kind"], "met");

        let missing_value = serde_json::json!({"kind": "met", "source_window_start": "1970-01-01T00:00:00Z", "source_window_end": "1970-01-01T00:00:00Z"});
        assert!(serde_json::from_value::<MetricEvidenceDto>(missing_value).is_err());

        let mut missing_instrumented = json;
        missing_instrumented
            .as_object_mut()
            .unwrap()
            .remove("instrumented");
        assert!(serde_json::from_value::<MetricReadingDto>(missing_instrumented).is_err());
    }

    #[test]
    fn discovers_valid_markdown_contracts_with_current_project() {
        let directory = tempfile::tempdir().unwrap();
        let metrics_dir = directory.path().join("metrics");
        fs::create_dir(&metrics_dir).unwrap();
        fs::write(
            metrics_dir.join("task-loop-trust.md"),
            contract_markdown("task-loop-trust", "lifecycle-scorecard"),
        )
        .unwrap();

        let discovery = discover_metric_contracts(&metrics_dir, "product").unwrap();

        assert!(discovery.contract_issues.is_empty());
        assert_eq!(discovery.contracts.len(), 1);
        let contract = &discovery.contracts[0].contract;
        assert_eq!(contract.identity.wave_id(), "product");
        assert_eq!(contract.identity.metric_id, "task-loop-trust");
        assert_eq!(contract.name, "Task loops earn trust");
    }

    #[test]
    fn fenced_examples_do_not_create_metric_headings() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("task-loop-trust.md");
        let markdown = contract_markdown("task-loop-trust", "lifecycle-scorecard").replace(
            "Count only settled review loops.",
            "Count only settled review loops.\n\n```sh\n# example output\n```",
        );
        fs::write(&path, markdown).unwrap();

        let contract = load_metric_contract(&path, "product").unwrap();

        assert!(contract.description.contains("# example output"));
    }

    #[test]
    fn malformed_and_unresolved_contracts_do_not_hide_valid_siblings() {
        let directory = tempfile::tempdir().unwrap();
        let metrics_dir = directory.path().join(".lf/wave/product/metrics");
        fs::create_dir_all(&metrics_dir).unwrap();
        fs::write(
            metrics_dir.join("task-loop-trust.md"),
            contract_markdown("task-loop-trust", "lifecycle-scorecard"),
        )
        .unwrap();
        fs::write(
            metrics_dir.join("wrong-name.md"),
            contract_markdown("different-id", "lifecycle-scorecard"),
        )
        .unwrap();
        fs::write(
            metrics_dir.join("unowned.md"),
            contract_markdown("unowned", "lifecycle-scorecard"),
        )
        .unwrap();

        let discovery = discover_metric_contracts(&metrics_dir, "product").unwrap();

        assert_eq!(discovery.contracts.len(), 2);
        assert!(discovery.contract_issues.iter().any(|issue| {
            matches!(issue, MetricContractIssueDto::MalformedContract { path, .. } if path == ".lf/wave/product/metrics/wrong-name.md")
        }));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_contract_cannot_source_meaning_outside_reviewed_metrics() {
        let directory = tempfile::tempdir().unwrap();
        let metrics_dir = directory.path().join(".lf/wave/product/metrics");
        fs::create_dir_all(&metrics_dir).unwrap();
        let external_contract = directory.path().join("external.md");
        fs::write(
            &external_contract,
            contract_markdown("external", "lifecycle-scorecard"),
        )
        .unwrap();
        std::os::unix::fs::symlink(&external_contract, metrics_dir.join("external.md")).unwrap();

        let discovery = discover_metric_contracts(&metrics_dir, "product").unwrap();

        assert!(discovery.contracts.is_empty());
        assert!(matches!(
            discovery.contract_issues.as_slice(),
            [MetricContractIssueDto::MalformedContract { path, message }]
                if path == ".lf/wave/product/metrics/external.md" && message.contains("symlink")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_metrics_directory_is_not_a_reviewed_contract_boundary() {
        let directory = tempfile::tempdir().unwrap();
        let repo = directory.path().join("repo");
        let metrics_parent = repo.join(".lf/wave/product");
        let external_metrics = directory.path().join("external-metrics");
        fs::create_dir_all(&metrics_parent).unwrap();
        fs::create_dir(&external_metrics).unwrap();
        fs::write(
            external_metrics.join("task-loop-trust.md"),
            contract_markdown("task-loop-trust", "lifecycle-scorecard"),
        )
        .unwrap();
        let metrics_dir = metrics_parent.join("metrics");
        std::os::unix::fs::symlink(&external_metrics, &metrics_dir).unwrap();

        let error = discover_metric_contracts(&metrics_dir, "product").unwrap_err();

        assert!(matches!(error, MetricError::SymlinkedContractDirectory(_)));
    }

    #[cfg(unix)]
    #[test]
    fn contract_directory_must_resolve_inside_the_repository() {
        let directory = tempfile::tempdir().unwrap();
        let repo = directory.path().join("repo");
        let external_wave = directory.path().join("external-wave");
        fs::create_dir_all(repo.join(".lf")).unwrap();
        fs::create_dir_all(external_wave.join("product/metrics")).unwrap();
        std::os::unix::fs::symlink(&external_wave, repo.join(".lf/wave")).unwrap();
        let metrics_dir = repo.join(".lf/wave/product/metrics");

        let error = discover_metric_contracts(&metrics_dir, "product").unwrap_err();

        assert!(matches!(
            error,
            MetricError::ContractDirectoryOutsideRepository { .. }
        ));
    }
}
