//! Narrowly scoped client for Disk Guardian's local socket.
//!
//! This module deliberately exposes only typed status, diagnostics, monitoring
//! threshold policy, and authorized scan lifecycle operations. It cannot send
//! arbitrary request types or payloads to Guardian.

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
    time::timeout,
};

const PROTOCOL_VERSION: u8 = 1;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_JS_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_SECURITY_KEYS_OR_PROFILES: u64 = 256;
const TOKEN_FILE: &str = "cenno.ipc-token";
const SOCKET_FILE: &str = "guardian.sock";
static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
struct GuardianPaths {
    directory: PathBuf,
    token: PathBuf,
    socket: PathBuf,
}

impl GuardianPaths {
    fn default_paths() -> Result<Self, String> {
        let home = dirs::home_dir().ok_or_else(|| "home directory is unavailable".to_owned())?;
        Ok(Self::under(
            home.join("Library/Application Support/app.disk-guardian"),
        ))
    }

    fn under(directory: PathBuf) -> Self {
        Self {
            token: directory.join(TOKEN_FILE),
            socket: directory.join(SOCKET_FILE),
            directory,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum GuardianRequestKind {
    GetStatus,
    GetDiagnostics,
    GetRuns,
    GetSecurityStatus,
    GetMonitoringPolicy,
    UpdateMonitoringPolicy,
    StartScan,
    CancelRun,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GuardianRequest<'a> {
    version: u8,
    #[serde(rename = "type")]
    kind: GuardianRequestKind,
    request_id: &'a str,
    token: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    authorization_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    policy: Option<&'a GuardianMonitoringPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuardianMonitoringPolicy {
    pub warning_bytes: String,
    pub critical_bytes: String,
    pub warning_basis_points: u32,
    pub critical_basis_points: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuardianMonitoringPolicySnapshot {
    pub schema_version: u8,
    pub revision: u64,
    pub policy: GuardianMonitoringPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuardianSecurityStatus {
    pub schema_version: u8,
    pub reported_by: GuardianSecurityReporter,
    pub execution: GuardianExecutionSecurityStatus,
    pub personal_contracts_authority: GuardianAuthoritySecurityStatus,
    pub executor_identity: GuardianExecutorSecurityStatus,
    pub profile_lifecycle: GuardianProfileLifecycleSecurityStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GuardianSecurityReporter {
    DiskGuardian,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GuardianSubsystemStatus {
    Available,
    Degraded,
    Unavailable,
    Uninitialized,
    Active,
    PendingRotation,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuardianExecutionSecurityStatus {
    pub status: GuardianSubsystemStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuardianSecurityKey {
    pub key_id: String,
    pub public_key_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuardianAuthoritySecurityStatus {
    pub status: GuardianSubsystemStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_key: Option<GuardianSecurityKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_key: Option<GuardianSecurityKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retired_key_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_key_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuardianExecutorSecurityStatus {
    pub status: GuardianSubsystemStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_key: Option<GuardianSecurityKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_key: Option<GuardianSecurityKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuardianProfileLifecycleHead {
    pub generation: u64,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuardianProfileLifecycleSecurityStatus {
    pub status: GuardianSubsystemStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<GuardianProfileLifecycleHead>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retired_profile_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GuardianStatus {
    pub schema_version: u8,
    pub generated_at: String,
    pub sample: DiskSample,
    pub evaluation: DiskRiskEvaluation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiskSample {
    pub schema_version: u8,
    pub target_id: String,
    pub observed_at: String,
    pub total_bytes: String,
    pub available_bytes: String,
    pub free_bytes: String,
    pub block_size_bytes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiskRiskEvaluation {
    pub severity: DiskSeverity,
    pub reasons: Vec<String>,
    pub evaluated_at: String,
    pub latest_sample_at: String,
    pub available_bytes: String,
    pub available_basis_points: u32,
    #[serde(default)]
    pub trend: Option<DiskTrend>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiskSeverity {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiskTrend {
    pub window_ms: u64,
    pub drop_bytes: String,
    pub time_to_critical_ms: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GuardianDiagnostics {
    pub schema_version: u8,
    pub generated_at: String,
    pub report: DiagnosticReport,
    #[serde(default)]
    pub growth: Option<DiagnosticGrowth>,
    pub preview: DiagnosticPreview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    pub schema_version: u8,
    pub observed_at: String,
    pub scopes: Vec<DiagnosticScopeSummary>,
    pub totals: DiagnosticTotals,
    pub top_candidates: Vec<DiagnosticCandidate>,
    pub partial: bool,
    pub limits_hit: Vec<String>,
    pub error_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticScopeSummary {
    pub id: DiagnosticScope,
    pub files: u64,
    pub folders: u64,
    pub bytes: String,
    pub readable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticTotals {
    pub files: u64,
    pub folders: u64,
    pub bytes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticCandidate {
    pub scope: DiagnosticScope,
    pub kind: DiagnosticKind,
    pub relative_path: String,
    pub bytes: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiagnosticScope {
    #[serde(rename = "user.caches")]
    UserCaches,
    #[serde(rename = "user.downloads")]
    UserDownloads,
    #[serde(rename = "user.developer-builds")]
    UserDeveloperBuilds,
    #[serde(rename = "user.trash")]
    UserTrash,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticKind {
    File,
    Folder,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticGrowth {
    pub schema_version: u8,
    pub previous_observed_at: String,
    pub current_observed_at: String,
    pub totals: DiagnosticGrowthTotals,
    pub candidates: Vec<DiagnosticGrowthCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticGrowthTotals {
    pub files_delta: i64,
    pub bytes_delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticGrowthCandidate {
    pub scope: DiagnosticScope,
    pub relative_path: String,
    pub previous_bytes: String,
    pub current_bytes: String,
    pub delta_bytes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticPreview {
    pub schema_version: u8,
    pub generated_from: String,
    pub preview_ref: String,
    pub estimated_bytes: String,
    pub candidates: Vec<DiagnosticCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuardianRuns {
    pub schema_version: u8,
    pub sequence: u64,
    pub runs: Vec<GuardianRun>,
    pub events: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuardianRun {
    pub authorization_id: String,
    pub profile_id: String,
    pub profile_version: u64,
    pub expires_at: String,
    pub status: GuardianRunStatus,
    pub progress: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GuardianRunStatus {
    Imported,
    Claimed,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuardianStartScanResult {
    pub authorization_id: String,
    pub status: GuardianStartedStatus,
    pub started_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GuardianStartedStatus {
    Running,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuardianCancelRunResult {
    pub authorization_id: String,
    pub status: GuardianCancelledStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GuardianCancelledStatus {
    Cancelled,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GuardianEnvelope<T> {
    version: u8,
    request_id: String,
    ok: bool,
    data: Option<T>,
    error: Option<GuardianServerError>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GuardianServerError {
    code: String,
    message: String,
}

#[tauri::command]
pub async fn guardian_get_status() -> Result<GuardianStatus, String> {
    request_default(GuardianRequestKind::GetStatus, None).await
}

#[tauri::command]
pub async fn guardian_get_diagnostics() -> Result<GuardianDiagnostics, String> {
    request_default(GuardianRequestKind::GetDiagnostics, None).await
}

#[tauri::command]
pub async fn guardian_get_runs() -> Result<GuardianRuns, String> {
    let runs: GuardianRuns = request_default(GuardianRequestKind::GetRuns, None).await?;
    validate_runs(&runs)?;
    Ok(runs)
}

#[tauri::command]
pub async fn guardian_get_security_status() -> Result<GuardianSecurityStatus, String> {
    let status: GuardianSecurityStatus =
        request_default(GuardianRequestKind::GetSecurityStatus, None).await?;
    validate_security_status(&status)?;
    Ok(status)
}

#[tauri::command]
pub async fn guardian_get_monitoring_policy() -> Result<GuardianMonitoringPolicySnapshot, String> {
    let snapshot = request_default(GuardianRequestKind::GetMonitoringPolicy, None).await?;
    validate_monitoring_policy_snapshot(&snapshot)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn guardian_update_monitoring_policy(
    expected_revision: u64,
    policy: GuardianMonitoringPolicy,
) -> Result<GuardianMonitoringPolicySnapshot, String> {
    validate_monitoring_policy(&policy)?;
    let paths = GuardianPaths::default_paths()?;
    let request_id = new_request_id();
    let snapshot =
        request_monitoring_policy_update(&paths, expected_revision, &policy, &request_id).await?;
    validate_monitoring_policy_snapshot(&snapshot)?;
    let next_revision = expected_revision
        .checked_add(1)
        .ok_or_else(|| "Guardian monitoring policy revision cannot advance".to_owned())?;
    if snapshot.revision != next_revision || snapshot.policy != policy {
        return Err(
            "Guardian monitoring policy update response does not match the request".to_owned(),
        );
    }
    Ok(snapshot)
}

#[tauri::command]
pub async fn guardian_start_scan(
    authorization_id: String,
) -> Result<GuardianStartScanResult, String> {
    validate_authorization_id(&authorization_id)?;
    let result: GuardianStartScanResult =
        request_default(GuardianRequestKind::StartScan, Some(&authorization_id)).await?;
    validate_start_scan_result(&result, &authorization_id)?;
    Ok(result)
}

#[tauri::command]
pub async fn guardian_cancel_run(
    authorization_id: String,
) -> Result<GuardianCancelRunResult, String> {
    validate_authorization_id(&authorization_id)?;
    let result: GuardianCancelRunResult =
        request_default(GuardianRequestKind::CancelRun, Some(&authorization_id)).await?;
    validate_cancel_run_result(&result, &authorization_id)?;
    Ok(result)
}

async fn request_default<T: DeserializeOwned>(
    kind: GuardianRequestKind,
    authorization_id: Option<&str>,
) -> Result<T, String> {
    let paths = GuardianPaths::default_paths()?;
    let request_id = new_request_id();
    match authorization_id {
        Some(authorization_id) => {
            request_with_authorization(&paths, kind, Some(authorization_id), &request_id).await
        }
        None => request(&paths, kind, &request_id).await,
    }
}

async fn request<T: DeserializeOwned>(
    paths: &GuardianPaths,
    kind: GuardianRequestKind,
    request_id: &str,
) -> Result<T, String> {
    request_with_authorization(paths, kind, None, request_id).await
}

async fn request_with_authorization<T: DeserializeOwned>(
    paths: &GuardianPaths,
    kind: GuardianRequestKind,
    authorization_id: Option<&str>,
    request_id: &str,
) -> Result<T, String> {
    let token = validate_and_read_token(paths)?;
    validate_socket(paths)?;

    let request = GuardianRequest {
        version: PROTOCOL_VERSION,
        kind,
        request_id,
        token: &token,
        authorization_id,
        expected_revision: None,
        policy: None,
    };
    let mut bytes = serde_json::to_vec(&request)
        .map_err(|error| format!("could not encode Guardian request: {error}"))?;
    bytes.push(b'\n');

    timeout(REQUEST_TIMEOUT, exchange(&paths.socket, &bytes))
        .await
        .map_err(|_| "Guardian IPC request timed out".to_owned())?
        .and_then(|response| parse_response(&response, request_id))
}

async fn request_monitoring_policy_update(
    paths: &GuardianPaths,
    expected_revision: u64,
    policy: &GuardianMonitoringPolicy,
    request_id: &str,
) -> Result<GuardianMonitoringPolicySnapshot, String> {
    validate_monitoring_policy(policy)?;
    let token = validate_and_read_token(paths)?;
    validate_socket(paths)?;
    let request = GuardianRequest {
        version: PROTOCOL_VERSION,
        kind: GuardianRequestKind::UpdateMonitoringPolicy,
        request_id,
        token: &token,
        authorization_id: None,
        expected_revision: Some(expected_revision),
        policy: Some(policy),
    };
    let mut bytes = serde_json::to_vec(&request)
        .map_err(|error| format!("could not encode Guardian request: {error}"))?;
    bytes.push(b'\n');
    timeout(REQUEST_TIMEOUT, exchange(&paths.socket, &bytes))
        .await
        .map_err(|_| "Guardian IPC request timed out".to_owned())?
        .and_then(|response| parse_response(&response, request_id))
}

fn canonical_decimal_bytes(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 40
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
}

fn decimal_le(left: &str, right: &str) -> bool {
    left.len() < right.len() || (left.len() == right.len() && left <= right)
}

fn validate_monitoring_policy(policy: &GuardianMonitoringPolicy) -> Result<(), String> {
    if !canonical_decimal_bytes(&policy.warning_bytes)
        || !canonical_decimal_bytes(&policy.critical_bytes)
    {
        return Err("Guardian thresholds must be canonical decimal byte strings".to_owned());
    }
    if !decimal_le(&policy.critical_bytes, &policy.warning_bytes) {
        return Err(
            "Guardian critical byte threshold must not exceed warning threshold".to_owned(),
        );
    }
    if policy.warning_basis_points > 10_000 || policy.critical_basis_points > 10_000 {
        return Err("Guardian basis points must be between 0 and 10000".to_owned());
    }
    if policy.critical_basis_points > policy.warning_basis_points {
        return Err(
            "Guardian critical basis points must not exceed warning basis points".to_owned(),
        );
    }
    Ok(())
}

fn validate_monitoring_policy_snapshot(
    snapshot: &GuardianMonitoringPolicySnapshot,
) -> Result<(), String> {
    if snapshot.schema_version != PROTOCOL_VERSION {
        return Err(format!(
            "Guardian monitoring policy schema version mismatch: expected {PROTOCOL_VERSION}, got {}",
            snapshot.schema_version
        ));
    }
    validate_monitoring_policy(&snapshot.policy)
}

fn valid_fingerprint(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=128).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

fn validate_security_key(key: &GuardianSecurityKey) -> bool {
    valid_identifier(&key.key_id) && valid_fingerprint(&key.public_key_fingerprint)
}

fn validate_security_status(status: &GuardianSecurityStatus) -> Result<(), String> {
    if status.schema_version != PROTOCOL_VERSION {
        return Err("Guardian security status schema version mismatch".to_owned());
    }
    let execution_valid = match status.execution.status {
        GuardianSubsystemStatus::Available => status.execution.code.is_none(),
        GuardianSubsystemStatus::Degraded => {
            status.execution.code.as_deref() == Some("execution_store_lock_held")
        }
        GuardianSubsystemStatus::Unavailable => {
            status.execution.code.as_deref() == Some("execution_health_unavailable")
        }
        _ => false,
    };
    if !execution_valid {
        return Err("Guardian execution security status is invalid".to_owned());
    }

    let authority = &status.personal_contracts_authority;
    let authority_details = authority.generation.is_some()
        || authority.active_key.is_some()
        || authority.pending_key.is_some()
        || authority.retired_key_count.is_some()
        || authority.revoked_key_count.is_some();
    let authority_valid = match authority.status {
        GuardianSubsystemStatus::Uninitialized => !authority_details && authority.code.is_none(),
        GuardianSubsystemStatus::Unavailable => {
            !authority_details && authority.code.as_deref() == Some("authority_trust_unavailable")
        }
        GuardianSubsystemStatus::Active
        | GuardianSubsystemStatus::PendingRotation
        | GuardianSubsystemStatus::Revoked => {
            authority.code.is_none()
                && authority
                    .generation
                    .is_some_and(|value| (1..=MAX_JS_SAFE_INTEGER).contains(&value))
                && authority.retired_key_count.is_some()
                && authority.revoked_key_count.is_some()
                && authority
                    .active_key
                    .as_ref()
                    .is_none_or(validate_security_key)
                && authority
                    .pending_key
                    .as_ref()
                    .is_none_or(validate_security_key)
                && match authority.status {
                    GuardianSubsystemStatus::Active => {
                        authority.active_key.is_some() && authority.pending_key.is_none()
                    }
                    GuardianSubsystemStatus::PendingRotation => {
                        authority.active_key.is_some()
                            && authority.pending_key.is_some()
                            && authority.active_key != authority.pending_key
                    }
                    GuardianSubsystemStatus::Revoked => {
                        authority.active_key.is_none() && authority.pending_key.is_none()
                    }
                    _ => false,
                }
        }
        _ => false,
    };
    if !authority_valid {
        return Err("Guardian authority security status is invalid".to_owned());
    }
    if authority
        .retired_key_count
        .zip(authority.revoked_key_count)
        .and_then(|(retired, revoked)| retired.checked_add(revoked))
        .is_some_and(|count| count > MAX_SECURITY_KEYS_OR_PROFILES)
    {
        return Err("Guardian authority security status is invalid".to_owned());
    }

    let executor = &status.executor_identity;
    let executor_details = executor.generation.is_some()
        || executor.executor_id.is_some()
        || executor.active_key.is_some()
        || executor.pending_key.is_some();
    let executor_valid = match executor.status {
        GuardianSubsystemStatus::Uninitialized => !executor_details && executor.code.is_none(),
        GuardianSubsystemStatus::Unavailable => {
            !executor_details && executor.code.as_deref() == Some("executor_identity_unavailable")
        }
        GuardianSubsystemStatus::Active
        | GuardianSubsystemStatus::PendingRotation
        | GuardianSubsystemStatus::Revoked => {
            executor.code.is_none()
                && executor
                    .generation
                    .is_some_and(|value| (1..=MAX_JS_SAFE_INTEGER).contains(&value))
                && executor
                    .executor_id
                    .as_deref()
                    .is_some_and(valid_identifier)
                && executor
                    .active_key
                    .as_ref()
                    .is_none_or(validate_security_key)
                && executor
                    .pending_key
                    .as_ref()
                    .is_none_or(validate_security_key)
                && match executor.status {
                    GuardianSubsystemStatus::Active => {
                        executor.active_key.is_some() && executor.pending_key.is_none()
                    }
                    GuardianSubsystemStatus::PendingRotation => {
                        executor.active_key.is_some()
                            && executor.pending_key.is_some()
                            && executor.active_key != executor.pending_key
                    }
                    GuardianSubsystemStatus::Revoked => {
                        executor.active_key.is_none() && executor.pending_key.is_none()
                    }
                    _ => false,
                }
        }
        _ => false,
    };
    if !executor_valid {
        return Err("Guardian executor security status is invalid".to_owned());
    }

    let lifecycle = &status.profile_lifecycle;
    let lifecycle_details = lifecycle.head.is_some()
        || lifecycle.active_profile_count.is_some()
        || lifecycle.retired_profile_count.is_some();
    let lifecycle_valid = match lifecycle.status {
        GuardianSubsystemStatus::Uninitialized => !lifecycle_details && lifecycle.code.is_none(),
        GuardianSubsystemStatus::Unavailable => {
            !lifecycle_details && lifecycle.code.as_deref() == Some("profile_lifecycle_unavailable")
        }
        GuardianSubsystemStatus::Available => {
            lifecycle.code.is_none()
                && lifecycle.head.as_ref().is_some_and(|head| {
                    (1..=MAX_JS_SAFE_INTEGER).contains(&head.generation)
                        && valid_fingerprint(&head.digest)
                })
                && lifecycle.active_profile_count.is_some()
                && lifecycle.retired_profile_count.is_some()
        }
        _ => false,
    };
    if !lifecycle_valid {
        return Err("Guardian profile lifecycle security status is invalid".to_owned());
    }
    if lifecycle
        .active_profile_count
        .zip(lifecycle.retired_profile_count)
        .and_then(|(active, retired)| active.checked_add(retired))
        .is_some_and(|count| count > MAX_SECURITY_KEYS_OR_PROFILES)
    {
        return Err("Guardian profile lifecycle security status is invalid".to_owned());
    }
    Ok(())
}

fn validate_authorization_id(authorization_id: &str) -> Result<(), String> {
    let bytes = authorization_id.as_bytes();
    let valid = (1..=128).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'));
    if valid {
        Ok(())
    } else {
        Err("Guardian authorizationId is invalid".to_owned())
    }
}

fn validate_runs(runs: &GuardianRuns) -> Result<(), String> {
    if runs.schema_version != PROTOCOL_VERSION {
        return Err(format!(
            "Guardian runs schema version mismatch: expected {PROTOCOL_VERSION}, got {}",
            runs.schema_version
        ));
    }
    for run in &runs.runs {
        validate_authorization_id(&run.authorization_id)
            .map_err(|_| "Guardian runs response contains an invalid authorizationId".to_owned())?;
        if run.progress > 100 {
            return Err("Guardian runs response contains invalid progress".to_owned());
        }
        if run.status == GuardianRunStatus::Expired {
            let exact_expiry_result = run.result.as_ref().is_some_and(|result| {
                result.len() == 1
                    && result.get("code")
                        == Some(&serde_json::Value::String("authorization_expired".to_owned()))
            });
            if run.progress != 0
                || run.started_at.is_some()
                || run.finished_at.is_none()
                || !exact_expiry_result
            {
                return Err("Guardian expired run is invalid".to_owned());
            }
        }
    }
    if runs.events.len() > 100 {
        return Err("Guardian runs response contains too many events".to_owned());
    }
    Ok(())
}

fn validate_start_scan_result(
    result: &GuardianStartScanResult,
    authorization_id: &str,
) -> Result<(), String> {
    if result.authorization_id == authorization_id {
        Ok(())
    } else {
        Err("Guardian start response authorizationId does not match".to_owned())
    }
}

fn validate_cancel_run_result(
    result: &GuardianCancelRunResult,
    authorization_id: &str,
) -> Result<(), String> {
    if result.authorization_id == authorization_id {
        Ok(())
    } else {
        Err("Guardian cancel response authorizationId does not match".to_owned())
    }
}

async fn exchange(socket_path: &Path, request: &[u8]) -> Result<Vec<u8>, String> {
    let mut stream = UnixStream::connect(socket_path)
        .await
        .map_err(|error| format!("could not connect to Guardian IPC: {error}"))?;
    stream
        .write_all(request)
        .await
        .map_err(|error| format!("could not write Guardian IPC request: {error}"))?;

    let mut response = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|error| format!("could not read Guardian IPC response: {error}"))?;
        if read == 0 {
            break;
        }
        if response.len().saturating_add(read) > MAX_RESPONSE_BYTES {
            return Err("Guardian IPC response is too large".to_owned());
        }
        response.extend_from_slice(&chunk[..read]);
    }
    Ok(response)
}

fn parse_response<T: DeserializeOwned>(bytes: &[u8], request_id: &str) -> Result<T, String> {
    let newline = bytes
        .iter()
        .position(|byte| *byte == b'\n')
        .ok_or_else(|| "Guardian IPC response is missing its newline frame".to_owned())?;
    if newline == 0 {
        return Err("Guardian IPC response frame is empty".to_owned());
    }
    if newline + 1 != bytes.len() {
        return Err("Guardian IPC returned multiple response frames".to_owned());
    }

    let envelope: GuardianEnvelope<T> = serde_json::from_slice(&bytes[..newline])
        .map_err(|error| format!("Guardian IPC response is malformed: {error}"))?;
    if envelope.version != PROTOCOL_VERSION {
        return Err(format!(
            "Guardian IPC protocol version mismatch: expected {PROTOCOL_VERSION}, got {}",
            envelope.version
        ));
    }
    if envelope.request_id != request_id {
        return Err("Guardian IPC response requestId does not match".to_owned());
    }
    match (envelope.ok, envelope.data, envelope.error) {
        (true, Some(data), None) => Ok(data),
        (false, None, Some(error)) if !error.code.is_empty() && !error.message.is_empty() => Err(
            format!("Guardian IPC error {}: {}", error.code, error.message),
        ),
        _ => Err("Guardian IPC response envelope is invalid".to_owned()),
    }
}

fn validate_and_read_token(paths: &GuardianPaths) -> Result<String, String> {
    validate_directory(&paths.directory)?;
    reject_symlink(&paths.token, "Guardian IPC token")?;

    let mut options = OpenOptions::new();
    options.read(true).custom_flags(no_follow_flag());
    let mut file = options
        .open(&paths.token)
        .map_err(|error| format!("could not open Guardian IPC token: {error}"))?;
    validate_regular_owner_only(&file, "Guardian IPC token")?;

    let mut raw = String::new();
    file.read_to_string(&mut raw)
        .map_err(|error| format!("could not read Guardian IPC token: {error}"))?;
    let token = raw.trim();
    if token.len() != 64
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("Guardian IPC token contents are invalid".to_owned());
    }
    Ok(token.to_owned())
}

fn validate_directory(path: &Path) -> Result<(), String> {
    reject_symlink(path, "Guardian IPC directory")?;
    let metadata = fs::metadata(path)
        .map_err(|error| format!("could not inspect Guardian IPC directory: {error}"))?;
    if !metadata.is_dir() {
        return Err("Guardian IPC directory is not a directory".to_owned());
    }
    if metadata.permissions().mode() & 0o777 != 0o700 {
        return Err("Guardian IPC directory must have mode 0700".to_owned());
    }
    if metadata.uid() != effective_uid() {
        return Err("Guardian IPC directory is not owned by the current user".to_owned());
    }
    Ok(())
}

fn validate_socket(paths: &GuardianPaths) -> Result<(), String> {
    reject_symlink(&paths.socket, "Guardian IPC socket")?;
    let metadata = fs::metadata(&paths.socket)
        .map_err(|error| format!("could not inspect Guardian IPC socket: {error}"))?;
    if !metadata.file_type().is_socket() {
        return Err("Guardian IPC socket path is not a Unix socket".to_owned());
    }
    if metadata.permissions().mode() & 0o777 != 0o600 {
        return Err("Guardian IPC socket must have mode 0600".to_owned());
    }
    if metadata.uid() != effective_uid() {
        return Err("Guardian IPC socket is not owned by the current user".to_owned());
    }
    Ok(())
}

fn reject_symlink(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("could not inspect {label}: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("{label} must not be a symlink"));
    }
    Ok(())
}

fn validate_regular_owner_only(file: &File, label: &str) -> Result<(), String> {
    let metadata = file
        .metadata()
        .map_err(|error| format!("could not inspect {label}: {error}"))?;
    if !metadata.is_file() {
        return Err(format!("{label} is not a regular file"));
    }
    if metadata.permissions().mode() & 0o777 != 0o600 {
        return Err(format!("{label} must have mode 0600"));
    }
    if metadata.uid() != effective_uid() {
        return Err(format!("{label} is not owned by the current user"));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn no_follow_flag() -> i32 {
    0x0000_0100
}

#[cfg(not(target_os = "macos"))]
fn no_follow_flag() -> i32 {
    0x0002_0000
}

fn effective_uid() -> u32 {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid has no arguments, no preconditions, and no side effects.
    unsafe { geteuid() }
}

fn new_request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("cenno-{}-{nanos}-{sequence}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::{
        fs,
        os::unix::fs::{symlink, PermissionsExt},
    };
    use tempfile::TempDir;
    use tokio::net::UnixListener;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const REQUEST_ID: &str = "test-request";

    struct Fixture {
        _temp: TempDir,
        paths: GuardianPaths,
        listener: UnixListener,
    }

    impl Fixture {
        async fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let directory = temp.path().join("app.disk-guardian");
            fs::create_dir(&directory).unwrap();
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
            let paths = GuardianPaths::under(directory);
            fs::write(&paths.token, format!("{TOKEN}\n")).unwrap();
            fs::set_permissions(&paths.token, fs::Permissions::from_mode(0o600)).unwrap();
            let listener = UnixListener::bind(&paths.socket).unwrap();
            fs::set_permissions(&paths.socket, fs::Permissions::from_mode(0o600)).unwrap();
            Self {
                _temp: temp,
                paths,
                listener,
            }
        }

        fn serve(self, response: Vec<u8>) -> tokio::task::JoinHandle<Vec<u8>> {
            let keep_temp_directory_alive = self._temp;
            let listener = self.listener;
            tokio::spawn(async move {
                let _keep_temp_directory_alive = keep_temp_directory_alive;
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut byte = [0_u8; 1];
                while socket.read_exact(&mut byte).await.is_ok() {
                    request.push(byte[0]);
                    if byte[0] == b'\n' {
                        break;
                    }
                }
                socket.write_all(&response).await.unwrap();
                socket.shutdown().await.unwrap();
                request
            })
        }
    }

    fn sample() -> Value {
        json!({
            "schemaVersion": 1,
            "generatedAt": "2026-08-08T10:00:00.000Z",
            "sample": {
                "schemaVersion": 1,
                "targetId": "root",
                "observedAt": "2026-08-08T10:00:00.000Z",
                "totalBytes": "1000",
                "availableBytes": "123",
                "freeBytes": "120",
                "blockSizeBytes": "4096"
            },
            "evaluation": {
                "severity": "warning",
                "reasons": ["available_bytes_warning"],
                "evaluatedAt": "2026-08-08T10:00:00.000Z",
                "latestSampleAt": "2026-08-08T10:00:00.000Z",
                "availableBytes": "123",
                "availableBasisPoints": 1230
            }
        })
    }

    fn diagnostics() -> Value {
        json!({
            "schemaVersion": 1,
            "generatedAt": "2026-08-08T10:00:00.000Z",
            "report": {
                "schemaVersion": 1,
                "observedAt": "2026-08-08T10:00:00.000Z",
                "scopes": [{
                    "id": "user.downloads", "files": 2, "folders": 1,
                    "bytes": "50", "readable": true
                }],
                "totals": {"files": 2, "folders": 1, "bytes": "50"},
                "topCandidates": [{
                    "scope": "user.downloads", "kind": "file",
                    "relativePath": "large.bin", "bytes": "40"
                }],
                "partial": false,
                "limitsHit": [],
                "errorCount": 0
            },
            "growth": null,
            "preview": {
                "schemaVersion": 1,
                "generatedFrom": "2026-08-08T10:00:00.000Z",
                "previewRef": "sha256:test",
                "estimatedBytes": "40",
                "candidates": [{
                    "scope": "user.downloads", "kind": "file",
                    "relativePath": "large.bin", "bytes": "40"
                }]
            }
        })
    }

    fn runs() -> Value {
        json!({
            "schemaVersion": 1,
            "sequence": 7,
            "runs": [
                {
                    "authorizationId": "authorization-imported",
                    "profileId": "safe-diagnostics",
                    "profileVersion": 1,
                    "expiresAt": "2026-08-08T11:00:00.000Z",
                    "status": "imported",
                    "progress": 0
                },
                {
                    "authorizationId": "authorization-running",
                    "profileId": "safe-diagnostics",
                    "profileVersion": 1,
                    "expiresAt": "2026-08-08T11:00:00.000Z",
                    "status": "running",
                    "progress": 25,
                    "claimedAt": "2026-08-08T10:00:00.000Z",
                    "startedAt": "2026-08-08T10:00:01.000Z"
                },
                {
                    "authorizationId": "authorization-expired",
                    "profileId": "safe-diagnostics",
                    "profileVersion": 1,
                    "expiresAt": "2026-08-08T10:00:02.000Z",
                    "status": "expired",
                    "progress": 0,
                    "finishedAt": "2026-08-08T10:00:02.001Z",
                    "result": {"code": "authorization_expired"}
                }
            ],
            "events": [{
                "id": "run-event-000007",
                "type": "authorization.started",
                "authorizationId": "authorization-running",
                "occurredAt": "2026-08-08T10:00:01.000Z"
            }]
        })
    }

    fn security_status() -> Value {
        json!({
            "schemaVersion": 1,
            "reportedBy": "disk-guardian",
            "execution": {"status": "available"},
            "personalContractsAuthority": {
                "status": "active",
                "generation": 4,
                "activeKey": {
                    "keyId": "pc-key",
                    "publicKeyFingerprint": format!("sha256:{}", "a".repeat(64))
                },
                "pendingKey": null,
                "retiredKeyCount": 1,
                "revokedKeyCount": 0
            },
            "executorIdentity": {
                "status": "active",
                "generation": 2,
                "executorId": "guardian.local",
                "activeKey": {
                    "keyId": "guardian-key",
                    "publicKeyFingerprint": format!("sha256:{}", "b".repeat(64))
                },
                "pendingKey": null
            },
            "profileLifecycle": {
                "status": "available",
                "head": {
                    "generation": 7,
                    "digest": format!("sha256:{}", "c".repeat(64))
                },
                "activeProfileCount": 1,
                "retiredProfileCount": 1
            }
        })
    }

    fn response(data: Value) -> Vec<u8> {
        format!(
            "{}\n",
            json!({"version": 1, "requestId": REQUEST_ID, "ok": true, "data": data})
        )
        .into_bytes()
    }

    #[tokio::test]
    async fn sends_exact_status_request_and_parses_string_bytes() {
        let fixture = Fixture::new().await;
        let paths = fixture.paths.clone();
        let server = fixture.serve(response(sample()));
        let status: GuardianStatus = request(&paths, GuardianRequestKind::GetStatus, REQUEST_ID)
            .await
            .unwrap();
        assert_eq!(status.sample.available_bytes, "123");
        assert_eq!(status.evaluation.severity, DiskSeverity::Warning);
        assert_eq!(
            server.await.unwrap(),
            format!(
                "{{\"version\":1,\"type\":\"get_status\",\"requestId\":\"{REQUEST_ID}\",\"token\":\"{TOKEN}\"}}\n"
            )
            .into_bytes()
        );
    }

    #[tokio::test]
    async fn sends_exact_diagnostics_request_and_parses_typed_report() {
        let fixture = Fixture::new().await;
        let paths = fixture.paths.clone();
        let server = fixture.serve(response(diagnostics()));
        let result: GuardianDiagnostics =
            request(&paths, GuardianRequestKind::GetDiagnostics, REQUEST_ID)
                .await
                .unwrap();
        assert_eq!(result.report.totals.bytes, "50");
        assert_eq!(result.preview.candidates[0].bytes, "40");
        assert_eq!(
            server.await.unwrap(),
            format!(
                "{{\"version\":1,\"type\":\"get_diagnostics\",\"requestId\":\"{REQUEST_ID}\",\"token\":\"{TOKEN}\"}}\n"
            )
            .into_bytes()
        );
    }

    #[tokio::test]
    async fn sends_exact_get_runs_request_and_parses_imported_receipts() {
        let fixture = Fixture::new().await;
        let paths = fixture.paths.clone();
        let server = fixture.serve(response(runs()));
        let result: GuardianRuns = request(&paths, GuardianRequestKind::GetRuns, REQUEST_ID)
            .await
            .unwrap();
        validate_runs(&result).unwrap();
        assert_eq!(result.sequence, 7);
        assert_eq!(result.runs[0].status, GuardianRunStatus::Imported);
        assert_eq!(result.runs[0].claimed_at, None);
        assert_eq!(result.runs[2].status, GuardianRunStatus::Expired);
        assert_eq!(
            result.runs[2].result.as_ref().unwrap().get("code"),
            Some(&json!("authorization_expired"))
        );
        assert_eq!(
            server.await.unwrap(),
            format!(
                "{{\"version\":1,\"type\":\"get_runs\",\"requestId\":\"{REQUEST_ID}\",\"token\":\"{TOKEN}\"}}\n"
            )
            .into_bytes()
        );
    }

    #[test]
    fn rejects_extended_or_incomplete_expired_run_results() {
        let mut extended: GuardianRuns = serde_json::from_value(runs()).unwrap();
        extended.runs[2]
            .result
            .as_mut()
            .unwrap()
            .insert("message".to_owned(), json!("raw backend detail"));
        assert_eq!(
            validate_runs(&extended),
            Err("Guardian expired run is invalid".to_owned())
        );

        let mut incomplete: GuardianRuns = serde_json::from_value(runs()).unwrap();
        incomplete.runs[2].finished_at = None;
        assert_eq!(
            validate_runs(&incomplete),
            Err("Guardian expired run is invalid".to_owned())
        );
    }

    #[tokio::test]
    async fn sends_exact_security_status_request_and_parses_strict_redacted_dto() {
        let fixture = Fixture::new().await;
        let paths = fixture.paths.clone();
        let server = fixture.serve(response(security_status()));
        let result: GuardianSecurityStatus =
            request(&paths, GuardianRequestKind::GetSecurityStatus, REQUEST_ID)
                .await
                .unwrap();
        validate_security_status(&result).unwrap();
        assert_eq!(result.reported_by, GuardianSecurityReporter::DiskGuardian);
        assert_eq!(
            result.profile_lifecycle.head.as_ref().unwrap().generation,
            7
        );
        assert_eq!(
            server.await.unwrap(),
            format!(
                "{{\"version\":1,\"type\":\"get_security_status\",\"requestId\":\"{REQUEST_ID}\",\"token\":\"{TOKEN}\"}}\n"
            )
            .into_bytes()
        );
    }

    #[test]
    fn security_status_rejects_unknown_fields_and_impossible_states() {
        let mut unknown = security_status();
        unknown["executorIdentity"]["publicKeyPem"] = json!("must not be accepted");
        assert!(serde_json::from_value::<GuardianSecurityStatus>(unknown).is_err());

        let unknown_envelope = format!(
            "{}\n",
            json!({
                "version": 1,
                "requestId": REQUEST_ID,
                "ok": true,
                "data": security_status(),
                "error": null,
                "extra": "must not be accepted"
            })
        );
        assert!(
            parse_response::<GuardianSecurityStatus>(unknown_envelope.as_bytes(), REQUEST_ID)
                .is_err()
        );

        let mut impossible = security_status();
        impossible["personalContractsAuthority"]["status"] = json!("revoked");
        let parsed: GuardianSecurityStatus = serde_json::from_value(impossible).unwrap();
        assert!(validate_security_status(&parsed).is_err());

        let mut unsafe_generation = security_status();
        unsafe_generation["executorIdentity"]["generation"] = json!(MAX_JS_SAFE_INTEGER + 1);
        let parsed: GuardianSecurityStatus = serde_json::from_value(unsafe_generation).unwrap();
        assert!(validate_security_status(&parsed).is_err());

        let mut unbounded_counts = security_status();
        unbounded_counts["profileLifecycle"]["activeProfileCount"] =
            json!(MAX_SECURITY_KEYS_OR_PROFILES + 1);
        let parsed: GuardianSecurityStatus = serde_json::from_value(unbounded_counts).unwrap();
        assert!(validate_security_status(&parsed).is_err());

        let mut duplicate_pending = security_status();
        duplicate_pending["executorIdentity"]["status"] = json!("pending_rotation");
        duplicate_pending["executorIdentity"]["pendingKey"] =
            duplicate_pending["executorIdentity"]["activeKey"].clone();
        let parsed: GuardianSecurityStatus = serde_json::from_value(duplicate_pending).unwrap();
        assert!(validate_security_status(&parsed).is_err());
    }

    #[tokio::test]
    async fn sends_exact_start_and_cancel_requests() {
        const AUTHORIZATION_ID: &str = "authorization:scan-1";
        let fixture = Fixture::new().await;
        let paths = fixture.paths.clone();
        let started = json!({
            "authorizationId": AUTHORIZATION_ID,
            "status": "running",
            "startedAt": "2026-08-08T10:00:01.000Z"
        });
        let server = fixture.serve(response(started));
        let result: GuardianStartScanResult = request_with_authorization(
            &paths,
            GuardianRequestKind::StartScan,
            Some(AUTHORIZATION_ID),
            REQUEST_ID,
        )
        .await
        .unwrap();
        assert_eq!(result.status, GuardianStartedStatus::Running);
        validate_start_scan_result(&result, AUTHORIZATION_ID).unwrap();
        assert_eq!(
            server.await.unwrap(),
            format!(
                "{{\"version\":1,\"type\":\"start_scan\",\"requestId\":\"{REQUEST_ID}\",\"token\":\"{TOKEN}\",\"authorizationId\":\"{AUTHORIZATION_ID}\"}}\n"
            )
            .into_bytes()
        );

        let fixture = Fixture::new().await;
        let paths = fixture.paths.clone();
        let cancelled = json!({
            "authorizationId": AUTHORIZATION_ID,
            "status": "cancelled"
        });
        let server = fixture.serve(response(cancelled));
        let result: GuardianCancelRunResult = request_with_authorization(
            &paths,
            GuardianRequestKind::CancelRun,
            Some(AUTHORIZATION_ID),
            REQUEST_ID,
        )
        .await
        .unwrap();
        assert_eq!(result.status, GuardianCancelledStatus::Cancelled);
        validate_cancel_run_result(&result, AUTHORIZATION_ID).unwrap();
        assert_eq!(
            server.await.unwrap(),
            format!(
                "{{\"version\":1,\"type\":\"cancel_run\",\"requestId\":\"{REQUEST_ID}\",\"token\":\"{TOKEN}\",\"authorizationId\":\"{AUTHORIZATION_ID}\"}}\n"
            )
            .into_bytes()
        );
    }

    #[tokio::test]
    async fn rejects_invalid_authorization_ids_before_ipc() {
        for authorization_id in ["", "-leading", "contains/path", "space here"] {
            let error = guardian_start_scan(authorization_id.to_owned())
                .await
                .unwrap_err();
            assert_eq!(error, "Guardian authorizationId is invalid");
            let error = guardian_cancel_run(authorization_id.to_owned())
                .await
                .unwrap_err();
            assert_eq!(error, "Guardian authorizationId is invalid");
        }
        assert!(validate_authorization_id(&"a".repeat(128)).is_ok());
        assert!(validate_authorization_id(&"a".repeat(129)).is_err());
    }

    #[tokio::test]
    async fn rejects_malformed_run_lifecycle_responses() {
        let mut leaked_signature = runs();
        leaked_signature["runs"][0]["receiptSignature"] = json!("bearer-secret");
        let fixture = Fixture::new().await;
        let paths = fixture.paths.clone();
        let _server = fixture.serve(response(leaked_signature));
        let error = request::<GuardianRuns>(&paths, GuardianRequestKind::GetRuns, REQUEST_ID)
            .await
            .unwrap_err();
        assert!(error.contains("unknown field `receiptSignature`"));

        let fixture = Fixture::new().await;
        let paths = fixture.paths.clone();
        let malformed = json!({
            "authorizationId": "authorization-scan",
            "status": "running"
        });
        let _server = fixture.serve(response(malformed));
        let error = request_with_authorization::<GuardianStartScanResult>(
            &paths,
            GuardianRequestKind::StartScan,
            Some("authorization-scan"),
            REQUEST_ID,
        )
        .await
        .unwrap_err();
        assert!(error.contains("missing field `startedAt`"));

        let mismatched = GuardianStartScanResult {
            authorization_id: "different-authorization".to_owned(),
            status: GuardianStartedStatus::Running,
            started_at: "2026-08-08T10:00:01.000Z".to_owned(),
        };
        assert!(
            validate_start_scan_result(&mismatched, "authorization-scan")
                .unwrap_err()
                .contains("does not match")
        );
    }

    #[tokio::test]
    async fn rejects_insecure_directory_token_and_socket_modes() {
        let fixture = Fixture::new().await;
        fs::set_permissions(&fixture.paths.directory, fs::Permissions::from_mode(0o755)).unwrap();
        let error =
            request::<GuardianStatus>(&fixture.paths, GuardianRequestKind::GetStatus, REQUEST_ID)
                .await
                .unwrap_err();
        assert!(error.contains("0700"));

        let fixture = Fixture::new().await;
        fs::set_permissions(&fixture.paths.token, fs::Permissions::from_mode(0o644)).unwrap();
        let error =
            request::<GuardianStatus>(&fixture.paths, GuardianRequestKind::GetStatus, REQUEST_ID)
                .await
                .unwrap_err();
        assert!(error.contains("token must have mode 0600"));

        let fixture = Fixture::new().await;
        fs::set_permissions(&fixture.paths.socket, fs::Permissions::from_mode(0o666)).unwrap();
        let error =
            request::<GuardianStatus>(&fixture.paths, GuardianRequestKind::GetStatus, REQUEST_ID)
                .await
                .unwrap_err();
        assert!(error.contains("socket must have mode 0600"));
    }

    #[tokio::test]
    async fn rejects_invalid_token_and_non_socket_path() {
        let fixture = Fixture::new().await;
        fs::write(&fixture.paths.token, "ABC\n").unwrap();
        let error =
            request::<GuardianStatus>(&fixture.paths, GuardianRequestKind::GetStatus, REQUEST_ID)
                .await
                .unwrap_err();
        assert!(error.contains("contents are invalid"));

        let fixture = Fixture::new().await;
        drop(fixture.listener);
        fs::remove_file(&fixture.paths.socket).unwrap();
        fs::write(&fixture.paths.socket, "not a socket").unwrap();
        fs::set_permissions(&fixture.paths.socket, fs::Permissions::from_mode(0o600)).unwrap();
        let error =
            request::<GuardianStatus>(&fixture.paths, GuardianRequestKind::GetStatus, REQUEST_ID)
                .await
                .unwrap_err();
        assert!(error.contains("not a Unix socket"));
    }

    #[tokio::test]
    async fn rejects_directory_token_and_socket_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let real = temp.path().join("real");
        fs::create_dir(&real).unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o700)).unwrap();
        let linked = temp.path().join("linked");
        symlink(&real, &linked).unwrap();
        let paths = GuardianPaths::under(linked);
        let error = validate_and_read_token(&paths).unwrap_err();
        assert!(error.contains("directory must not be a symlink"));

        let fixture = Fixture::new().await;
        let real_token = fixture.paths.directory.join("real-token");
        fs::rename(&fixture.paths.token, &real_token).unwrap();
        symlink(&real_token, &fixture.paths.token).unwrap();
        let error = validate_and_read_token(&fixture.paths).unwrap_err();
        assert!(error.contains("token must not be a symlink"));

        let fixture = Fixture::new().await;
        let real_socket = fixture.paths.directory.join("real.sock");
        fs::rename(&fixture.paths.socket, &real_socket).unwrap();
        symlink(&real_socket, &fixture.paths.socket).unwrap();
        let error = validate_socket(&fixture.paths).unwrap_err();
        assert!(error.contains("socket must not be a symlink"));
    }

    #[tokio::test]
    async fn rejects_missing_newline_and_multiple_frames() {
        let fixture = Fixture::new().await;
        let paths = fixture.paths.clone();
        let _server = fixture.serve(
            response(sample())
                .into_iter()
                .take_while(|b| *b != b'\n')
                .collect(),
        );
        let error = request::<GuardianStatus>(&paths, GuardianRequestKind::GetStatus, REQUEST_ID)
            .await
            .unwrap_err();
        assert!(error.contains("missing its newline"));

        let fixture = Fixture::new().await;
        let paths = fixture.paths.clone();
        let mut two = response(sample());
        two.extend_from_slice(b"{}\n");
        let _server = fixture.serve(two);
        let error = request::<GuardianStatus>(&paths, GuardianRequestKind::GetStatus, REQUEST_ID)
            .await
            .unwrap_err();
        assert!(error.contains("multiple response frames"));
    }

    #[tokio::test]
    async fn rejects_oversized_response() {
        let fixture = Fixture::new().await;
        let paths = fixture.paths.clone();
        let _server = fixture.serve(vec![b'x'; MAX_RESPONSE_BYTES + 1]);
        let error = request::<GuardianStatus>(&paths, GuardianRequestKind::GetStatus, REQUEST_ID)
            .await
            .unwrap_err();
        assert!(error.contains("too large"));
    }

    #[tokio::test]
    async fn times_out_when_guardian_does_not_respond() {
        let fixture = Fixture::new().await;
        let paths = fixture.paths.clone();
        let keep_temp_directory_alive = fixture._temp;
        let listener = fixture.listener;
        let server = tokio::spawn(async move {
            let _keep_temp_directory_alive = keep_temp_directory_alive;
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            socket.read_to_end(&mut request).await.unwrap();
            tokio::time::sleep(Duration::from_secs(3)).await;
        });
        let error = request::<GuardianStatus>(&paths, GuardianRequestKind::GetStatus, REQUEST_ID)
            .await
            .unwrap_err();
        assert!(error.contains("timed out"));
        server.abort();
    }

    #[tokio::test]
    async fn rejects_wrong_version_request_id_and_invalid_envelopes() {
        for (body, expected) in [
            (
                json!({"version": 2, "requestId": REQUEST_ID, "ok": true, "data": sample()}),
                "version mismatch",
            ),
            (
                json!({"version": 1, "requestId": "other", "ok": true, "data": sample()}),
                "requestId does not match",
            ),
            (
                json!({"version": 1, "requestId": REQUEST_ID, "ok": true, "data": sample(), "error": {"code": "bad", "message": "bad"}}),
                "envelope is invalid",
            ),
            (
                json!({"version": 1, "requestId": REQUEST_ID, "ok": false}),
                "envelope is invalid",
            ),
        ] {
            let fixture = Fixture::new().await;
            let paths = fixture.paths.clone();
            let _server = fixture.serve(format!("{body}\n").into_bytes());
            let error =
                request::<GuardianStatus>(&paths, GuardianRequestKind::GetStatus, REQUEST_ID)
                    .await
                    .unwrap_err();
            assert!(error.contains(expected), "unexpected error: {error}");
        }
    }

    #[tokio::test]
    async fn surfaces_typed_server_error_envelope() {
        let fixture = Fixture::new().await;
        let paths = fixture.paths.clone();
        let body = json!({
            "version": 1,
            "requestId": REQUEST_ID,
            "ok": false,
            "error": {"code": "authentication_failed", "message": "authentication failed"}
        });
        let _server = fixture.serve(format!("{body}\n").into_bytes());
        let error = request::<GuardianStatus>(&paths, GuardianRequestKind::GetStatus, REQUEST_ID)
            .await
            .unwrap_err();
        assert_eq!(
            error,
            "Guardian IPC error authentication_failed: authentication failed"
        );
    }

    fn monitoring_policy() -> Value {
        json!({
            "schemaVersion": 1,
            "revision": 7,
            "policy": {
                "warningBytes": "53687091200",
                "criticalBytes": "21474836480",
                "warningBasisPoints": 1000,
                "criticalBasisPoints": 400
            }
        })
    }

    #[tokio::test]
    async fn sends_exact_get_monitoring_policy_request_and_parses_strict_snapshot() {
        let fixture = Fixture::new().await;
        let paths = fixture.paths.clone();
        let server = fixture.serve(response(monitoring_policy()));
        let result: GuardianMonitoringPolicySnapshot =
            request(&paths, GuardianRequestKind::GetMonitoringPolicy, REQUEST_ID)
                .await
                .unwrap();

        assert_eq!(result.schema_version, 1);
        assert_eq!(result.revision, 7);
        assert_eq!(result.policy.warning_bytes, "53687091200");
        assert_eq!(result.policy.critical_basis_points, 400);
        assert_eq!(
            server.await.unwrap(),
            format!(
                "{{\"version\":1,\"type\":\"get_monitoring_policy\",\"requestId\":\"{REQUEST_ID}\",\"token\":\"{TOKEN}\"}}\n"
            )
            .into_bytes()
        );
    }

    #[tokio::test]
    async fn sends_exact_update_monitoring_policy_cas_request_and_parses_revision() {
        let fixture = Fixture::new().await;
        let paths = fixture.paths.clone();
        let mut updated = monitoring_policy();
        updated["revision"] = json!(8);
        updated["policy"]["warningBytes"] = json!("60000000000");
        let server = fixture.serve(response(updated));
        let policy = GuardianMonitoringPolicy {
            warning_bytes: "60000000000".to_owned(),
            critical_bytes: "21474836480".to_owned(),
            warning_basis_points: 1000,
            critical_basis_points: 400,
        };

        let result = request_monitoring_policy_update(&paths, 7, &policy, REQUEST_ID)
            .await
            .unwrap();
        assert_eq!(result.revision, 8);
        assert_eq!(result.policy, policy);
        assert_eq!(
            server.await.unwrap(),
            format!(
                "{{\"version\":1,\"type\":\"update_monitoring_policy\",\"requestId\":\"{REQUEST_ID}\",\"token\":\"{TOKEN}\",\"expectedRevision\":7,\"policy\":{{\"warningBytes\":\"60000000000\",\"criticalBytes\":\"21474836480\",\"warningBasisPoints\":1000,\"criticalBasisPoints\":400}}}}\n"
            )
            .into_bytes()
        );
    }

    #[tokio::test]
    async fn monitoring_policy_response_rejects_unknown_missing_and_mistyped_fields() {
        for malformed in [
            json!({
                "schemaVersion": 1,
                "revision": 7,
                "policy": {
                    "warningBytes": "53687091200",
                    "criticalBytes": "21474836480",
                    "warningBasisPoints": 1000,
                    "criticalBasisPoints": 400,
                    "path": "/"
                }
            }),
            json!({
                "schemaVersion": 1,
                "revision": 7,
                "policy": {
                    "warningBytes": "53687091200",
                    "criticalBytes": "21474836480",
                    "warningBasisPoints": "1000",
                    "criticalBasisPoints": 400
                }
            }),
            json!({
                "schemaVersion": 1,
                "revision": 7,
                "policy": {
                    "warningBytes": "53687091200",
                    "criticalBytes": "21474836480",
                    "warningBasisPoints": 1000
                }
            }),
        ] {
            let fixture = Fixture::new().await;
            let paths = fixture.paths.clone();
            let _server = fixture.serve(response(malformed));
            let error = request::<GuardianMonitoringPolicySnapshot>(
                &paths,
                GuardianRequestKind::GetMonitoringPolicy,
                REQUEST_ID,
            )
            .await
            .unwrap_err();
            assert!(
                error.contains("unknown field")
                    || error.contains("missing field")
                    || error.contains("invalid type"),
                "unexpected error: {error}"
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires the user Disk Guardian LaunchAgent"]
    async fn live_guardian_probe_uses_the_production_typed_boundary() {
        let status = guardian_get_status().await.unwrap();
        assert_eq!(status.schema_version, PROTOCOL_VERSION);
        assert!(!status.sample.total_bytes.is_empty());

        let diagnostics = guardian_get_diagnostics().await.unwrap();
        assert_eq!(diagnostics.schema_version, PROTOCOL_VERSION);
        assert!(diagnostics.report.top_candidates.len() <= 20);
    }
}
