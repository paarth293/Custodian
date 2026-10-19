//! Typed, bounded requests for application adapters.
//!
//! This is the native boundary between planning and application control. It
//! validates shape and identity only; a platform adapter must still remeasure
//! the target immediately before observing or applying an effect.

use crate::app_authority::{ApplicationAuthority, ApplicationControl, ApplicationGrant};
use crate::app_privacy::{ApplicationIdentity, ApplicationPrivacyPolicy};
use crate::task_program::{Failure, FailureClass};
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;

pub const MAX_DOCUMENT_NAME: usize = 512;
pub const MAX_EDIT_TEXT: usize = 64 * 1024;
pub const MAX_OBSERVATION_TEXT: usize = 64 * 1024;
pub const MAX_TIMEOUT_MS: u64 = 120_000;

fn invalid_text(text: &str) -> bool {
    text.chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
}

pub fn capability_kind(capability: &str) -> Option<crate::task_program::Kind> {
    use crate::task_program::Kind;
    match capability {
        "app.open_bound_document"
        | "app.edit_bound_range"
        | "app.edit_bound_cells"
        | "app.save_bound_document"
        | "app.close_document_normally" => Some(Kind::App),
        "app.inspect_bound_document" => Some(Kind::Read),
        "app.verify_saved_output" => Some(Kind::Verify),
        _ => None,
    }
}

pub const TEXT_EDITOR_CAPABILITIES: &[&str] = &[
    "app.open_bound_document",
    "app.inspect_bound_document",
    "app.edit_bound_range",
    "app.save_bound_document",
    "app.verify_saved_output",
];

pub fn validate_text_editor_program(capabilities: &[String]) -> Result<(), &'static str> {
    if capabilities.len() != TEXT_EDITOR_CAPABILITIES.len()
        || capabilities
            .iter()
            .map(String::as_str)
            .ne(TEXT_EDITOR_CAPABILITIES.iter().copied())
    {
        return Err("text-editor program must open, inspect, edit, save, and verify exactly once");
    }
    Ok(())
}

pub fn validate_capability(capability: &str) -> Result<(), &'static str> {
    if capability_kind(capability).is_none() {
        return Err("application capability is not registered");
    }
    Ok(())
}

pub fn validate_remeasurement(
    bound: &ApplicationIdentity,
    measured: &ApplicationIdentity,
    current_revision: u64,
) -> Result<(), &'static str> {
    if bound.privacy_revision != current_revision
        || measured.privacy_revision != current_revision
        || !bound.same_target(measured)
    {
        return Err("application target changed or privacy authority is stale");
    }
    Ok(())
}

