//! Read-only eligibility checks for replaying one recorded AgentInvocation.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::replay::{
    context_manifest_sha256, is_lowercase_sha256, read_execution_contract, read_replay_contract,
    sha256_bytes, ArtifactReferenceV1, ExecutionContractV1, LocalFileIdentityV1,
    ReplayArtifactError, ReplayContractV1, ReplayTurnV1, REPLAY_CONTRACT_SCHEMA_VERSION,
};
use crate::store::sqlite::SqliteStore;
use crate::store::{ReplayContractRow, StoreResult};
use crate::trace::{AgentInvocationRow, AgentTurnRow, ContextAsset};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ReplayRefusalCode {
    NotFound,
    AmbiguousAddress,
    CaptureIncomplete,
    MissingExecutionContract,
    ArtifactAuthorityUnavailable,
    ArtifactMissing,
    ArtifactHashMismatch,
    UnsupportedSchema,
    ContractInvalid,
    ContractIdentityMismatch,
    ConversationIncomplete,
    ConversationTimingNotReplayable,
    ManifestInvalid,
    MissingEffectiveProvider,
    MissingEffectiveModel,
    MissingRepositoryRevision,
    RepositoryRevisionUnavailable,
    SourceStateNotReplayable,
    ProviderRuntimeUnavailable,
    UnsupportedSurface,
    UnsafeExecutionBoundary,
}

