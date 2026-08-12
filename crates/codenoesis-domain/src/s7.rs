use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

pub const PROVIDER_CAPABILITY: &str = "rust-direct-json-map/v1";
pub const CLIENT_CAPABILITY: &str = "kotlin-direct-json-access/v1";
pub const CONTRACT_CAPABILITY: &str = "openapi-3.1-http-json/v1";
pub const REPORT_SCHEMA: &str = "codenoesis.semantic-compatibility-report/v1";
pub const ANALYSIS_PROFILE: &str = "implementation-aware-http-json/v1";
pub const REPORT_PIPELINE: &str = "codenoesis.pipeline/semantic-impact/v1";
pub const ONTOLOGY_VERSION: &str = "codenoesis.ontology/api-compatibility/v1";
pub const EVIDENCE_LINEAGE_VERSION: &str = "codenoesis.source-evidence/v2";
pub const RULE_CATALOG_VERSION: &str = "codenoesis.compatibility-rules/http-json/v1";
pub const CONFIGURATION_HASH: &str =
    "blake3:5fdb5f89e0c04758bc424a131605fcae02502fb09e6321f41c4cec04d9ddc4db";

const DIFF_ID_DOMAIN: &str = "codenoesis.diff-id/v1";
const EVIDENCE_ID_DOMAIN: &str = "codenoesis.evidence-id/v1";
const GAP_ID_DOMAIN: &str = "codenoesis.coverage-gap-id/v1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum S7Limit {
    WorkspaceBytes,
    FederationReportBytes,
    SourceBytesPerFile,
    TotalSourceBytes,
    LogicalPathBytes,
    CallableSymbolBytes,
    Operations,
    FieldsPerOperation,
    LinkedClients,
    CallSites,
    SemanticDiffs,
    EvidenceItems,
    CoverageGaps,
    ReportBytes,
    SourceFiles,
    SyntaxNodesPerSource,
    SourceNestingDepth,
    StringLiteralBytes,
}