pub fn compile_text_editor_program(
    goal: String,
    constraints: Vec<String>,
    scoped_grants: Vec<String>,
) -> crate::task_program::Program {
    use crate::task_program::{Kind, Predicate, Reference, Step, Value};
    use std::collections::BTreeMap;

    let step = |title: &str, kind: Kind, capability: &str, dependencies: Vec<usize>| Step {
        title: title.into(),
        kind,
        capability: capability.into(),
        dependencies,
        inputs: BTreeMap::new(),
        when: None,
        retries: 0,
    };
    crate::task_program::Program {
        goal,
        constraints,
        scoped_grants,
        steps: vec![
            step(
                "Open the bound document",
                Kind::App,
                "app.open_bound_document",
                vec![],
            ),
            step(
                "Inspect the bound document",
                Kind::Read,
                "app.inspect_bound_document",
                vec![0],
            ),
            step(
                "Edit the bound range",
                Kind::App,
                "app.edit_bound_range",
                vec![1],
            ),
            step(
                "Save the bound document",
                Kind::App,
                "app.save_bound_document",
                vec![2],
            ),
            step(
                "Verify the saved output",
                Kind::Verify,
                "app.verify_saved_output",
                vec![3],
            ),
        ],
        completion: vec![Predicate {
            fact: Reference {
                step: 4,
                key: "verified".into(),
            },
            equals: Value::Bool(true),
        }],
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextDocumentSnapshot {
    pub identity: ApplicationIdentity,
    pub document_name: String,
    pub byte_len: u64,
    pub digest: [u8; 32],
    pub privacy_revision: u64,
}

impl TextDocumentSnapshot {
    pub fn from_bytes(
        identity: ApplicationIdentity,
        document_name: String,
        bytes: &[u8],
        privacy_revision: u64,
    ) -> Result<Self, &'static str> {
        if document_name.trim().is_empty()
            || document_name.len() > MAX_DOCUMENT_NAME
            || document_name.chars().any(char::is_control)
        {
            return Err("invalid text document identity");
        }
        identity.validate()?;
        if identity.privacy_revision != privacy_revision {
            return Err("text snapshot privacy revision is stale");
        }
        Ok(Self {
            identity,
            document_name,
            byte_len: bytes.len() as u64,
            digest: *blake3::hash(bytes).as_bytes(),
            privacy_revision,
        })
    }

    /// Verifies a fresh reopen without exposing the document bytes to the
    /// planner or model. A changed length or digest is stale-world evidence.
    pub fn verify_bytes(&self, bytes: &[u8]) -> Result<(), &'static str> {
        if bytes.len() as u64 != self.byte_len || *blake3::hash(bytes).as_bytes() != self.digest {
            return Err("reopened text document does not match its bound snapshot");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ApplicationAction {
    OpenBoundDocument,
    InspectBoundDocument,
    EditBoundRange {
        start: u64,
        end: u64,
        replacement: String,
    },
    EditBoundCells {
        sheet_id: String,
        cells: Vec<CellEdit>,
    },
    SaveBoundDocument,
    CloseDocumentNormally,
    VerifySavedOutput,
}

/// Decimal numbers remain strings on the authority wire, preserving an exact
/// grant digest and rejecting NaN/Infinity before they reach Office.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CellInput {
    Empty,
    Text {
        value: String,
    },
    Number {
        decimal: String,
    },
    Sum {
        first_row: u16,
        first_col: u16,
        last_row: u16,
        last_col: u16,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellEdit {
    pub row: u16,
    pub col: u16,
    pub value: CellInput,
}
impl CellInput {
    pub fn native(&self) -> Result<custodian_winsec::excel::CellValue, &'static str> {
        use custodian_winsec::excel::CellValue;
        Ok(match self {
            Self::Empty => CellValue::Empty,
            Self::Text { value } if value.len() <= 4096 && !invalid_text(value) => {
                CellValue::Text {
                    value: value.clone(),
                }
            }
            Self::Number { decimal } => {
                if decimal.is_empty()
                    || decimal.len() > 32
                    || !decimal.bytes().all(|c| {
                        c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.' | b'e' | b'E')
                    })
                {
                    return Err("invalid cell number");
                }
                let value = decimal.parse::<f64>().map_err(|_| "invalid cell number")?;
                if !value.is_finite() || value.abs() > 1e12 {
                    return Err("invalid cell number");
                }
                CellValue::Number { value }
            }
            Self::Sum {
                first_row,
                first_col,
                last_row,
                last_col,
            } if (1..=16).contains(first_row)
                && (1..=16).contains(first_col)
                && (1..=16).contains(last_row)
                && (1..=16).contains(last_col)
                && first_row <= last_row
                && first_col <= last_col =>
            {
                CellValue::Sum {
                    first_row: *first_row,
                    first_col: *first_col,
                    last_row: *last_row,
                    last_col: *last_col,
                }
            }
            _ => return Err("unsupported cell value"),
        })
    }
}
pub fn validate_cell_edits(sheet_id: &str, cells: &[CellEdit]) -> Result<(), &'static str> {
    if sheet_id.is_empty()
        || sheet_id.len() > 256
        || sheet_id.chars().any(char::is_control)
        || cells.is_empty()
        || cells.len() > 32
    {
        return Err("invalid bound worksheet or cell count");
    }
    let mut coordinates = std::collections::BTreeSet::new();
    let mut size = 0;
    for cell in cells {
        if !(1..=16).contains(&cell.row)
            || !(1..=16).contains(&cell.col)
            || !coordinates.insert((cell.row, cell.col))
        {
            return Err("invalid or duplicate cell coordinate");
        }
        cell.value.native()?;
        size += serde_json::to_vec(cell).map_err(|_| "invalid cell")?.len();
    }
    if size > MAX_EDIT_TEXT {
        return Err("cell edit payload exceeds limit");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationRequest {
    pub identity: ApplicationIdentity,
    /// Opaque native document identity, distinct from a possibly duplicate title.
    pub document_id: String,
    pub document_name: String,
    /// Native measured document/version token; not a display title or authority.
    pub document_version: String,
    pub action: ApplicationAction,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationReceipt {
    pub action: ApplicationAction,
    pub identity: ApplicationIdentity,
    pub document_id: String,
    pub document_name: String,
    pub verified: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationObservation {
    pub identity: ApplicationIdentity,
    pub document_id: String,
    pub document_name: String,
    pub text: String,
    pub truncated: bool,
    pub privacy_revision: u64,
}

impl ApplicationObservation {
    pub fn validate(
        &self,
        policy: &ApplicationPrivacyPolicy,
        current_revision: u64,
    ) -> Result<(), &'static str> {
        self.identity.validate()?;
        if self.privacy_revision != current_revision
            || self.identity.privacy_revision != current_revision
            || !policy.allows_document(&self.identity, &self.document_id, current_revision)
        {
            return Err("application observation is private, stale, or invalid");
        }
        if self.document_name.trim().is_empty()
            || self.document_name.len() > MAX_DOCUMENT_NAME
            || self.document_name.chars().any(char::is_control)
        {
            return Err("invalid observed document identity");
        }
        if self.text.len() > MAX_OBSERVATION_TEXT || invalid_text(&self.text) {
            return Err("application observation exceeds its bounded text contract");
        }
        if self.truncated && self.text.is_empty() {
            return Err("truncated observation must contain a bounded excerpt");
        }
        Ok(())
    }
}

/// Platform adapters implement this trait; they never receive an unvalidated
/// request from the planner. Implementations must remeasure the target before
/// touching the application and return a receipt describing the observed
/// effect.
pub trait ApplicationExecutor {
    /// Minimal target/document metadata only, after initial admission; no window
    /// title/content scraping. An unqualified adapter defaults to unavailable.
    fn remeasure(
        &mut self,
        _request: &ApplicationRequest,
        _control: &ApplicationControl<'_>,
    ) -> Result<ApplicationRequest, Failure> {
        Err(Failure::new(
            FailureClass::UnavailableCapability,
            "native target remeasurement unavailable",
        ))
    }

    fn execute(
        &mut self,
        request: &ApplicationRequest,
        control: &ApplicationControl<'_>,
    ) -> Result<ApplicationReceipt, Failure>;
}

/// Native admission/cooperative execution gate, not worker containment.
/// A worker must still enforce hard timeouts and journal effects before input.
pub fn execute_checked(
    request: &ApplicationRequest,
    authority: &ApplicationAuthority,
    grant: &ApplicationGrant,
    cancel: &AtomicBool,
    executor: &mut dyn ApplicationExecutor,
) -> Result<ApplicationReceipt, ApplicationExecutionFailure> {
    let control = ApplicationControl::new(authority, grant, request, cancel)?;
    control.check()?;
    let measured = executor.remeasure(request, &control)?;
    control.check()?;
    if measured != *request {
        return Err(Failure::new(
            FailureClass::StaleWorld,
            "native application/document target changed",
        )
        .into());
    }
    let receipt = executor.execute(request, &control)?;
    if receipt.identity != request.identity
        || receipt.document_id != request.document_id
        || receipt.action != request.action
        || receipt.document_name != request.document_name
    {
        return Err(ApplicationExecutionFailure::with_receipt(
            Failure::new(
                FailureClass::Unrecoverable,
                "application adapter returned a receipt for a different target or action",
            ),
            receipt,
        ));
    }
    if receipt.detail.trim().is_empty()
        || receipt.detail.len() > 4096
        || invalid_text(&receipt.detail)
        || !receipt.verified
    {
        return Err(ApplicationExecutionFailure::with_receipt(
            Failure::new(
                FailureClass::Unrecoverable,
                "application adapter omitted valid verified effect evidence",
            ),
            receipt,
        ));
    }
    if let Err(error) = control.check() {
        return Err(ApplicationExecutionFailure::with_receipt(error, receipt));
    }
    Ok(receipt)
}

/// Preserve returned evidence on cancellation/revocation/late completion, so a
/// future host can reconcile an effect instead of blindly retrying it. Debug
/// deliberately omits document text and receipt content. This is not a journal.
pub struct ApplicationExecutionFailure {
    pub class: FailureClass,
    pub detail: String,
    pub recovery_receipt: Option<Box<ApplicationReceipt>>,
}

impl From<Failure> for ApplicationExecutionFailure {
    fn from(error: Failure) -> Self {
        Self {
            class: error.class,
            detail: error.detail,
            recovery_receipt: None,
        }
    }
}

impl ApplicationExecutionFailure {
    fn with_receipt(error: Failure, receipt: ApplicationReceipt) -> Self {
        Self {
            recovery_receipt: Some(Box::new(receipt)),
            ..error.into()
        }
    }
}

impl std::fmt::Debug for ApplicationExecutionFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApplicationExecutionFailure")
            .field("class", &self.class)
            .field("has_recovery_receipt", &self.recovery_receipt.is_some())
            .finish()
    }
}

impl ApplicationRequest {
    pub fn validate(
        &self,
        policy: &ApplicationPrivacyPolicy,
        current_revision: u64,
    ) -> Result<(), &'static str> {
        self.identity.validate()?;
        if self.document_id.trim().is_empty()
            || self.document_id.len() > 256
            || self.document_id.chars().any(char::is_control)
        {
            return Err("invalid native document binding");
        }
        if !policy.allows_document(&self.identity, &self.document_id, current_revision) {
            return Err("application target is private, stale, or invalid");
        }
        if self.document_name.trim().is_empty()
            || self.document_name.len() > MAX_DOCUMENT_NAME
            || self.document_name.chars().any(char::is_control)
        {
            return Err("invalid document identity");
        }
        if self.document_version.trim().is_empty()
            || self.document_version.len() > 256
            || self.document_version.chars().any(char::is_control)
        {
            return Err("invalid document version binding");
        }
        if !(1..=MAX_TIMEOUT_MS).contains(&self.timeout_ms) {
            return Err("application timeout is outside the bounded range");
        }
        if let ApplicationAction::EditBoundRange {
            start,
            end,
            replacement,
        } = &self.action
        {
            if start > end
                || *end > MAX_OBSERVATION_TEXT as u64
                || replacement.len() > MAX_EDIT_TEXT
                || invalid_text(replacement)
            {
                return Err("replacement text is outside the bounded range");
            }
        }
        if let ApplicationAction::EditBoundCells { sheet_id, cells } = &self.action {
            validate_cell_edits(sheet_id, cells)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_refuse_nonfinite_numbers_duplicates_outside_ranges_and_untyped_formulas() {
        for decimal in ["NaN", "inf", "1e999", "1000000000001", "=1+1", " 7"] {
            assert!(CellInput::Number {
                decimal: decimal.into()
            }
            .native()
            .is_err());
        }
        let cell = CellEdit {
            row: 1,
            col: 1,
            value: CellInput::Text {
                value: "=WEBSERVICE(\"x\")".into(),
            },
        };
        assert!(matches!(
            cell.value.native().unwrap(),
            custodian_winsec::excel::CellValue::Text { .. }
        ));
        assert!(validate_cell_edits("sheet", &[cell.clone(), cell.clone()]).is_err());
        assert!(validate_cell_edits(
            "sheet",
            &[CellEdit {
                row: 17,
                ..cell.clone()
            }]
        )
        .is_err());
        assert!(validate_cell_edits("", &[cell]).is_err());
        assert!(CellInput::Sum {
            first_row: 0,
            first_col: 1,
            last_row: 16,
            last_col: 1
        }
        .native()
        .is_err());
        assert!(serde_json::from_value::<CellInput>(
            serde_json::json!({"kind":"formula","value":"=1+1"})
        )
        .is_err());
    }

    #[test]
    fn observed_document_exclusion_cannot_be_bypassed_by_an_equal_display_name() {
        let req = request(ApplicationAction::InspectBoundDocument);
        let mut policy = ApplicationPrivacyPolicy::default();
        policy
            .set_private_documents(std::slice::from_ref(&req.document_id))
            .unwrap();
        let mut observation = ApplicationObservation {
            identity: req.identity,
            document_id: req.document_id,
            document_name: "same display name".into(),
            text: "bounded synthetic text".into(),
            truncated: false,
            privacy_revision: 1,
        };
        observation.privacy_revision = observation.identity.privacy_revision;
        assert!(observation
            .validate(&policy, observation.privacy_revision)
            .is_err());
        observation.document_id = "public-document-same-display-name".into();
        assert!(observation
            .validate(&policy, observation.privacy_revision)
            .is_ok());
    }
    use crate::app_authority::AppPermission;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    #[test]
    fn multiline_unicode_is_valid_but_forged_authority_and_bad_ranges_are_not() {
        let mut req = request(ApplicationAction::EditBoundRange {
            start: 0,
            end: 12,
            replacement: "Heading\r\n\tहैलो\nNext line".into(),
        });
        assert!(req
            .validate(&ApplicationPrivacyPolicy::default(), 5)
            .is_ok());
        let mut wire = serde_json::to_value(&req).unwrap();
        wire["approved"] = serde_json::json!(true);
        assert!(serde_json::from_value::<ApplicationRequest>(wire).is_err());
        req.action = ApplicationAction::EditBoundRange {
            start: 20,
            end: 10,
            replacement: "safe".into(),
        };
        assert!(req
            .validate(&ApplicationPrivacyPolicy::default(), 5)
            .is_err());
        req.action = ApplicationAction::EditBoundRange {
            start: 0,
            end: 65_537,
            replacement: "safe".into(),
        };
        assert!(req
            .validate(&ApplicationPrivacyPolicy::default(), 5)
            .is_err());
    }

    #[test]
    fn app_capability_cannot_disguise_its_effect_as_a_read_or_verifier() {
        use crate::task_program::Kind;
        let mut p = compile_text_editor_program("fixture".into(), vec![], vec![]);
        p.steps[2].kind = Kind::Read;
        assert!(p.validate().is_err());
        p.steps[2].kind = Kind::Verify;
        assert!(p.validate().is_err());
        p.steps[2].kind = Kind::App;
        p.steps[1].capability = "app.run_macro".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn unicode_truncated_excerpt_does_not_need_to_end_on_an_exact_byte_limit() {
        let observation = ApplicationObservation {
            document_id: "native-doc-1".into(),
            identity: request(ApplicationAction::InspectBoundDocument).identity,
            document_name: "fixture".into(),
            text: "हैलो\nexcerpt".into(),
            truncated: true,
            privacy_revision: 5,
        };
        assert!(observation
            .validate(&ApplicationPrivacyPolicy::default(), 5)
            .is_ok());
        assert!(
            TextDocumentSnapshot::from_bytes(observation.identity, "fixture".into(), b"x", 6)
                .is_err()
        );
    }

    fn request(action: ApplicationAction) -> ApplicationRequest {
        ApplicationRequest {
            identity: ApplicationIdentity {
                executable: r"C:\Tools\editor.exe".into(),
                pid: 12,
                process_generation: "p12-1".into(),
                window: 34,
                window_generation: "w34-1".into(),
                privacy_revision: 5,
            },
            document_id: "native-doc-1".into(),
            document_name: "fixture.txt".into(),
            document_version: "fixture-version-1".into(),
            action,
            timeout_ms: 10_000,
        }
    }

    #[test]
    fn accepts_bounded_edit_request() {
        let request = request(ApplicationAction::EditBoundRange {
            start: 0,
            end: 0,
            replacement: "safe fixture text".into(),
        });
        assert!(request
            .validate(&ApplicationPrivacyPolicy::default(), 5)
            .is_ok());
    }

    #[test]
    fn refuses_control_characters_in_edit_text() {
        let request = request(ApplicationAction::EditBoundRange {
            start: 0,
            end: 0,
            replacement: "bad\u{0000}text".into(),
        });
        assert_eq!(
            request.validate(&ApplicationPrivacyPolicy::default(), 5),
            Err("replacement text is outside the bounded range")
        );
    }

    #[test]
    fn refuses_unbounded_timeout() {
        let mut request = request(ApplicationAction::InspectBoundDocument);
        request.timeout_ms = MAX_TIMEOUT_MS + 1;
        assert_eq!(
            request.validate(&ApplicationPrivacyPolicy::default(), 5),
            Err("application timeout is outside the bounded range")
        );
    }

    #[test]
    fn refuses_private_target_before_adapter_execution() {
        let mut policy = ApplicationPrivacyPolicy::default();
        policy.set_private_executables([r"c:\tools\editor.exe"]);
        assert_eq!(
            request(ApplicationAction::OpenBoundDocument).validate(&policy, 5),
            Err("application target is private, stale, or invalid")
        );
    }

    struct MockExecutor {
        receipt: ApplicationReceipt,
    }

    impl ApplicationExecutor for MockExecutor {
        fn remeasure(
            &mut self,
            request: &ApplicationRequest,
            control: &ApplicationControl<'_>,
        ) -> Result<ApplicationRequest, Failure> {
            control.check()?;
            Ok(request.clone())
        }
        fn execute(
            &mut self,
            _request: &ApplicationRequest,
            _control: &ApplicationControl<'_>,
        ) -> Result<ApplicationReceipt, Failure> {
            Ok(self.receipt.clone())
        }
    }

    #[test]
    fn execution_gate_accepts_matching_evidence() {
        let request = request(ApplicationAction::InspectBoundDocument);
        let receipt = ApplicationReceipt {
            action: request.action.clone(),
            identity: request.identity.clone(),
            document_id: request.document_id.clone(),
            document_name: request.document_name.clone(),
            verified: true,
            detail: "fixture inspected".into(),
        };
        let mut executor = MockExecutor { receipt };
        let authority = ApplicationAuthority::new(ApplicationPrivacyPolicy::default(), 5);
        let grant = authority
            .issue_after_native_consent(
                &request,
                &[AppPermission::Open, AppPermission::Inspect],
                Duration::from_secs(30),
            )
            .unwrap();
        let result = execute_checked(
            &request,
            &authority,
            &grant,
            &AtomicBool::new(false),
            &mut executor,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn execution_gate_rejects_mismatched_receipt() {
        let request = request(ApplicationAction::InspectBoundDocument);
        let mut other = request.clone();
        other.document_name = "different.txt".into();
        let mut executor = MockExecutor {
            receipt: ApplicationReceipt {
                action: other.action,
                identity: other.identity,
                document_id: other.document_id,
                document_name: other.document_name,
                verified: true,
                detail: "wrong target".into(),
            },
        };
        let authority = ApplicationAuthority::new(ApplicationPrivacyPolicy::default(), 5);
        let grant = authority
            .issue_after_native_consent(
                &request,
                &[AppPermission::Open, AppPermission::Inspect],
                Duration::from_secs(30),
            )
            .unwrap();
        let result = execute_checked(
            &request,
            &authority,
            &grant,
            &AtomicBool::new(false),
            &mut executor,
        );
        assert_eq!(result.unwrap_err().class, FailureClass::Unrecoverable);
    }

    #[test]
    fn execution_gate_does_not_start_after_stop() {
        let request = request(ApplicationAction::OpenBoundDocument);
        let receipt = ApplicationReceipt {
            action: request.action.clone(),
            identity: request.identity.clone(),
            document_id: request.document_id.clone(),
            document_name: request.document_name.clone(),
            verified: true,
            detail: "must not run".into(),
        };
        let mut executor = MockExecutor { receipt };
        let stopped = AtomicBool::new(true);
        let authority = ApplicationAuthority::new(ApplicationPrivacyPolicy::default(), 5);
        let grant = authority
            .issue_after_native_consent(
                &request,
                &[AppPermission::Open, AppPermission::Inspect],
                Duration::from_secs(30),
            )
            .unwrap();
        let result = execute_checked(&request, &authority, &grant, &stopped, &mut executor);
        assert_eq!(result.unwrap_err().class, FailureClass::Unrecoverable);
    }

    #[test]
    fn observation_requires_current_policy_and_bounded_text() {
        let identity = request(ApplicationAction::InspectBoundDocument).identity;
        let observation = ApplicationObservation {
            document_id: "native-doc-1".into(),
            identity,
            document_name: "fixture.txt".into(),
            text: "observed fixture".into(),
            truncated: false,
            privacy_revision: 5,
        };
        assert!(observation
            .validate(&ApplicationPrivacyPolicy::default(), 5)
            .is_ok());
        assert_eq!(
            observation.validate(&ApplicationPrivacyPolicy::default(), 6),
            Err("application observation is private, stale, or invalid")
        );
    }

    #[test]
    fn invalid_truncation_metadata_is_refused() {
        let identity = request(ApplicationAction::InspectBoundDocument).identity;
        let observation = ApplicationObservation {
            document_id: "native-doc-1".into(),
            identity,
            document_name: "fixture.txt".into(),
            text: "".into(),
            truncated: true,
            privacy_revision: 5,
        };
        assert_eq!(
            observation.validate(&ApplicationPrivacyPolicy::default(), 5),
            Err("truncated observation must contain a bounded excerpt")
        );
    }

    #[test]
    fn text_snapshot_verifies_reopened_bytes_without_returning_content() {
        let identity = request(ApplicationAction::InspectBoundDocument).identity;
        let snapshot =
            TextDocumentSnapshot::from_bytes(identity, "fixture.txt".into(), b"known fixture", 5)
                .unwrap();
        assert!(snapshot.verify_bytes(b"known fixture").is_ok());
        assert_eq!(
            snapshot.verify_bytes(b"changed fixture"),
            Err("reopened text document does not match its bound snapshot")
        );
    }

    #[test]
    fn text_editor_program_has_fixed_dependency_order() {
        let capabilities = TEXT_EDITOR_CAPABILITIES
            .iter()
            .map(|capability| (*capability).to_string())
            .collect::<Vec<_>>();
        assert!(validate_text_editor_program(&capabilities).is_ok());

        let mut reordered = capabilities.clone();
        reordered.swap(1, 2);
        assert_eq!(
            validate_text_editor_program(&reordered),
            Err("text-editor program must open, inspect, edit, save, and verify exactly once")
        );
    }

    #[test]
    fn unregistered_application_capability_is_refused() {
        assert!(validate_capability("app.edit_bound_range").is_ok());
        assert_eq!(
            validate_capability("app.run_macro"),
            Err("application capability is not registered")
        );
    }

    #[test]
    fn compiled_text_editor_program_is_coordinator_valid() {
        let program = compile_text_editor_program("edit fixture".into(), vec![], vec![]);
        assert!(program.validate().is_ok());
        assert_eq!(
            program
                .steps
                .iter()
                .map(|step| step.capability.as_str())
                .collect::<Vec<_>>(),
            TEXT_EDITOR_CAPABILITIES
        );
    }

    #[test]
    fn compiled_text_editor_program_runs_through_task_coordinator() {
        use crate::coordinator::{TaskCoordinator, TaskState};
        use crate::task_program::{Adapter, Facts, Kind, Outcome, Step, Value};
        use std::collections::BTreeMap;
        use std::sync::atomic::AtomicBool;

        struct FixtureAdapter;
        impl Adapter for FixtureAdapter {
            fn authorize(&mut self, step: &Step) -> Result<(), Failure> {
                validate_capability(&step.capability)
                    .map_err(|detail| Failure::new(FailureClass::MissingAuthority, detail))
            }

            fn execute(
                &mut self,
                step: &Step,
                _inputs: &Facts,
                _cancel: &AtomicBool,
            ) -> Result<Outcome, Failure> {
                Ok(Outcome {
                    facts: if step.kind == Kind::Verify {
                        [("verified".into(), Value::Bool(true))].into()
                    } else {
                        BTreeMap::new()
                    },
                    receipts: if matches!(step.kind, Kind::App) {
                        vec![format!("fixture.receipt.{}", step.capability)]
                    } else {
                        vec![]
                    },
                })
            }
        }

        let coordinator = TaskCoordinator::new();
        let (id, cancel) = coordinator
            .create_task("Edit a fixture document".into(), TaskState::Running, &[])
            .unwrap();
        let result = coordinator.run_program(
            &id,
            compile_text_editor_program("Edit a fixture document".into(), vec![], vec![]),
            &cancel,
            &mut FixtureAdapter,
        );
        assert!(result.is_ok());
        assert_eq!(
            coordinator.get_task(&id).unwrap().state,
            TaskState::Completed
        );
    }

    #[test]
    fn remeasurement_rejects_reused_processes_and_changed_windows() {
        let bound = request(ApplicationAction::InspectBoundDocument).identity;
        let mut reused_pid = bound.clone();
        reused_pid.process_generation = "p12-2".into();
        assert_eq!(
            validate_remeasurement(&bound, &reused_pid, 5),
            Err("application target changed or privacy authority is stale")
        );

        let mut changed_window = bound.clone();
        changed_window.window_generation = "w34-2".into();
        assert_eq!(
            validate_remeasurement(&bound, &changed_window, 5),
            Err("application target changed or privacy authority is stale")
        );

        assert!(validate_remeasurement(&bound, &bound, 5).is_ok());
    }

    #[test]
    fn interrupted_text_editor_effect_requires_recovery_and_is_not_replayed() {
        use crate::coordinator::{TaskCoordinator, TaskState};
        use crate::task_program::{Adapter, Facts, Kind, Outcome, Step, Value};
        use std::collections::BTreeMap;
        use std::sync::atomic::AtomicBool;

        struct InterruptedAdapter;
        impl Adapter for InterruptedAdapter {
            fn authorize(&mut self, step: &Step) -> Result<(), Failure> {
                validate_capability(&step.capability)
                    .map_err(|detail| Failure::new(FailureClass::MissingAuthority, detail))
            }

            fn execute(
                &mut self,
                step: &Step,
                _inputs: &Facts,
                _cancel: &AtomicBool,
            ) -> Result<Outcome, Failure> {
                if step.capability == "app.save_bound_document" {
                    return Err(Failure::new(
                        FailureClass::Unrecoverable,
                        "fixture process stopped after save began",
                    ));
                }
                Ok(Outcome {
                    facts: if step.kind == Kind::Verify {
                        [("verified".into(), Value::Bool(true))].into()
                    } else {
                        BTreeMap::new()
                    },
                    receipts: if step.kind == Kind::App {
                        vec![format!("fixture.receipt.{}", step.capability)]
                    } else {
                        vec![]
                    },
                })
            }
        }

        let coordinator = TaskCoordinator::new();
        let (id, cancel) = coordinator
            .create_task("Interrupted fixture edit".into(), TaskState::Running, &[])
            .unwrap();
        let result = coordinator.run_program(
            &id,
            compile_text_editor_program("Interrupted fixture edit".into(), vec![], vec![]),
            &cancel,
            &mut InterruptedAdapter,
        );
        assert!(result.is_err());
        assert_eq!(
            coordinator.get_task(&id).unwrap().state,
            TaskState::RecoveryRequired
        );
    }

    #[test]
    fn stopped_text_editor_program_never_starts_an_application_effect() {
        use crate::coordinator::{TaskCoordinator, TaskState};
        use crate::task_program::{Adapter, Facts, Outcome, Step};
        use std::sync::atomic::AtomicBool;

        struct MustNotRun;
        impl Adapter for MustNotRun {
            fn authorize(&mut self, _step: &Step) -> Result<(), Failure> {
                panic!("application authorization must not run after stop");
            }

            fn execute(
                &mut self,
                _step: &Step,
                _inputs: &Facts,
                _cancel: &AtomicBool,
            ) -> Result<Outcome, Failure> {
                panic!("application execution must not run after stop");
            }
        }

        let coordinator = TaskCoordinator::new();
        let (id, cancel) = coordinator
            .create_task("Stopped fixture edit".into(), TaskState::Running, &[])
            .unwrap();
        cancel.store(true, std::sync::atomic::Ordering::Release);
        let result = coordinator.run_program(
            &id,
            compile_text_editor_program("Stopped fixture edit".into(), vec![], vec![]),
            &cancel,
            &mut MustNotRun,
        );
        assert!(result.is_err());
        assert_eq!(
            coordinator.get_task(&id).unwrap().state,
            TaskState::Cancelled
        );
    }
}