impl ReplayRefusalCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::AmbiguousAddress => "ambiguous_address",
            Self::CaptureIncomplete => "capture_incomplete",
            Self::MissingExecutionContract => "missing_execution_contract",
            Self::ArtifactAuthorityUnavailable => "artifact_authority_unavailable",
            Self::ArtifactMissing => "artifact_missing",
            Self::ArtifactHashMismatch => "artifact_hash_mismatch",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::ContractInvalid => "contract_invalid",
            Self::ContractIdentityMismatch => "contract_identity_mismatch",
            Self::ConversationIncomplete => "conversation_incomplete",
            Self::ConversationTimingNotReplayable => "conversation_timing_not_replayable",
            Self::ManifestInvalid => "manifest_invalid",
            Self::MissingEffectiveProvider => "missing_effective_provider",
            Self::MissingEffectiveModel => "missing_effective_model",
            Self::MissingRepositoryRevision => "missing_repository_revision",
            Self::RepositoryRevisionUnavailable => "repository_revision_unavailable",
            Self::SourceStateNotReplayable => "source_state_not_replayable",
            Self::ProviderRuntimeUnavailable => "provider_runtime_unavailable",
            Self::UnsupportedSurface => "unsupported_surface",
            Self::UnsafeExecutionBoundary => "unsafe_execution_boundary",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayRefusal {
    pub code: ReplayRefusalCode,
    pub boundary: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayContractIndexDto {
    pub schema_version: u32,
    pub home_id: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayInputsDto {
    pub home_id: Option<String>,
    pub capture_status: String,
    pub repository_root: String,
    pub repository_commit: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub surface: String,
    pub wave: Option<String>,
    pub project: Option<String>,
    pub task: Option<String>,
    pub flow: Option<String>,
    pub skill: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayCheckDto {
    pub address: String,
    pub invocation_id: Option<String>,
    pub eligible: bool,
    pub candidates: Vec<String>,
    pub contract: Option<ReplayContractIndexDto>,
    pub inputs: Option<ReplayInputsDto>,
    pub reasons: Vec<ReplayRefusal>,
}

pub fn run(address: &str, json: bool) -> Result<()> {
    let database =
        crate::store::observability_database_path().context("resolve the selected Home ledger")?;
    let home = crate::store::observability_home_dir();
    let store = SqliteStore::open_run_ledger_read_only(&database)
        .map_err(|error| anyhow!("replay ledger unavailable: {error}"))?;
    let result = store
        .read_run_ledger_snapshot(|store| check(store, &home, address))
        .map_err(|error| anyhow!("failed to check replay eligibility: {error}"))?;

    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        _print_text(&result);
    }
    if result.eligible {
        Ok(())
    } else {
        Err(anyhow!("invocation is not replayable"))
    }
}

fn check(store: &SqliteStore, home: &Path, address: &str) -> StoreResult<ReplayCheckDto> {
    let mut candidates = if address.is_empty() {
        Vec::new()
    } else {
        store.agent_invocations_matching_address(address)?
    };
    candidates.sort_by(|left, right| left.id.cmp(&right.id));
    match candidates.as_slice() {
        [] => Ok(_unresolved(
            address,
            Vec::new(),
            ReplayRefusalCode::NotFound,
            "no invocation id matches the literal address",
        )),
        [invocation] => _check_invocation(store, home, address, invocation),
        _ => Ok(_unresolved(
            address,
            candidates
                .into_iter()
                .map(|invocation| invocation.id)
                .collect(),
            ReplayRefusalCode::AmbiguousAddress,
            "more than one invocation id matches the literal address",
        )),
    }
}

fn _unresolved(
    address: &str,
    candidates: Vec<String>,
    code: ReplayRefusalCode,
    detail: &str,
) -> ReplayCheckDto {
    ReplayCheckDto {
        address: address.to_string(),
        invocation_id: None,
        eligible: false,
        candidates,
        contract: None,
        inputs: None,
        reasons: vec![_refusal(code, "invocation.address", detail)],
    }
}

fn _check_invocation(
    store: &SqliteStore,
    home: &Path,
    address: &str,
    invocation: &AgentInvocationRow,
) -> StoreResult<ReplayCheckDto> {
    let provider = _nonempty(&invocation.provider);
    let model = invocation.model.as_deref().and_then(_nonempty);
    let mut inputs = ReplayInputsDto {
        home_id: None,
        capture_status: invocation.capture_status.clone(),
        repository_root: invocation.repo.clone(),
        repository_commit: None,
        provider: provider.map(str::to_string),
        model: model.map(str::to_string),
        surface: invocation.surface.clone(),
        wave: invocation.wave.clone(),
        project: invocation.project.clone(),
        task: invocation.task.clone(),
        flow: invocation.flow.clone(),
        skill: invocation.skill.clone(),
    };
    let mut reasons = Vec::new();

    if invocation.capture_status != "complete" {
        reasons.push(_refusal(
            ReplayRefusalCode::CaptureIncomplete,
            "invocation.capture_status",
            &format!(
                "capture status is {:?}, not complete",
                invocation.capture_status
            ),
        ));
    }
    if provider.is_none() {
        reasons.push(_refusal(
            ReplayRefusalCode::MissingEffectiveProvider,
            "provider.name",
            "the invocation has no recorded effective provider",
        ));
    }
    if model.is_none() {
        reasons.push(_refusal(
            ReplayRefusalCode::MissingEffectiveModel,
            "provider.model",
            "the invocation has no recorded effective model",
        ));
    }

    let Some(index) = store.replay_contract_for_invocation(&invocation.id)? else {
        reasons.push(_refusal(
            ReplayRefusalCode::MissingExecutionContract,
            "replay_contract",
            "the producer did not finalize a replay contract",
        ));
        reasons.push(_refusal(
            ReplayRefusalCode::MissingRepositoryRevision,
            "repository.commit",
            "legacy invocation rows contain no repository revision",
        ));
        return Ok(_resolved(address, invocation, None, inputs, reasons));
    };
    inputs.home_id = _nonempty(&index.home_id).map(str::to_string);

    let Some(index_dto) = _validate_index(&index, &mut reasons) else {
        return Ok(_resolved(address, invocation, None, inputs, reasons));
    };

    let authority_matches = match store.local_home() {
        Ok(local) if local.id.as_str() == index.home_id => true,
        Ok(local) => {
            reasons.push(_refusal(
                ReplayRefusalCode::ArtifactAuthorityUnavailable,
                "replay_contract.home_id",
                &format!(
                    "contract Home {} does not match selected Home {}",
                    index.home_id, local.id
                ),
            ));
            false
        }
        Err(error) => {
            reasons.push(_refusal(
                ReplayRefusalCode::ArtifactAuthorityUnavailable,
                "replay_contract.home_id",
                &format!("selected ledger has no readable local Home identity: {error}"),
            ));
            false
        }
    };
    if !authority_matches {
        return Ok(_resolved(
            address,
            invocation,
            Some(index_dto),
            inputs,
            reasons,
        ));
    }

    let trace_root = home.join("traces");
    let contract_ref = ArtifactReferenceV1 {
        path: index.contract_path.clone(),
        sha256: index.contract_sha256.clone(),
    };
    let Some(replay_bytes) =
        _read_trace_artifact(&trace_root, &contract_ref, "replay_contract", &mut reasons)
    else {
        return Ok(_resolved(
            address,
            invocation,
            Some(index_dto),
            inputs,
            reasons,
        ));
    };
    let replay = match read_replay_contract(&replay_bytes) {
        Ok(contract) => contract,
        Err(error) => {
            _record_artifact_decode_error(error, "replay_contract.schema_version", &mut reasons);
            return Ok(_resolved(
                address,
                invocation,
                Some(index_dto),
                inputs,
                reasons,
            ));
        }
    };
    if index.schema_version != replay.schema_version {
        reasons.push(_refusal(
            ReplayRefusalCode::ContractIdentityMismatch,
            "replay_contract.schema_version",
            &format!(
                "index schema {} disagrees with artifact schema {}",
                index.schema_version, replay.schema_version
            ),
        ));
    }
    _validate_replay_identity(&index, invocation, &replay, &mut reasons);

    let Some(execution_bytes) = _read_trace_artifact(
        &trace_root,
        &replay.execution_contract,
        "execution_contract",
        &mut reasons,
    ) else {
        return Ok(_resolved(
            address,
            invocation,
            Some(index_dto),
            inputs,
            reasons,
        ));
    };
    let execution = match read_execution_contract(&execution_bytes) {
        Ok(contract) => contract,
        Err(error) => {
            _record_artifact_decode_error(error, "execution_contract.schema_version", &mut reasons);
            return Ok(_resolved(
                address,
                invocation,
                Some(index_dto),
                inputs,
                reasons,
            ));
        }
    };
    inputs.repository_commit = _nonempty(&execution.repository.commit).map(str::to_string);
    _validate_execution_identity(invocation, &replay, &execution, &mut reasons);
    _validate_recorded_context(store, &trace_root, invocation, &replay, &mut reasons)?;
    _validate_repository(&execution, &mut reasons);
    _validate_runtime(store, invocation, &execution, &mut reasons)?;

    Ok(_resolved(
        address,
        invocation,
        Some(index_dto),
        inputs,
        reasons,
    ))
}

fn _validate_index(
    index: &ReplayContractRow,
    reasons: &mut Vec<ReplayRefusal>,
) -> Option<ReplayContractIndexDto> {
    let mut valid = true;
    if crate::durable::HomeId::parse(&index.home_id).is_err() {
        reasons.push(_refusal(
            ReplayRefusalCode::ContractInvalid,
            "replay_contract.home_id",
            "the replay index has an invalid Home identity",
        ));
        valid = false;
    }
    if crate::trace::resolve_artifact_from(Path::new("."), &index.contract_path).is_err() {
        reasons.push(_refusal(
            ReplayRefusalCode::ContractInvalid,
            "replay_contract.path",
            "the replay index path is not a safe relative artifact path",
        ));
        valid = false;
    }
    if !is_lowercase_sha256(&index.contract_sha256) {
        reasons.push(_refusal(
            ReplayRefusalCode::ContractInvalid,
            "replay_contract.sha256",
            "the replay index hash is not a lowercase SHA-256",
        ));
        valid = false;
    }
    if index.schema_version == 0 {
        reasons.push(_refusal(
            ReplayRefusalCode::ContractInvalid,
            "replay_contract.schema_version",
            "the replay index schema version must be positive",
        ));
        valid = false;
    } else if index.schema_version != REPLAY_CONTRACT_SCHEMA_VERSION {
        reasons.push(_refusal(
            ReplayRefusalCode::UnsupportedSchema,
            "replay_contract.schema_version",
            &format!(
                "replay contract schema {} is unsupported",
                index.schema_version
            ),
        ));
    }
    valid.then(|| ReplayContractIndexDto {
        schema_version: index.schema_version,
        home_id: index.home_id.clone(),
        path: index.contract_path.clone(),
        sha256: index.contract_sha256.clone(),
    })
}

fn _validate_replay_identity(
    index: &ReplayContractRow,
    invocation: &AgentInvocationRow,
    replay: &ReplayContractV1,
    reasons: &mut Vec<ReplayRefusal>,
) {
    let identities = [
        (
            "replay_contract.invocation_id",
            replay.invocation_id.as_str(),
            invocation.id.as_str(),
        ),
        (
            "replay_contract.home_id",
            replay.home_id.as_str(),
            index.home_id.as_str(),
        ),
    ];
    for (boundary, recorded, expected) in identities {
        if recorded != expected {
            reasons.push(_identity_mismatch(boundary, recorded, expected));
        }
    }
    let scopes = [
        (
            "replay_contract.wave",
            replay.wave.as_deref(),
            invocation.wave.as_deref(),
        ),
        (
            "replay_contract.project",
            replay.project.as_deref(),
            invocation.project.as_deref(),
        ),
        (
            "replay_contract.task",
            replay.task.as_deref(),
            invocation.task.as_deref(),
        ),
        (
            "replay_contract.flow",
            replay.flow.as_deref(),
            invocation.flow.as_deref(),
        ),
        (
            "replay_contract.skill",
            replay.skill.as_deref(),
            invocation.skill.as_deref(),
        ),
    ];
    for (boundary, recorded, expected) in scopes {
        if recorded != expected {
            reasons.push(_refusal(
                ReplayRefusalCode::ContractIdentityMismatch,
                boundary,
                &format!("contract value {recorded:?} disagrees with ledger value {expected:?}"),
            ));
        }
    }
}

fn _validate_execution_identity(
    invocation: &AgentInvocationRow,
    replay: &ReplayContractV1,
    execution: &ExecutionContractV1,
    reasons: &mut Vec<ReplayRefusal>,
) {
    for (boundary, recorded, expected) in [
        (
            "execution_contract.invocation_id",
            execution.invocation_id.as_str(),
            invocation.id.as_str(),
        ),
        (
            "execution_contract.home_id",
            execution.home_id.as_str(),
            replay.home_id.as_str(),
        ),
        (
            "execution_contract.repository.root",
            execution.repository.root.as_str(),
            invocation.repo.as_str(),
        ),
        (
            "execution_contract.provider.name",
            execution.provider.provider.as_str(),
            invocation.provider.as_str(),
        ),
        (
            "execution_contract.process.surface",
            execution.process.surface.as_str(),
            invocation.surface.as_str(),
        ),
        (
            "execution_contract.agent.cwd",
            execution.agent.cwd.as_str(),
            invocation.worktree.as_str(),
        ),
    ] {
        if recorded != expected {
            reasons.push(_identity_mismatch(boundary, recorded, expected));
        }
    }
    if invocation.model.as_deref() != Some(execution.provider.model.as_str()) {
        reasons.push(_refusal(
            ReplayRefusalCode::ContractIdentityMismatch,
            "execution_contract.provider.model",
            &format!(
                "contract model {:?} disagrees with ledger model {:?}",
                execution.provider.model, invocation.model
            ),
        ));
    }
    match replay.turns.first() {
        Some(initial) if initial == &execution.initial_turn => {}
        Some(_) => reasons.push(_refusal(
            ReplayRefusalCode::ContractIdentityMismatch,
            "execution_contract.initial_turn",
            "the execution contract initial Turn disagrees with the replay contract",
        )),
        None => reasons.push(_refusal(
            ReplayRefusalCode::ContractInvalid,
            "replay_contract.turns",
            "a replay contract must contain at least one Turn",
        )),
    }
}

fn _validate_recorded_context(
    store: &SqliteStore,
    trace_root: &Path,
    invocation: &AgentInvocationRow,
    replay: &ReplayContractV1,
    reasons: &mut Vec<ReplayRefusal>,
) -> StoreResult<()> {
    let invocation_ids = [invocation.id.clone()];
    let mut turns = store.agent_turns_for_invocations(&invocation_ids)?;
    turns.sort_by_key(|turn| turn.ordinal);
    let turn_ids = turns.iter().map(|turn| turn.id.clone()).collect::<Vec<_>>();
    let assets = store.context_assets_for_turns(&turn_ids)?;
    if turns.len() != replay.turns.len() {
        reasons.push(_refusal(
            ReplayRefusalCode::ConversationIncomplete,
            "replay_contract.turns",
            &format!(
                "contract has {} Turn(s), ledger has {}",
                replay.turns.len(),
                turns.len()
            ),
        ));
    }

    for (position, contract_turn) in replay.turns.iter().enumerate() {
        let Some(turn) = turns.get(position) else {
            continue;
        };
        _validate_turn_identity(turn, contract_turn, reasons);
        _validate_turn_timing(position, turn, contract_turn, reasons);
        let system = _validate_prompt(
            trace_root,
            turn.system_prompt_path.as_deref(),
            contract_turn.system_prompt.as_ref(),
            &format!("turns[{}].system_prompt", contract_turn.ordinal),
            reasons,
        );
        let task = _validate_prompt(
            trace_root,
            Some(&turn.task_prompt_path),
            Some(&contract_turn.task_prompt),
            &format!("turns[{}].task_prompt", contract_turn.ordinal),
            reasons,
        );
        let turn_assets = assets
            .iter()
            .filter(|row| row.turn_id == turn.id)
            .map(|row| row.asset.clone())
            .collect::<Vec<_>>();
        _validate_manifest(
            contract_turn,
            &turn_assets,
            system.as_deref(),
            task.as_deref(),
            reasons,
        );
    }

    _validate_conversation(trace_root, invocation, replay, &turns, reasons);
    Ok(())
}

fn _validate_turn_identity(
    turn: &AgentTurnRow,
    contract: &ReplayTurnV1,
    reasons: &mut Vec<ReplayRefusal>,
) {
    if contract.turn_id != turn.id || i64::from(contract.ordinal) != turn.ordinal {
        reasons.push(_refusal(
            ReplayRefusalCode::ConversationIncomplete,
            &format!("turns[{}].identity", contract.ordinal),
            &format!(
                "contract Turn {} ordinal {} disagrees with ledger Turn {} ordinal {}",
                contract.turn_id, contract.ordinal, turn.id, turn.ordinal
            ),
        ));
    }
    if contract.input_op != turn.input_op {
        reasons.push(_refusal(
            ReplayRefusalCode::ContractIdentityMismatch,
            &format!("turns[{}].input_op", contract.ordinal),
            &format!(
                "contract input operation {:?} disagrees with ledger operation {:?}",
                contract.input_op, turn.input_op
            ),
        ));
    }
    if !matches!(turn.status.as_str(), "completed" | "failed") {
        reasons.push(_refusal(
            ReplayRefusalCode::ConversationIncomplete,
            &format!("turns[{}].status", contract.ordinal),
            &format!("Turn status {:?} is not terminal and complete", turn.status),
        ));
    }
}

fn _validate_turn_timing(
    position: usize,
    turn: &AgentTurnRow,
    contract: &ReplayTurnV1,
    reasons: &mut Vec<ReplayRefusal>,
) {
    let supported = if position == 0 {
        turn.input_op == "initial" && contract.timing == "initial"
    } else {
        turn.input_op == "message" && contract.timing == "turn_boundary"
    };
    if !supported {
        reasons.push(_refusal(
            ReplayRefusalCode::ConversationTimingNotReplayable,
            &format!("turns[{}].timing", contract.ordinal),
            &format!(
                "input operation {:?} at timing {:?} is not a supported Turn boundary",
                turn.input_op, contract.timing
            ),
        ));
    }
}

fn _validate_prompt(
    trace_root: &Path,
    ledger_path: Option<&str>,
    reference: Option<&ArtifactReferenceV1>,
    boundary: &str,
    reasons: &mut Vec<ReplayRefusal>,
) -> Option<Vec<u8>> {
    match (ledger_path, reference) {
        (None, None) => None,
        (Some(ledger), Some(reference)) => {
            if ledger != reference.path {
                reasons.push(_refusal(
                    ReplayRefusalCode::ContractIdentityMismatch,
                    boundary,
                    &format!(
                        "contract path {:?} disagrees with ledger path {:?}",
                        reference.path, ledger
                    ),
                ));
            }
            _read_trace_artifact(trace_root, reference, boundary, reasons)
        }
        _ => {
            reasons.push(_refusal(
                ReplayRefusalCode::ContractIdentityMismatch,
                boundary,
                "contract and ledger disagree about whether the prompt exists",
            ));
            None
        }
    }
}

fn _validate_manifest(
    contract: &ReplayTurnV1,
    assets: &[ContextAsset],
    system: Option<&[u8]>,
    task: Option<&[u8]>,
    reasons: &mut Vec<ReplayRefusal>,
) {
    let boundary = format!("turns[{}].context_manifest", contract.ordinal);
    let expected_count =
        usize::try_from(contract.context_manifest.asset_count).unwrap_or(usize::MAX);
    if assets.len() != expected_count {
        reasons.push(_refusal(
            ReplayRefusalCode::ManifestInvalid,
            &boundary,
            &format!(
                "contract names {} asset(s), ledger has {}",
                contract.context_manifest.asset_count,
                assets.len()
            ),
        ));
    }
    if !is_lowercase_sha256(&contract.context_manifest.sha256)
        || context_manifest_sha256(assets) != contract.context_manifest.sha256
    {
        reasons.push(_refusal(
            ReplayRefusalCode::ManifestInvalid,
            &boundary,
            "ordered context manifest identity does not match the ledger",
        ));
    }
    if assets
        .iter()
        .enumerate()
        .any(|(position, asset)| asset.position as usize != position)
    {
        reasons.push(_refusal(
            ReplayRefusalCode::ManifestInvalid,
            &boundary,
            "context asset positions are not complete and ordered",
        ));
    }
    _validate_manifest_channel(
        &boundary,
        "system",
        system,
        assets
            .iter()
            .filter(|asset| asset.channel.as_str() == "system"),
        reasons,
    );
    _validate_manifest_channel(
        &boundary,
        "task",
        task,
        assets
            .iter()
            .filter(|asset| asset.channel.as_str() == "task"),
        reasons,
    );
}

fn _validate_manifest_channel<'a>(
    boundary: &str,
    channel: &str,
    prompt: Option<&[u8]>,
    assets: impl Iterator<Item = &'a ContextAsset>,
    reasons: &mut Vec<ReplayRefusal>,
) {
    let assets = assets.collect::<Vec<_>>();
    let Some(prompt) = prompt else {
        if !assets.is_empty() {
            reasons.push(_refusal(
                ReplayRefusalCode::ManifestInvalid,
                boundary,
                &format!("{channel} manifest exists without a prompt"),
            ));
        }
        return;
    };
    let mut cursor = 0_usize;
    for asset in &assets {
        let Ok(start) = usize::try_from(asset.byte_start) else {
            cursor = usize::MAX;
            break;
        };
        let Ok(end) = usize::try_from(asset.byte_end) else {
            cursor = usize::MAX;
            break;
        };
        if start != cursor
            || end < start
            || end > prompt.len()
            || asset.bytes != (end - start) as u64
            || sha256_bytes(&prompt[start..end]) != asset.content_sha256
        {
            cursor = usize::MAX;
            break;
        }
        cursor = end;
    }
    if assets.is_empty() || cursor != prompt.len() {
        reasons.push(_refusal(
            ReplayRefusalCode::ManifestInvalid,
            boundary,
            &format!("{channel} manifest does not cover the exact prompt bytes"),
        ));
    }
}

fn _validate_conversation(
    trace_root: &Path,
    invocation: &AgentInvocationRow,
    replay: &ReplayContractV1,
    turns: &[AgentTurnRow],
    reasons: &mut Vec<ReplayRefusal>,
) {
    let reference = ArtifactReferenceV1 {
        path: replay.conversation.path.clone(),
        sha256: replay.conversation.sha256.clone(),
    };
    if reference.path != invocation.conversation_path {
        reasons.push(_refusal(
            ReplayRefusalCode::ContractIdentityMismatch,
            "conversation.path",
            &format!(
                "contract path {:?} disagrees with ledger path {:?}",
                reference.path, invocation.conversation_path
            ),
        ));
    }
    let Some(bytes) = _read_trace_artifact(trace_root, &reference, "conversation", reasons) else {
        return;
    };
    if replay.conversation.trace_schema_version != crate::trace::TRACE_SCHEMA_VERSION {
        reasons.push(_refusal(
            ReplayRefusalCode::UnsupportedSchema,
            "conversation.trace_schema_version",
            &format!(
                "trace schema {} is unsupported",
                replay.conversation.trace_schema_version
            ),
        ));
        return;
    }
    let path = match crate::trace::resolve_artifact_from(trace_root, &reference.path) {
        Ok(path) => path,
        Err(_) => return,
    };
    let conversation = match crate::trace::read_conversation_status(&path) {
        Ok(conversation) => conversation,
        Err(error)
            if error
                .to_string()
                .contains("unsupported trace schema version") =>
        {
            reasons.push(_refusal(
                ReplayRefusalCode::UnsupportedSchema,
                "conversation.trace_schema_version",
                &error.to_string(),
            ));
            return;
        }
        Err(error) => {
            reasons.push(_refusal(
                ReplayRefusalCode::ConversationIncomplete,
                "conversation",
                &format!("normalized conversation is unreadable: {error}"),
            ));
            return;
        }
    };
    if conversation.incomplete_tail {
        reasons.push(_refusal(
            ReplayRefusalCode::ConversationIncomplete,
            "conversation.tail",
            "normalized conversation ends with an incomplete JSONL record",
        ));
    }
    let actual_count = conversation.events.len() as u64;
    if replay.conversation.event_count != actual_count
        || invocation.conversation_event_count < 0
        || invocation.conversation_event_count as u64 != actual_count
    {
        reasons.push(_refusal(
            ReplayRefusalCode::ConversationIncomplete,
            "conversation.event_count",
            &format!(
                "contract count {}, ledger count {}, and artifact count {} do not agree",
                replay.conversation.event_count, invocation.conversation_event_count, actual_count
            ),
        ));
    }
    if replay.conversation.bytes != bytes.len() as u64
        || invocation.conversation_bytes < 0
        || invocation.conversation_bytes as u64 != bytes.len() as u64
    {
        reasons.push(_refusal(
            ReplayRefusalCode::ConversationIncomplete,
            "conversation.bytes",
            &format!(
                "contract bytes {}, ledger bytes {}, and artifact bytes {} do not agree",
                replay.conversation.bytes,
                invocation.conversation_bytes,
                bytes.len()
            ),
        ));
    }
    if conversation
        .events
        .iter()
        .enumerate()
        .any(|(seq, event)| event.seq != seq as u64)
    {
        reasons.push(_refusal(
            ReplayRefusalCode::ConversationIncomplete,
            "conversation.sequence",
            "normalized conversation sequence is not contiguous from zero",
        ));
    }
    let turn_ids = turns
        .iter()
        .map(|turn| turn.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut observed = BTreeMap::<String, (u64, u64)>::new();
    for event in &conversation.events {
        let Some(turn_id) = event.turn_id.as_ref().map(ToString::to_string) else {
            continue;
        };
        if !turn_ids.contains(turn_id.as_str()) {
            reasons.push(_refusal(
                ReplayRefusalCode::ConversationIncomplete,
                "conversation.turn_id",
                &format!(
                    "conversation event {} names unknown Turn {turn_id}",
                    event.seq
                ),
            ));
            continue;
        }
        observed
            .entry(turn_id)
            .and_modify(|range| range.1 = event.seq)
            .or_insert((event.seq, event.seq));
    }
    for turn in turns {
        let recorded = observed.get(&turn.id).copied();
        let ledger = turn
            .first_event_seq
            .zip(turn.last_event_seq)
            .and_then(|(first, last)| {
                Some((u64::try_from(first).ok()?, u64::try_from(last).ok()?))
            });
        if recorded != ledger {
            reasons.push(_refusal(
                ReplayRefusalCode::ConversationIncomplete,
                &format!("turns[{}].event_range", turn.ordinal),
                &format!(
                    "conversation event range {recorded:?} disagrees with ledger range {ledger:?}"
                ),
            ));
        }
    }
}

fn _validate_repository(execution: &ExecutionContractV1, reasons: &mut Vec<ReplayRefusal>) {
    if _nonempty(&execution.repository.commit).is_none() {
        reasons.push(_refusal(
            ReplayRefusalCode::MissingRepositoryRevision,
            "repository.commit",
            "the execution contract has no immutable repository commit",
        ));
    }
    if !execution.repository.clean {
        reasons.push(_refusal(
            ReplayRefusalCode::SourceStateNotReplayable,
            "repository.clean",
            "the recorded source state was dirty",
        ));
    }
    if _nonempty(&execution.repository.commit).is_some() {
        let root = Path::new(&execution.repository.root);
        let object = format!("{}^{{commit}}", execution.repository.commit);
        let available = root.is_absolute()
            && root.is_dir()
            && Command::new("git")
                .args(["-C"])
                .arg(root)
                .args(["cat-file", "-e", &object])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success());
        if !available {
            reasons.push(_refusal(
                ReplayRefusalCode::RepositoryRevisionUnavailable,
                "repository.commit",
                &format!(
                    "commit {:?} is not available in recorded repository {}",
                    execution.repository.commit, execution.repository.root
                ),
            ));
        }
    }
}

fn _validate_runtime(
    store: &SqliteStore,
    invocation: &AgentInvocationRow,
    execution: &ExecutionContractV1,
    reasons: &mut Vec<ReplayRefusal>,
) -> StoreResult<()> {
    if _nonempty(&execution.provider.provider).is_none()
        || _nonempty(&execution.provider.model).is_none()
        || _nonempty(&execution.provider.account_id).is_none()
        || _nonempty(&execution.agent.agent).is_none()
        || execution.sanitized_argv.is_empty()
    {
        reasons.push(_refusal(
            ReplayRefusalCode::ContractInvalid,
            "execution_contract.provider",
            "provider, model, account, agent, and argv must all be recorded",
        ));
    }
    match crate::store::ProviderAccountId::parse(&execution.provider.account_id) {
        Ok(account_id) => {
            match store.get_provider_account(&execution.provider.provider, &account_id)? {
                Some(account)
                    if account.credential_state == crate::store::CredentialState::Connected => {}
                Some(_) => reasons.push(_refusal(
                    ReplayRefusalCode::ProviderRuntimeUnavailable,
                    "provider.account_id",
                    &format!(
                        "managed {} account {:?} is not connected",
                        execution.provider.provider, execution.provider.account_id
                    ),
                )),
                None => reasons.push(_refusal(
                    ReplayRefusalCode::ProviderRuntimeUnavailable,
                    "provider.account_id",
                    &format!(
                        "managed {} account {:?} is not present in the selected ledger",
                        execution.provider.provider, execution.provider.account_id
                    ),
                )),
            }
        }
        Err(_) => reasons.push(_refusal(
            ReplayRefusalCode::ContractInvalid,
            "provider.account_id",
            "the recorded provider account identity is invalid",
        )),
    }
    _validate_runtime_file(&execution.provider.binary, "provider.binary", reasons);
    if execution.provider.config_files.is_empty() {
        reasons.push(_refusal(
            ReplayRefusalCode::ContractInvalid,
            "provider.config_files",
            "the execution contract has no provider config identity",
        ));
    }
    let mut config_files = execution.provider.config_files.iter().collect::<Vec<_>>();
    config_files.sort_by(|left, right| left.path.cmp(&right.path));
    for config in config_files {
        _validate_runtime_file(config, "provider.config_files", reasons);
    }

    if invocation.surface != "headless"
        || execution.process.surface != "headless"
        || !execution.process.unattended
    {
        reasons.push(_refusal(
            ReplayRefusalCode::UnsupportedSurface,
            "execution_contract.process.surface",
            "schema V1 supports only unattended headless execution",
        ));
    }
    for (unsafe_boundary, unsafe_state, detail) in [
        (
            "execution_contract.agent.permission_policy",
            execution.agent.permission_policy != "managed",
            "permission policy is not the managed replay policy",
        ),
        (
            "execution_contract.agent.write_scope",
            execution.agent.write_scope != "worktree",
            "filesystem writes are not limited to the assigned worktree",
        ),
        (
            "execution_contract.agent.writable_roots",
            !execution.agent.writable_roots.is_empty(),
            "external writable roots were recorded",
        ),
        (
            "execution_contract.agent.network_access",
            execution.agent.network_access,
            "tool network access was recorded",
        ),
        (
            "execution_contract.agent.skip_permissions",
            execution.agent.skip_permissions,
            "provider permission enforcement was disabled",
        ),
        (
            "execution_contract.agent.directive_relay",
            execution.agent.directive_relay.is_some(),
            "an external directive relay was writable",
        ),
    ] {
        if unsafe_state {
            reasons.push(_refusal(
                ReplayRefusalCode::UnsafeExecutionBoundary,
                unsafe_boundary,
                detail,
            ));
        }
    }
    if !matches!(execution.agent.run_context.as_str(), "inherit" | "detached")
        || !matches!(execution.process.stream_format.as_str(), "raw" | "human")
        || execution.agent.max_turns == Some(0)
        || execution.process.timeout_ms == Some(0)
    {
        reasons.push(_refusal(
            ReplayRefusalCode::ContractInvalid,
            "execution_contract.settings",
            "the execution contract contains an invalid runtime setting",
        ));
    }
    for reply in &execution.agent.structured_replies {
        if _nonempty(&reply.name).is_none()
            || _nonempty(&reply.description).is_none()
            || _nonempty(&reply.guidance).is_none()
        {
            reasons.push(_refusal(
                ReplayRefusalCode::ContractInvalid,
                "execution_contract.agent.structured_replies",
                "a structured reply contract is incomplete",
            ));
        }
    }
    const ENVIRONMENT_ALLOWLIST: [&str; 11] = [
        "PATH",
        "SHELL",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "TERM",
        "COLORTERM",
        "NO_COLOR",
        "TZ",
        "CODEX_HOME",
        "CLAUDE_CONFIG_DIR",
    ];
    for name in execution.environment_selectors.keys() {
        let upper = name.to_ascii_uppercase();
        if name.trim().is_empty()
            || !ENVIRONMENT_ALLOWLIST.contains(&name.as_str())
            || ["TOKEN", "KEY", "SECRET", "PASSWORD", "CREDENTIAL", "AUTH"]
                .iter()
                .any(|marker| upper.contains(marker))
        {
            reasons.push(_refusal(
                ReplayRefusalCode::UnsafeExecutionBoundary,
                "execution_contract.environment_selectors",
                &format!("environment selector {name:?} is not allowlisted or is secret-bearing"),
            ));
        }
    }
    Ok(())
}

fn _validate_runtime_file(
    identity: &LocalFileIdentityV1,
    boundary: &str,
    reasons: &mut Vec<ReplayRefusal>,
) {
    if !is_lowercase_sha256(&identity.sha256) || !Path::new(&identity.path).is_absolute() {
        reasons.push(_refusal(
            ReplayRefusalCode::ContractInvalid,
            boundary,
            &format!(
                "runtime identity {:?} is not an absolute path and lowercase SHA-256",
                identity.path
            ),
        ));
        return;
    }
    let bytes = match fs::read(&identity.path) {
        Ok(bytes) => bytes,
        Err(error) => {
            reasons.push(_refusal(
                ReplayRefusalCode::ArtifactMissing,
                boundary,
                &format!(
                    "recorded runtime artifact {} is unavailable: {error}",
                    identity.path
                ),
            ));
            return;
        }
    };
    if sha256_bytes(&bytes) != identity.sha256 {
        reasons.push(_refusal(
            ReplayRefusalCode::ArtifactHashMismatch,
            boundary,
            &format!("recorded runtime artifact {} has changed", identity.path),
        ));
    }
}

fn _read_trace_artifact(
    trace_root: &Path,
    reference: &ArtifactReferenceV1,
    boundary: &str,
    reasons: &mut Vec<ReplayRefusal>,
) -> Option<Vec<u8>> {
    if !is_lowercase_sha256(&reference.sha256) {
        reasons.push(_refusal(
            ReplayRefusalCode::ContractInvalid,
            &format!("{boundary}.sha256"),
            "artifact identity is not a lowercase SHA-256",
        ));
        return None;
    }
    let path = match crate::trace::resolve_artifact_from(trace_root, &reference.path) {
        Ok(path) => path,
        Err(_) => {
            reasons.push(_refusal(
                ReplayRefusalCode::ContractInvalid,
                &format!("{boundary}.path"),
                &format!(
                    "artifact path {:?} is not a safe relative path",
                    reference.path
                ),
            ));
            return None;
        }
    };
    if !path.exists() {
        reasons.push(_refusal(
            ReplayRefusalCode::ArtifactMissing,
            boundary,
            &format!("artifact {} is unavailable", reference.path),
        ));
        return None;
    }
    if !_artifact_is_below(trace_root, &path) {
        reasons.push(_refusal(
            ReplayRefusalCode::ContractInvalid,
            &format!("{boundary}.path"),
            &format!(
                "artifact path {:?} escapes the selected Home trace root",
                reference.path
            ),
        ));
        return None;
    }
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            reasons.push(_refusal(
                ReplayRefusalCode::ArtifactMissing,
                boundary,
                &format!("artifact {} is unavailable: {error}", reference.path),
            ));
            return None;
        }
    };
    if sha256_bytes(&bytes) != reference.sha256 {
        reasons.push(_refusal(
            ReplayRefusalCode::ArtifactHashMismatch,
            boundary,
            &format!(
                "artifact {} does not match its recorded SHA-256",
                reference.path
            ),
        ));
        return None;
    }
    Some(bytes)
}