impl S7Limit {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceBytes => "workspace_bytes",
            Self::FederationReportBytes => "federation_report_bytes",
            Self::SourceBytesPerFile => "source_bytes_per_file",
            Self::TotalSourceBytes => "total_source_bytes",
            Self::LogicalPathBytes => "logical_path_bytes",
            Self::CallableSymbolBytes => "callable_symbol_bytes",
            Self::Operations => "operations",
            Self::FieldsPerOperation => "fields_per_operation",
            Self::LinkedClients => "linked_clients",
            Self::CallSites => "call_sites",
            Self::SemanticDiffs => "semantic_diffs",
            Self::EvidenceItems => "evidence_items",
            Self::CoverageGaps => "coverage_gaps",
            Self::ReportBytes => "report_bytes",
            Self::SourceFiles => "source_files",
            Self::SyntaxNodesPerSource => "syntax_nodes_per_source",
            Self::SourceNestingDepth => "source_nesting_depth",
            Self::StringLiteralBytes => "string_literal_bytes",
        }
    }

    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::WorkspaceBytes => 1_048_576,
            Self::FederationReportBytes | Self::ReportBytes => 67_108_864,
            Self::SourceBytesPerFile => 2_097_152,
            Self::TotalSourceBytes => 268_435_456,
            Self::LogicalPathBytes => 4_096,
            Self::CallableSymbolBytes => 1_024,
            Self::Operations | Self::LinkedClients => 10_000,
            Self::FieldsPerOperation => 5_000,
            Self::CallSites | Self::EvidenceItems | Self::SyntaxNodesPerSource => 1_000_000,
            Self::SemanticDiffs | Self::CoverageGaps => 200_000,
            Self::SourceFiles => 10_002,
            Self::SourceNestingDepth => 256,
            Self::StringLiteralBytes => 16_384,
        }
    }

    /// Checks one exact observed cardinality or byte count.
    ///
    /// # Errors
    ///
    /// Returns the inclusive maximum and first rejected observation.
    pub const fn check(self, observed: u64) -> Result<(), S7LimitExceeded> {
        let maximum = self.maximum();
        if observed > maximum {
            Err(S7LimitExceeded {
                limit: self,
                maximum,
                observed,
            })
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct S7LimitExceeded {
    pub limit: S7Limit,
    pub maximum: u64,
    pub observed: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceExtractionError {
    InvalidUtf8,
    InvalidSyntax,
    CallableMissing,
    CallableAmbiguous,
    UnsupportedSemantics,
    LimitExceeded(S7LimitExceeded),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: u64,
    pub end_line: u64,
}

impl SourceSpan {
    #[must_use]
    pub const fn is_valid_for(self, source_length: usize) -> bool {
        self.start_byte < self.end_byte
            && self.end_byte <= source_length
            && self.start_line > 0
            && self.start_line <= self.end_line
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderPresence {
    GuaranteedPresent,
    MayBeAbsent,
    Unknown,
}

impl ProviderPresence {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GuaranteedPresent => "guaranteed_present",
            Self::MayBeAbsent => "may_be_absent",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderFieldExtraction {
    pub field_name: String,
    pub presence: ProviderPresence,
    pub span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSourceExtraction {
    pub fields: Vec<ProviderFieldExtraction>,
    pub custom_mapping_spans: Vec<SourceSpan>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ClientPresenceAssumption {
    RequiresPresent,
    HandlesAbsent,
    Unknown,
}

impl ClientPresenceAssumption {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RequiresPresent => "requires_present",
            Self::HandlesAbsent => "handles_absent",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientFieldExtraction {
    pub field_name: String,
    pub assumption: ClientPresenceAssumption,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientSourceExtraction {
    pub path_template: String,
    pub assumptions: Vec<ClientFieldExtraction>,
    pub evidence_span: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractFieldProjection {
    pub field_id: String,
    pub json_pointer: String,
    pub required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractProjection {
    pub service_id: String,
    pub operation_id: String,
    pub method: String,
    pub path_template: String,
    pub explicit_operation_id: String,
    pub response_status: String,
    pub fields: Vec<ContractFieldProjection>,
    pub evidence_span: SourceSpan,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EvidenceSourceKind {
    DeclaredContract,
    ProviderImplementation,
    ClientAssumption,
}

impl EvidenceSourceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeclaredContract => "declared_contract",
            Self::ProviderImplementation => "provider_implementation",
            Self::ClientAssumption => "client_assumption",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEvidenceLocator {
    pub repository_identity: String,
    pub revision: String,
    pub path: String,
    pub start_line: u64,
    pub end_line: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEvidence {
    pub id: String,
    pub repository_identity: String,
    pub revision: String,
    pub path: String,
    pub start_line: u64,
    pub end_line: u64,
    pub excerpt_sha256: String,
    pub source_kind: EvidenceSourceKind,
    pub capability_version: &'static str,
}

impl SourceEvidence {
    #[must_use]
    pub fn new(
        locator: SourceEvidenceLocator,
        excerpt_sha256: String,
        source_kind: EvidenceSourceKind,
        capability_version: &'static str,
    ) -> Self {
        let id = evidence_id(
            &locator.repository_identity,
            &locator.revision,
            &locator.path,
            locator.start_line,
            locator.end_line,
            &excerpt_sha256,
        );
        Self {
            id,
            repository_identity: locator.repository_identity,
            revision: locator.revision,
            path: locator.path,
            start_line: locator.start_line,
            end_line: locator.end_line,
            excerpt_sha256,
            source_kind,
            capability_version,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderFieldFact {
    pub field_pointer: String,
    pub presence: ProviderPresence,
    pub evidence_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRevisionFacts {
    pub revision: String,
    pub contract_sha256: String,
    pub contract: ContractProjection,
    pub contract_evidence_id: String,
    pub fields: Vec<ProviderFieldFact>,
    pub custom_mapping_evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FederationState {
    Confirmed { operation_id: String },
    Rejected { operation_candidate_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientFact {
    pub repository_identity: String,
    pub revision: String,
    pub client_id: String,
    pub call_site_id: String,
    pub call_symbol: String,
    pub path_template: String,
    pub assumptions: Vec<(String, ClientPresenceAssumption)>,
    pub evidence_id: String,
    pub federation: FederationState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactAnalysisInput {
    pub provider_repository_identity: String,
    pub baseline: ProviderRevisionFacts,
    pub target: ProviderRevisionFacts,
    pub clients: Vec<ClientFact>,
    pub evidence: Vec<SourceEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImpactAnalysisError {
    InvalidAuthority,
    UnsupportedSemantics,
    LimitExceeded(S7LimitExceeded),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderReport {
    pub service_identity: String,
    pub repository_identity: String,
    pub baseline_revision: String,
    pub baseline_contract_sha256: String,
    pub target_revision: String,
    pub target_contract_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewDelta {
    pub before: &'static str,
    pub after: &'static str,
    pub delta: &'static str,
    pub claim_state: &'static str,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticDiff {
    pub id: String,
    pub operation_id: String,
    pub field_id: String,
    pub field_pointer: String,
    pub contract: ViewDelta,
    pub implementation: ViewDelta,
    pub change_kind: &'static str,
    pub classification: &'static str,
    pub rule_id: &'static str,
    pub affected_client_ids: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub coverage_gap_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientAssessment {
    pub client_identity: String,
    pub repository_identity: String,
    pub operation_id: String,
    pub call_site_id: String,
    pub call_symbol: String,
    pub presence_assumption: ClientPresenceAssumption,
    pub baseline_risk: &'static str,
    pub target_impact: &'static str,
    pub affected: bool,
    pub rule_ids: Vec<&'static str>,
    pub evidence_ids: Vec<String>,
    pub coverage_gap_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectedCandidate {
    pub client_identity: String,
    pub repository_identity: String,
    pub call_site_id: String,
    pub call_symbol: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImpactCoverageGap {
    pub id: String,
    pub subject_id: String,
    pub revisions: Vec<String>,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticCompatibilityReport {
    pub provider: ProviderReport,
    pub semantic_diffs: Vec<SemanticDiff>,
    pub client_assessments: Vec<ClientAssessment>,
    pub rejected_candidates: Vec<RejectedCandidate>,
    pub evidence: Vec<SourceEvidence>,
    pub coverage_gaps: Vec<ImpactCoverageGap>,
}

pub struct ImpactClassifier;

impl ImpactClassifier {
    /// Reconciles the three independently evidenced S7 views.
    ///
    /// # Errors
    ///
    /// Returns a closed authority, capability, or resource-limit failure.
    pub fn analyze(
        input: ImpactAnalysisInput,
    ) -> Result<SemanticCompatibilityReport, ImpactAnalysisError> {
        validate_input(&input)?;
        let (mut diffs, gaps) = classify_provider_changes(&input)?;

        let mut changed = diffs
            .iter()
            .filter(|diff| diff.classification == "breaking");
        let changed = changed
            .next()
            .filter(|_| changed.next().is_none())
            .ok_or(ImpactAnalysisError::UnsupportedSemantics)?;
        let (assessments, rejected) = classify_clients(&input, changed)?;

        let affected_client_ids = assessments
            .iter()
            .filter(|assessment| assessment.affected)
            .map(|assessment| assessment.client_identity.clone())
            .collect::<Vec<_>>();
        let client_evidence = assessments
            .iter()
            .filter(|assessment| assessment.affected)
            .flat_map(|assessment| assessment.evidence_ids.iter())
            .filter(|id| client_evidence_id(&input.clients, id))
            .cloned()
            .collect::<Vec<_>>();
        let known_diff = diffs
            .iter_mut()
            .find(|diff| diff.classification == "breaking")
            .ok_or(ImpactAnalysisError::UnsupportedSemantics)?;
        known_diff.affected_client_ids = ordered(affected_client_ids);
        known_diff.evidence_ids.extend(client_evidence);
        known_diff.evidence_ids = ordered(std::mem::take(&mut known_diff.evidence_ids));

        let referenced = referenced_evidence(&diffs, &assessments, &rejected, &gaps);
        let evidence_by_id = input
            .evidence
            .into_iter()
            .map(|evidence| (evidence.id.clone(), evidence))
            .collect::<BTreeMap<_, _>>();
        if referenced.iter().any(|id| !evidence_by_id.contains_key(id)) {
            return Err(ImpactAnalysisError::InvalidAuthority);
        }
        let evidence = referenced
            .into_iter()
            .map(|id| evidence_by_id[&id].clone())
            .collect::<Vec<_>>();
        check_limit(S7Limit::EvidenceItems, evidence.len())?;

        Ok(SemanticCompatibilityReport {
            provider: ProviderReport {
                service_identity: input.baseline.contract.service_id,
                repository_identity: input.provider_repository_identity,
                baseline_revision: input.baseline.revision,
                baseline_contract_sha256: input.baseline.contract_sha256,
                target_revision: input.target.revision,
                target_contract_sha256: input.target.contract_sha256,
            },
            semantic_diffs: diffs,
            client_assessments: assessments,
            rejected_candidates: rejected,
            evidence,
            coverage_gaps: gaps,
        })
    }
}

fn classify_provider_changes(
    input: &ImpactAnalysisInput,
) -> Result<(Vec<SemanticDiff>, Vec<ImpactCoverageGap>), ImpactAnalysisError> {
    let baseline_fields = contract_fields(&input.baseline.contract);
    let target_fields = contract_fields(&input.target.contract);
    if baseline_fields.len() != input.baseline.contract.fields.len()
        || target_fields.len() != input.target.contract.fields.len()
        || baseline_fields.keys().ne(target_fields.keys())
    {
        return Err(ImpactAnalysisError::UnsupportedSemantics);
    }
    let baseline_implementation = implementation_fields(&input.baseline);
    let target_implementation = implementation_fields(&input.target);
    if baseline_implementation.len() != input.baseline.fields.len()
        || target_implementation.len() != input.target.fields.len()
        || baseline_implementation
            .keys()
            .chain(target_implementation.keys())
            .any(|pointer| !baseline_fields.contains_key(pointer))
    {
        return Err(ImpactAnalysisError::UnsupportedSemantics);
    }
    let mut gaps = Vec::new();
    let mut diffs = Vec::new();
    for (pointer, baseline_contract) in &baseline_fields {
        let target_contract = target_fields
            .get(pointer)
            .ok_or(ImpactAnalysisError::InvalidAuthority)?;
        if baseline_contract.field_id != target_contract.field_id
            || baseline_contract.required != target_contract.required
        {
            return Err(ImpactAnalysisError::UnsupportedSemantics);
        }
        match (
            baseline_implementation.get(pointer),
            target_implementation.get(pointer),
        ) {
            (Some(before), Some(after)) if before.presence == after.presence => {}
            (Some(before), Some(after))
                if before.presence == ProviderPresence::GuaranteedPresent
                    && after.presence == ProviderPresence::MayBeAbsent
                    && !baseline_contract.required =>
            {
                diffs.push(known_presence_diff(input, baseline_contract, before, after));
            }
            (None, None)
                if !baseline_contract.required
                    && !input.baseline.custom_mapping_evidence_ids.is_empty()
                    && !input.target.custom_mapping_evidence_ids.is_empty() =>
            {
                let gap = custom_mapping_gap(input, baseline_contract);
                diffs.push(unresolved_presence_diff(input, baseline_contract, &gap));
                gaps.push(gap);
            }
            _ => return Err(ImpactAnalysisError::UnsupportedSemantics),
        }
    }
    check_limit(S7Limit::SemanticDiffs, diffs.len())?;
    check_limit(S7Limit::CoverageGaps, gaps.len())?;
    diffs.sort_by(|left, right| left.id.cmp(&right.id));
    gaps.sort_by(|left, right| left.id.cmp(&right.id));
    Ok((diffs, gaps))
}

fn classify_clients(
    input: &ImpactAnalysisInput,
    changed: &SemanticDiff,
) -> Result<(Vec<ClientAssessment>, Vec<RejectedCandidate>), ImpactAnalysisError> {
    let mut assessments = Vec::new();
    let mut rejected = Vec::new();
    for client in &input.clients {
        match &client.federation {
            FederationState::Confirmed { operation_id }
                if operation_id == &changed.operation_id
                    && client.path_template == input.baseline.contract.path_template =>
            {
                assessments.push(client_assessment(input, client, changed)?);
            }
            FederationState::Rejected { .. }
                if client.path_template != input.baseline.contract.path_template =>
            {
                rejected.push(RejectedCandidate {
                    client_identity: client.client_id.clone(),
                    repository_identity: client.repository_identity.clone(),
                    call_site_id: client.call_site_id.clone(),
                    call_symbol: client.call_symbol.clone(),
                    evidence_ids: vec![client.evidence_id.clone()],
                });
            }
            _ => return Err(ImpactAnalysisError::InvalidAuthority),
        }
    }
    check_limit(S7Limit::LinkedClients, assessments.len())?;
    check_limit(S7Limit::CallSites, input.clients.len())?;
    assessments.sort_by(|left, right| left.client_identity.cmp(&right.client_identity));
    rejected.sort_by(|left, right| left.client_identity.cmp(&right.client_identity));
    Ok((assessments, rejected))
}

fn validate_input(input: &ImpactAnalysisInput) -> Result<(), ImpactAnalysisError> {
    check_limit(S7Limit::Operations, 1)?;
    check_limit(
        S7Limit::FieldsPerOperation,
        input.baseline.contract.fields.len(),
    )?;
    check_limit(S7Limit::SourceFiles, input.clients.len().saturating_add(4))?;
    if input.baseline.revision == input.target.revision
        || input.baseline.contract_sha256 != input.target.contract_sha256
        || input.baseline.contract.service_id != input.target.contract.service_id
        || input.baseline.contract.operation_id != input.target.contract.operation_id
        || input.baseline.contract.path_template != input.target.contract.path_template
        || input.baseline.contract.method != input.target.contract.method
        || input.baseline.contract.response_status != input.target.contract.response_status
        || input.baseline.contract.method != "GET"
        || input.baseline.contract.response_status != "200"
    {
        return Err(ImpactAnalysisError::InvalidAuthority);
    }
    let evidence_ids = input
        .evidence
        .iter()
        .map(|evidence| evidence.id.as_str())
        .collect::<BTreeSet<_>>();
    if evidence_ids.len() != input.evidence.len() {
        return Err(ImpactAnalysisError::InvalidAuthority);
    }
    Ok(())
}

fn known_presence_diff(
    input: &ImpactAnalysisInput,
    field: &ContractFieldProjection,
    before: &ProviderFieldFact,
    after: &ProviderFieldFact,
) -> SemanticDiff {
    let contract_evidence = ordered([
        input.baseline.contract_evidence_id.clone(),
        input.target.contract_evidence_id.clone(),
    ]);
    let implementation_evidence = ordered([before.evidence_id.clone(), after.evidence_id.clone()]);
    SemanticDiff {
        id: diff_id(
            &input.provider_repository_identity,
            &input.baseline.revision,
            &input.target.revision,
            &field.field_id,
            "presence",
        ),
        operation_id: input.baseline.contract.operation_id.clone(),
        field_id: field.field_id.clone(),
        field_pointer: field.json_pointer.clone(),
        contract: ViewDelta {
            before: "optional",
            after: "optional",
            delta: "unchanged",
            claim_state: "deterministic_fact",
            evidence_ids: contract_evidence.clone(),
        },
        implementation: ViewDelta {
            before: before.presence.as_str(),
            after: after.presence.as_str(),
            delta: "weakened",
            claim_state: "derived_fact",
            evidence_ids: implementation_evidence.clone(),
        },
        change_kind: "implementation_behavior_changed_without_contract_change",
        classification: "breaking",
        rule_id: "cmp.response.presence.undocumented-guarantee-removed/v1",
        affected_client_ids: Vec::new(),
        evidence_ids: ordered(contract_evidence.into_iter().chain(implementation_evidence)),
        coverage_gap_ids: Vec::new(),
    }
}

fn unresolved_presence_diff(
    input: &ImpactAnalysisInput,
    field: &ContractFieldProjection,
    gap: &ImpactCoverageGap,
) -> SemanticDiff {
    let contract_evidence = ordered([
        input.baseline.contract_evidence_id.clone(),
        input.target.contract_evidence_id.clone(),
    ]);
    let implementation_evidence = gap.evidence_ids.clone();
    SemanticDiff {
        id: diff_id(
            &input.provider_repository_identity,
            &input.baseline.revision,
            &input.target.revision,
            &field.field_id,
            "presence",
        ),
        operation_id: input.baseline.contract.operation_id.clone(),
        field_id: field.field_id.clone(),
        field_pointer: field.json_pointer.clone(),
        contract: ViewDelta {
            before: "optional",
            after: "optional",
            delta: "unchanged",
            claim_state: "deterministic_fact",
            evidence_ids: contract_evidence.clone(),
        },
        implementation: ViewDelta {
            before: ProviderPresence::Unknown.as_str(),
            after: ProviderPresence::Unknown.as_str(),
            delta: "unresolved",
            claim_state: "derived_fact",
            evidence_ids: implementation_evidence.clone(),
        },
        change_kind: "unresolved_implementation_semantics",
        classification: "unresolved",
        rule_id: "cmp.unresolved.insufficient-evidence/v1",
        affected_client_ids: Vec::new(),
        evidence_ids: ordered(contract_evidence.into_iter().chain(implementation_evidence)),
        coverage_gap_ids: vec![gap.id.clone()],
    }
}

fn custom_mapping_gap(
    input: &ImpactAnalysisInput,
    field: &ContractFieldProjection,
) -> ImpactCoverageGap {
    ImpactCoverageGap {
        id: coverage_gap_id(
            &field.field_id,
            "unsupported_custom_provider_mapping",
            &input.baseline.revision,
            &input.target.revision,
        ),
        subject_id: field.field_id.clone(),
        revisions: vec![
            input.baseline.revision.clone(),
            input.target.revision.clone(),
        ],
        evidence_ids: ordered(
            input
                .baseline
                .custom_mapping_evidence_ids
                .iter()
                .chain(&input.target.custom_mapping_evidence_ids)
                .cloned(),
        ),
    }
}

fn client_assessment(
    input: &ImpactAnalysisInput,
    client: &ClientFact,
    changed: &SemanticDiff,
) -> Result<ClientAssessment, ImpactAnalysisError> {
    let assumption = client
        .assumptions
        .iter()
        .find(|(pointer, _)| pointer == &changed.field_pointer)
        .map(|(_, assumption)| *assumption)
        .ok_or(ImpactAnalysisError::UnsupportedSemantics)?;
    let (baseline_risk, target_impact, affected, rule_ids) = match assumption {
        ClientPresenceAssumption::HandlesAbsent => (
            "compatible",
            "compatible",
            false,
            vec!["cmp.response.presence.safe-absence-handling/v1"],
        ),
        ClientPresenceAssumption::RequiresPresent => (
            "potentially_breaking",
            "breaking",
            true,
            vec![
                "cmp.response.presence.client-stricter-than-contract/v1",
                "cmp.response.presence.undocumented-guarantee-removed/v1",
            ],
        ),
        ClientPresenceAssumption::Unknown => {
            return Err(ImpactAnalysisError::UnsupportedSemantics);
        }
    };
    Ok(ClientAssessment {
        client_identity: client.client_id.clone(),
        repository_identity: client.repository_identity.clone(),
        operation_id: input.baseline.contract.operation_id.clone(),
        call_site_id: client.call_site_id.clone(),
        call_symbol: client.call_symbol.clone(),
        presence_assumption: assumption,
        baseline_risk,
        target_impact,
        affected,
        rule_ids,
        evidence_ids: ordered(
            changed
                .contract
                .evidence_ids
                .iter()
                .chain(&changed.implementation.evidence_ids)
                .cloned()
                .chain([client.evidence_id.clone()]),
        ),
        coverage_gap_ids: Vec::new(),
    })
}

fn contract_fields(contract: &ContractProjection) -> BTreeMap<String, &ContractFieldProjection> {
    contract
        .fields
        .iter()
        .map(|field| (field.json_pointer.clone(), field))
        .collect()
}

fn implementation_fields(revision: &ProviderRevisionFacts) -> BTreeMap<String, &ProviderFieldFact> {
    revision
        .fields
        .iter()
        .map(|field| (field.field_pointer.clone(), field))
        .collect()
}

fn client_evidence_id(clients: &[ClientFact], id: &str) -> bool {
    clients.iter().any(|client| client.evidence_id == id)
}

fn referenced_evidence(
    diffs: &[SemanticDiff],
    assessments: &[ClientAssessment],
    rejected: &[RejectedCandidate],
    gaps: &[ImpactCoverageGap],
) -> BTreeSet<String> {
    diffs
        .iter()
        .flat_map(|diff| diff.evidence_ids.iter())
        .chain(
            assessments
                .iter()
                .flat_map(|assessment| assessment.evidence_ids.iter()),
        )
        .chain(
            rejected
                .iter()
                .flat_map(|candidate| candidate.evidence_ids.iter()),
        )
        .chain(gaps.iter().flat_map(|gap| gap.evidence_ids.iter()))
        .cloned()
        .collect()
}

fn check_limit(limit: S7Limit, observed: usize) -> Result<(), ImpactAnalysisError> {
    let observed = u64::try_from(observed).unwrap_or(u64::MAX);
    limit
        .check(observed)
        .map_err(ImpactAnalysisError::LimitExceeded)
}

#[must_use]
pub fn diff_id(
    provider_repository_identity: &str,
    baseline_revision: &str,
    target_revision: &str,
    field_id: &str,
    dimension: &str,
) -> String {
    stable_id(
        "urn:codenoesis:diff:blake3:",
        &[
            DIFF_ID_DOMAIN,
            provider_repository_identity,
            baseline_revision,
            target_revision,
            field_id,
            dimension,
        ],
    )
}

#[must_use]
pub fn evidence_id(
    repository_identity: &str,
    revision: &str,
    path: &str,
    start_line: u64,
    end_line: u64,
    excerpt_sha256: &str,
) -> String {
    stable_id(
        "urn:codenoesis:evidence:blake3:",
        &[
            EVIDENCE_ID_DOMAIN,
            repository_identity,
            revision,
            path,
            &start_line.to_string(),
            &end_line.to_string(),
            excerpt_sha256,
        ],
    )
}

#[must_use]
pub fn coverage_gap_id(
    subject_id: &str,
    reason_code: &str,
    baseline_revision: &str,
    target_revision: &str,
) -> String {
    stable_id(
        "urn:codenoesis:coverage-gap:blake3:",
        &[
            GAP_ID_DOMAIN,
            subject_id,
            reason_code,
            baseline_revision,
            target_revision,
        ],
    )
}

fn stable_id(prefix: &str, components: &[&str]) -> String {
    format!(
        "{prefix}{}",
        blake3::hash(&canonical_string_array(components)).to_hex()
    )
}

fn canonical_string_array(components: &[&str]) -> Vec<u8> {
    let mut bytes = vec![b'['];
    for (index, component) in components.iter().enumerate() {
        if index > 0 {
            bytes.push(b',');
        }
        write_json_string(&mut bytes, component);
    }
    bytes.push(b']');
    bytes
}

fn write_json_string(bytes: &mut Vec<u8>, value: &str) {
    bytes.push(b'"');
    for character in value.chars() {
        match character {
            '"' => bytes.extend_from_slice(br#"\""#),
            '\\' => bytes.extend_from_slice(br"\\"),
            '\u{0008}' => bytes.extend_from_slice(br"\b"),
            '\u{000c}' => bytes.extend_from_slice(br"\f"),
            '\n' => bytes.extend_from_slice(br"\n"),
            '\r' => bytes.extend_from_slice(br"\r"),
            '\t' => bytes.extend_from_slice(br"\t"),
            '\u{0000}'..='\u{001f}' => {
                let _ = write!(ByteWriter(bytes), "\\u{:04x}", u32::from(character));
            }
            _ => {
                let mut buffer = [0; 4];
                bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    bytes.push(b'"');
}

struct ByteWriter<'a>(&'a mut Vec<u8>);

impl std::fmt::Write for ByteWriter<'_> {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.0.extend_from_slice(value.as_bytes());
        Ok(())
    }
}

fn ordered(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
