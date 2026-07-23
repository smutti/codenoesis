//! Versioned JSON contracts for the `CodeNoesis` S0 slice.

use codenoesis_domain::{AcquisitionError, BoundRevision, InputError};
use serde_json::{Value, json};

const CONFIGURATION_HASH_DOMAIN: &[u8] = b"codenoesis.configuration.semantic.v1";
const SNAPSHOT_HASH_DOMAIN: &[u8] = b"codenoesis.repository-snapshot.semantic.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotEnvelopeV1 {
    created_at: String,
    job_id: Option<String>,
    correlation_id: String,
}

impl SnapshotEnvelopeV1 {
    #[must_use]
    pub const fn new(created_at: String, job_id: Option<String>, correlation_id: String) -> Self {
        Self {
            created_at,
            job_id,
            correlation_id,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RepositorySnapshotV1 {
    value: Value,
}

impl RepositorySnapshotV1 {
    #[must_use]
    pub fn from_bound_revision(bound: &BoundRevision, envelope: SnapshotEnvelopeV1) -> Self {
        let SnapshotEnvelopeV1 {
            created_at,
            job_id,
            correlation_id,
        } = envelope;
        let configuration_semantic = json!({"profile": "standard-local-s0"});
        let configuration_hash = semantic_hash(CONFIGURATION_HASH_DOMAIN, &configuration_semantic);
        let semantic = json!({
            "repository": {
                "contract_version": "codenoesis.repository/v1",
                "identity_schema_version": "codenoesis.repository-identity/v1",
                "identity": bound.repository_identity().as_str(),
                "vcs": "git",
                "object_format": "sha1",
                "commit_oid": bound.commit_oid().as_str(),
                "tree_oid": bound.tree_oid().as_str()
            },
            "configuration": {
                "schema_version": "codenoesis.configuration/v1",
                "profile": "standard-local-s0",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": configuration_hash
                }
            },
            "pipeline_version": "codenoesis.pipeline/s0-v1",
            "ontology_version": "codenoesis.ontology/none-v1",
            "extractor_contract_version": "codenoesis.extraction/v1",
            "extractor_versions": [],
            "evidence_lineage_version": "codenoesis.evidence-lineage/v1"
        });
        let snapshot_hash = semantic_hash(SNAPSHOT_HASH_DOMAIN, &semantic);
        Self {
            value: json!({
                "schema_version": "codenoesis.repository-snapshot/v1",
                "semantic_hash": {
                    "algorithm": "blake3-256",
                    "value": snapshot_hash
                },
                "semantic": semantic,
                "envelope": {
                    "created_at": created_at,
                    "job_id": job_id,
                    "correlation_id": correlation_id
                }
            }),
        }
    }

    /// Serializes the complete snapshot as RFC 8785-compatible S0 bytes.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internally constructed JSON value cannot be
    /// serialized.
    pub fn canonical_stdout(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Serializes the semantic value as RFC 8785-compatible S0 bytes.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internally constructed JSON value cannot be
    /// serialized.
    pub fn canonical_semantic(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&self.value["semantic"])
    }

    #[must_use]
    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Clone, Debug)]
pub struct CodeNoesisErrorV1 {
    value: Value,
}

impl CodeNoesisErrorV1 {
    #[must_use]
    pub fn from_input(error: InputError) -> Self {
        let code = match error {
            InputError::InvalidRepositoryIdentity => "input.invalid_repository_identity",
            InputError::InvalidRevision => "input.invalid_revision",
        };
        Self::new(code, "input", &error.to_string(), &json!({}))
    }

    #[must_use]
    pub fn from_acquisition(error: &AcquisitionError) -> Self {
        match error {
            AcquisitionError::NotGitRepository => Self::new(
                "acquisition.not_git_repository",
                "acquisition",
                &error.to_string(),
                &json!({}),
            ),
            AcquisitionError::RevisionNotFound { revision } => Self::new(
                "acquisition.revision_not_found",
                "acquisition",
                &error.to_string(),
                &json!({"revision": revision.as_str()}),
            ),
            AcquisitionError::RevisionNotCommit {
                object_oid,
                actual_kind,
            } => Self::new(
                "acquisition.revision_not_commit",
                "acquisition",
                &error.to_string(),
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
                &json!({
                    "object_oid": object_oid.as_str(),
                    "expected_kind": expected_kind.as_str()
                }),
            ),
            AcquisitionError::UnsupportedRepositoryShape { feature } => Self::new(
                "acquisition.unsupported_repository_shape",
                "acquisition",
                &error.to_string(),
                &json!({"feature": feature.as_str()}),
            ),
        }
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            "internal.unexpected",
            "internal",
            "unexpected internal failure",
            &json!({}),
        )
    }