fn _artifact_is_below(root: &Path, path: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    path.canonicalize().is_ok_and(|path| path.starts_with(root))
}

fn _record_artifact_decode_error(
    error: ReplayArtifactError,
    boundary: &str,
    reasons: &mut Vec<ReplayRefusal>,
) {
    match error {
        ReplayArtifactError::UnsupportedSchema(version) => reasons.push(_refusal(
            ReplayRefusalCode::UnsupportedSchema,
            boundary,
            &format!("artifact schema {version} is unsupported"),
        )),
        ReplayArtifactError::Invalid(detail) => reasons.push(_refusal(
            ReplayRefusalCode::ContractInvalid,
            boundary,
            &detail,
        )),
    }
}

fn _resolved(
    address: &str,
    invocation: &AgentInvocationRow,
    contract: Option<ReplayContractIndexDto>,
    inputs: ReplayInputsDto,
    reasons: Vec<ReplayRefusal>,
) -> ReplayCheckDto {
    ReplayCheckDto {
        address: address.to_string(),
        invocation_id: Some(invocation.id.clone()),
        eligible: reasons.is_empty(),
        candidates: Vec::new(),
        contract,
        inputs: Some(inputs),
        reasons,
    }
}

fn _identity_mismatch(boundary: &str, recorded: &str, expected: &str) -> ReplayRefusal {
    _refusal(
        ReplayRefusalCode::ContractIdentityMismatch,
        boundary,
        &format!("contract value {recorded:?} disagrees with recorded value {expected:?}"),
    )
}

