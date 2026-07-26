use codenoesis_domain::storage::{
    ArtifactId, ArtifactRole, PublicationBoundary, SnapshotId, StorageError,
};
use serde_json::Value;

const SNAPSHOT_HASH_A: &str = "58f36e23cab8089bd018a9a70ae7585176dfd25628638f57906090fd1ded776f";
const SNAPSHOT_ID_A: &str = "urn:codenoesis:snapshot:blake3:7be8f9f9c453445b9d464b3c909593cf33fbf274f917b3c16e795ba811a14dce";
const SNAPSHOT_ARTIFACT_A: &str = "urn:codenoesis:artifact:blake3:77731ba299209e243651724195af01275710e4c5521378ad1d976f0baec5f61d";

#[test]
fn pt_fr_snp_001_snapshot_and_artifact_ids_match_reviewed_preimages() {
    let snapshot_id = SnapshotId::from_semantic_hash(SNAPSHOT_HASH_A).expect("valid snapshot hash");
    assert_eq!(snapshot_id.as_str(), SNAPSHOT_ID_A);

    let semantic: Value = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/s3/atomic-local-storage-v1/snapshot-semantic-a.json"
    ))
    .expect("parse reviewed semantic A");
    let canonical = serde_json::to_vec(&semantic).expect("canonicalize reviewed semantic A");
    let artifact_id = ArtifactId::from_bytes(&canonical);
    assert_eq!(artifact_id.as_str(), SNAPSHOT_ARTIFACT_A);
    assert_eq!(
        artifact_id.digest(),
        SNAPSHOT_ARTIFACT_A
            .strip_prefix("urn:codenoesis:artifact:blake3:")
            .expect("reviewed artifact prefix")
    );
}

#[test]
fn pt_fr_snp_001_artifact_and_boundary_orders_are_closed() {
    assert_eq!(
        ArtifactRole::ALL.map(ArtifactRole::as_str),
        ["snapshot_semantic", "knowledge_graph", "extraction_chunk"]
    );
    assert_eq!(
        PublicationBoundary::ALL.map(PublicationBoundary::as_str),
        [
            "cas_before_temp_create",
            "cas_after_temp_sync",
            "cas_after_object_move",
            "cas_after_parent_sync",
            "sqlite_after_begin",
            "sqlite_after_snapshot_rows",
            "sqlite_after_head_update",
            "sqlite_after_commit",
        ]
    );
}

#[test]
fn pt_fr_snp_001_only_contention_and_head_conflict_are_retryable() {
    assert!(StorageError::WriterBusy.retryable());
    assert!(
        StorageError::HeadConflict {
            expected: None,
            actual: None,
        }
        .retryable()
    );
    assert!(!StorageError::PublicationFailed.retryable());
    assert!(
        !StorageError::CorruptObject {
            artifact_id: SNAPSHOT_ARTIFACT_A.to_owned(),
            expected_hash: SNAPSHOT_ARTIFACT_A
                .strip_prefix("urn:codenoesis:artifact:blake3:")
                .expect("reviewed artifact prefix")
                .to_owned(),
            observed_hash: "0".repeat(64),
        }
        .retryable()
    );
}