    fn new(code: &str, stage: &str, message: &str, context: &Value) -> Self {
        Self {
            value: json!({
                "schema_version": "codenoesis.error/v1",
                "code": code,
                "stage": stage,
                "message": message,
                "retryable": false,
                "context": context
            }),
        }
    }

    /// Serializes one strict error document followed by one LF.
    ///
    /// # Errors
    ///
    /// Returns an error only if the internally constructed JSON value cannot be
    /// serialized.
    pub fn canonical_stderr(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

fn semantic_hash(domain: &[u8], value: &Value) -> String {
    let canonical =
        serde_json::to_vec(value).expect("JSON values constructed by CodeNoesis serialize");
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&[0]);
    hasher.update(&canonical);
    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    use codenoesis_domain::{BoundRevision, ObjectId, RepositoryIdentity};
    use serde_json::{Map, Value, json};

    use super::{RepositorySnapshotV1, SNAPSHOT_HASH_DOMAIN, SnapshotEnvelopeV1, semantic_hash};

    const COMMIT_A_OID: &str = "6d4152a7787ac82eedf3f9fc5df408dfdf6e412f";
    const TREE_A_OID: &str = "892c4a33b5529ba6b6651fc26765957f11f7ba9e";
    const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s0-one-file-v1";
    const SNAPSHOT_A_HASH: &str =
        "b673624a329f43fd84852bbdeefd66326a7fcb1c03fdb626e2de6bfedff11997";

    fn bound_revision() -> BoundRevision {
        BoundRevision::new(
            RepositoryIdentity::parse(REPOSITORY_ID).expect("approved fixture identity"),
            ObjectId::parse_sha1(COMMIT_A_OID).expect("approved commit OID"),
            ObjectId::parse_sha1(TREE_A_OID).expect("approved tree OID"),
        )
    }

    fn snapshot(envelope: SnapshotEnvelopeV1) -> RepositorySnapshotV1 {
        RepositorySnapshotV1::from_bound_revision(&bound_revision(), envelope)
    }

    fn fixed_envelope() -> SnapshotEnvelopeV1 {
        SnapshotEnvelopeV1::new(
            "2000-01-01T00:00:00Z".to_owned(),
            None,
            "s0-golden-a".to_owned(),
        )
    }

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/s0/one-file-v1")
            .join(name)
    }

    fn reviewed_jcs_body(name: &str) -> Vec<u8> {
        let mut bytes = fs::read(fixture_path(name)).expect("read reviewed JCS golden");
        if bytes.ends_with(b"\r\n") {
            bytes.truncate(bytes.len() - 2);
        } else {
            assert_eq!(bytes.pop(), Some(b'\n'), "golden must end in one newline");
        }
        assert!(
            !bytes.contains(&b'\r') && !bytes.contains(&b'\n'),
            "golden body must be one canonical JSON line"
        );
        bytes
    }

    #[test]
    fn conf_dr_art_001_repository_snapshot_v1() {
        let actual = snapshot(fixed_envelope())
            .canonical_stdout()
            .expect("serialize fixed snapshot");
        let mut expected = reviewed_jcs_body("expected-snapshot-a.jcs");
        expected.push(b'\n');
        assert_eq!(actual, expected);

        let value: Value = serde_json::from_slice(&actual).expect("parse generated snapshot");
        assert_exact_keys(
            &value,
            &["envelope", "schema_version", "semantic", "semantic_hash"],
        );
        assert_exact_keys(
            &value["semantic"],
            &[
                "configuration",
                "evidence_lineage_version",
                "extractor_contract_version",
                "extractor_versions",
                "ontology_version",
                "pipeline_version",
                "repository",
            ],
        );
        assert_exact_keys(
            &value["semantic"]["repository"],
            &[
                "commit_oid",
                "contract_version",
                "identity",
                "identity_schema_version",
                "object_format",
                "tree_oid",
                "vcs",
            ],
        );
        assert_exact_keys(
            &value["semantic"]["configuration"],
            &["profile", "schema_version", "semantic_hash"],
        );
        assert_exact_keys(
            &value["envelope"],
            &["correlation_id", "created_at", "job_id"],
        );
    }

