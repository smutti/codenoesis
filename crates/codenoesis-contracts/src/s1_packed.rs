use codenoesis_domain::s1_packed::{PackedAcquisitionError, PackedObjectDatabaseInvalid};
use codenoesis_domain::{AcquisitionError, InputError, UnsupportedFeature};
use serde_json::{Value, json};

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV6 {
    value: Value,
}

impl CodeNoesisErrorV6 {
    #[must_use]
    pub fn invalid_acquisition_profile() -> Self {
        Self::new(
            "input.invalid_acquisition_profile",
            "input",
            "invalid acquisition profile",
            false,
            &json!({}),
        )
    }

    #[must_use]
    pub fn from_input(error: InputError) -> Self {
        let code = match error {
            InputError::InvalidRepositoryIdentity => "input.invalid_repository_identity",
            InputError::InvalidRevision | InputError::InvalidStoreRoot => "input.invalid_revision",
            InputError::InvalidProfile => "input.invalid_profile",
        };
        Self::new(code, "input", &error.to_string(), false, &json!({}))
    }

    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn from_acquisition(error: &AcquisitionError) -> Self {
        match error {
            AcquisitionError::NotGitRepository => Self::new(
                "acquisition.not_git_repository",
                "acquisition",
                &error.to_string(),
                false,
                &json!({}),
            ),
            AcquisitionError::RevisionNotFound { revision } => Self::new(
                "acquisition.revision_not_found",
                "acquisition",
                &error.to_string(),
                false,
                &json!({"revision": revision.as_str()}),
            ),
            AcquisitionError::RevisionNotCommit {
                object_oid,
                actual_kind,
            } => Self::new(
                "acquisition.revision_not_commit",
                "acquisition",
                &error.to_string(),
                false,
                &json!({
                    "object_oid": object_oid.as_str(),
                    "actual_kind": actual_kind.as_str()
                }),
            ),
            AcquisitionError::ObjectMissing {
                object_oid,
                expected_kind,
                referenced_by,
            } => Self::new(
                "acquisition.object_missing",
                "acquisition",
                &error.to_string(),
                false,
                &json!({
                    "object_oid": object_oid.as_str(),
                    "expected_kind": expected_kind.as_str(),
                    "referenced_by": referenced_by.as_str()
                }),
            ),
            AcquisitionError::RepositoryInconsistent {
                object_oid,
                expected_kind,
            } => Self::new(
                "acquisition.repository_inconsistent",
                "acquisition",
                &error.to_string(),
                false,
                &json!({
                    "object_oid": object_oid.as_str(),
                    "expected_kind": expected_kind.as_str()
                }),
            ),
            AcquisitionError::UnsupportedRepositoryShape {
                feature: UnsupportedFeature::PackedAcquisition(error),
            } => Self::from_packed_acquisition(error),
            AcquisitionError::UnsupportedRepositoryShape { feature } => Self::new(
                "acquisition.unsupported_repository_shape",
                "acquisition",
                &error.to_string(),
                false,
                &json!({"feature": feature.as_str()}),
            ),
            AcquisitionError::PathInvalid { reason } => Self::new(
                "acquisition.path_invalid",
                "acquisition",
                &error.to_string(),
                false,
                &json!({"reason": reason.as_str()}),
            ),
            AcquisitionError::RootPolicyViolation { policy } => Self::new(
                "acquisition.root_policy_violation",
                "acquisition",
                &error.to_string(),
                false,
                &json!({"policy": policy.as_str()}),
            ),
            AcquisitionError::EntryPolicyViolation { path, entry } => Self::new(
                "acquisition.entry_policy_violation",
                "acquisition",
                &error.to_string(),
                false,
                &json!({"entry": entry.as_str(), "path": path}),
            ),
            AcquisitionError::LimitExceeded {
                limit,
                maximum,
                observed,
            } => Self::new(
                "acquisition.limit_exceeded",
                "acquisition",
                &error.to_string(),
                false,
                &json!({
                    "limit": limit.as_str(),
                    "maximum": maximum,
                    "observed": observed
                }),
            ),
        }
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal failure",
            false,
            &json!({}),
        )
    }

    fn from_packed_acquisition(error: &PackedAcquisitionError) -> Self {
        match error {
            PackedAcquisitionError::Invalid(error) => Self::new(
                "acquisition.object_database_invalid",
                "acquisition",
                "Git object database is invalid",
                false,
                &invalid_context(error),
            ),
            PackedAcquisitionError::Changed(component) => Self::new(
                "acquisition.object_database_changed",
                "acquisition",
                "Git object database changed during acquisition",
                true,
                &json!({"component": component.as_str()}),
            ),
            PackedAcquisitionError::Unavailable(component) => Self::new(
                "acquisition.object_database_unavailable",
                "acquisition",
                "Git object database is unavailable",
                false,
                &json!({"component": component.as_str()}),
            ),
        }
    }

    fn new(code: &str, stage: &str, message: &str, retryable: bool, context: &Value) -> Self {
        Self {
            value: json!({
                "schema_version": "codenoesis.error/v6",
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": retryable,
                "context": context
            }),
        }
    }

    /// Serializes the strict V6 error document followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns the underlying JSON serialization error.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

fn invalid_context(error: &PackedObjectDatabaseInvalid) -> Value {
    match error {
        PackedObjectDatabaseInvalid::CatalogEntry => {
            json!({"component": "catalog", "reason": "catalog_entry"})
        }
        PackedObjectDatabaseInvalid::Index { reason, pack_id } => json!({
            "component": "index",
            "reason": reason.as_str(),
            "pack_id": pack_id.as_str()
        }),
        PackedObjectDatabaseInvalid::IndexObject {
            reason,
            pack_id,
            object_oid,
        } => json!({
            "component": "index",
            "reason": reason.as_str(),
            "pack_id": pack_id.as_str(),
            "object_oid": object_oid.as_str()
        }),
        PackedObjectDatabaseInvalid::Pack { reason, pack_id } => json!({
            "component": "pack",
            "reason": reason.as_str(),
            "pack_id": pack_id.as_str()
        }),
        PackedObjectDatabaseInvalid::Entry {
            reason,
            pack_id,
            object_oid,
        } => json!({
            "component": "entry",
            "reason": reason.as_str(),
            "pack_id": pack_id.as_str(),
            "object_oid": object_oid.as_str()
        }),
        PackedObjectDatabaseInvalid::Object { reason, object_oid } => json!({
            "component": "object",
            "reason": reason.as_str(),
            "object_oid": object_oid.as_str()
        }),
        PackedObjectDatabaseInvalid::Delta {
            reason,
            pack_id,
            object_oid,
        } => json!({
            "component": "delta",
            "reason": reason.as_str(),
            "pack_id": pack_id.as_str(),
            "object_oid": object_oid.as_str()
        }),
    }
}