fn _refusal(code: ReplayRefusalCode, boundary: &str, detail: &str) -> ReplayRefusal {
    ReplayRefusal {
        code,
        boundary: boundary.to_string(),
        detail: detail.to_string(),
    }
}

fn _nonempty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn _print_text(result: &ReplayCheckDto) {
    println!(
        "replay check {}  {}",
        result.address,
        if result.eligible {
            "eligible"
        } else {
            "refused"
        }
    );
    println!(
        "  invocation  {}",
        result.invocation_id.as_deref().unwrap_or("-")
    );
    if !result.candidates.is_empty() {
        println!("  candidates  {}", result.candidates.join(", "));
    }
    if let Some(inputs) = &result.inputs {
        println!("  capture     {}", inputs.capture_status);
        println!(
            "  provider    {}:{}",
            inputs.provider.as_deref().unwrap_or("-"),
            inputs.model.as_deref().unwrap_or("-")
        );
        println!(
            "  repository  {}@{}",
            inputs.repository_root,
            inputs.repository_commit.as_deref().unwrap_or("-")
        );
    }
    for reason in &result.reasons {
        println!(
            "  {}  {} — {}",
            reason.code.as_str(),
            reason.boundary,
            reason.detail
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_code_has_a_stable_snake_case_name() {
        let codes = [
            ReplayRefusalCode::NotFound,
            ReplayRefusalCode::AmbiguousAddress,
            ReplayRefusalCode::CaptureIncomplete,
            ReplayRefusalCode::MissingExecutionContract,
            ReplayRefusalCode::ArtifactAuthorityUnavailable,
            ReplayRefusalCode::ArtifactMissing,
            ReplayRefusalCode::ArtifactHashMismatch,
            ReplayRefusalCode::UnsupportedSchema,
            ReplayRefusalCode::ContractInvalid,
            ReplayRefusalCode::ContractIdentityMismatch,
            ReplayRefusalCode::ConversationIncomplete,
            ReplayRefusalCode::ConversationTimingNotReplayable,
            ReplayRefusalCode::ManifestInvalid,
            ReplayRefusalCode::MissingEffectiveProvider,
            ReplayRefusalCode::MissingEffectiveModel,
            ReplayRefusalCode::MissingRepositoryRevision,
            ReplayRefusalCode::RepositoryRevisionUnavailable,
            ReplayRefusalCode::SourceStateNotReplayable,
            ReplayRefusalCode::ProviderRuntimeUnavailable,
            ReplayRefusalCode::UnsupportedSurface,
            ReplayRefusalCode::UnsafeExecutionBoundary,
        ];
        for code in codes {
            assert_eq!(
                serde_json::to_string(&code).unwrap(),
                format!("\"{}\"", code.as_str())
            );
        }
    }
}