    #[test]
    fn pt_dr_art_002_volatile_envelope_preserves_semantic_hash() {
        let baseline = snapshot(fixed_envelope());
        let baseline_semantic = baseline
            .canonical_semantic()
            .expect("serialize baseline semantic");
        let baseline_hash = baseline.value()["semantic_hash"].clone();
        let mut baseline_without_envelope = baseline.value().clone();
        baseline_without_envelope
            .as_object_mut()
            .expect("snapshot object")
            .remove("envelope");
        let fixed_stdout = baseline
            .canonical_stdout()
            .expect("serialize fixed snapshot");

        for index in 0..50 {
            let candidate = snapshot(SnapshotEnvelopeV1::new(
                format!("2000-01-01T00:00:{index:02}Z"),
                (index % 2 == 0).then(|| format!("job-{index}")),
                format!("correlation-{index}"),
            ));
            assert_eq!(
                candidate
                    .canonical_semantic()
                    .expect("serialize candidate semantic"),
                baseline_semantic,
                "semantic bytes changed for envelope {index}"
            );
            assert_eq!(candidate.value()["semantic_hash"], baseline_hash);

            let mut candidate_without_envelope = candidate.value().clone();
            candidate_without_envelope
                .as_object_mut()
                .expect("snapshot object")
                .remove("envelope");
            assert_eq!(candidate_without_envelope, baseline_without_envelope);
            assert_eq!(
                snapshot(fixed_envelope())
                    .canonical_stdout()
                    .expect("serialize replayed fixed snapshot"),
                fixed_stdout
            );
        }
    }

    #[test]
    fn pt_nfr_det_001_permutation_and_schedule_invariant() {
        let expected = reviewed_jcs_body("expected-semantic-a.jcs");

        for seed in 0..50 {
            let semantic = permuted_semantic(seed);
            let canonical = serde_json::to_vec(&semantic).expect("serialize permuted semantic");
            assert_eq!(
                canonical, expected,
                "canonical bytes differ for seed {seed}"
            );
            assert_eq!(
                semantic_hash(SNAPSHOT_HASH_DOMAIN, &semantic),
                SNAPSHOT_A_HASH,
                "semantic hash differs for seed {seed}"
            );
        }
    }

    fn assert_exact_keys(value: &Value, expected: &[&str]) {
        let actual = value
            .as_object()
            .expect("contract node must be an object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = expected.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(actual, expected);
    }

    fn permuted_semantic(seed: usize) -> Value {
        let repository = permuted_object(
            vec![
                ("contract_version", json!("codenoesis.repository/v1")),
                (
                    "identity_schema_version",
                    json!("codenoesis.repository-identity/v1"),
                ),
                ("identity", json!(REPOSITORY_ID)),
                ("vcs", json!("git")),
                ("object_format", json!("sha1")),
                ("commit_oid", json!(COMMIT_A_OID)),
                ("tree_oid", json!(TREE_A_OID)),
            ],
            seed,
        );
        let configuration = permuted_object(
            vec![
                ("schema_version", json!("codenoesis.configuration/v1")),
                ("profile", json!("standard-local-s0")),
                (
                    "semantic_hash",
                    json!({
                        "algorithm": "blake3-256",
                        "value": "4811a917bebed264f49382d65825686ad5ca506ce39bc51385e547b0c7ced1c0"
                    }),
                ),
            ],
            seed.wrapping_mul(3).wrapping_add(1),
        );
        permuted_object(
            vec![
                ("repository", repository),
                ("configuration", configuration),
                ("pipeline_version", json!("codenoesis.pipeline/s0-v1")),
                ("ontology_version", json!("codenoesis.ontology/none-v1")),
                (
                    "extractor_contract_version",
                    json!("codenoesis.extraction/v1"),
                ),
                ("extractor_versions", json!([])),
                (
                    "evidence_lineage_version",
                    json!("codenoesis.evidence-lineage/v1"),
                ),
            ],
            seed.wrapping_mul(7).wrapping_add(2),
        )
    }

    fn permuted_object(mut entries: Vec<(&'static str, Value)>, seed: usize) -> Value {
        let length = entries.len();
        entries.rotate_left(seed % length);
        if seed % 2 == 1 {
            entries.reverse();
        }
        let mut object = Map::new();
        for (key, value) in entries {
            object.insert(key.to_owned(), value);
        }
        Value::Object(object)
    }
}
