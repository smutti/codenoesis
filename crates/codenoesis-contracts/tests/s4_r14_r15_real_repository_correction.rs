use codenoesis_contracts::{
    RepositorySnapshotV16, RepositorySnapshotV16Error, RepositorySnapshotV17,
    RepositorySnapshotV17Error,
};
use codenoesis_domain::{
    K1OutputCapacityProfile, LOCAL_SNAPSHOT_256M_CANONICAL_OUTPUT_BYTES, LOCAL_SNAPSHOT_256M_V1,
};

#[test]
fn conf_fr_cli_001_r14_r15_share_the_bounded_256m_envelope() {
    assert_eq!(LOCAL_SNAPSHOT_256M_V1, "local-snapshot-256m-v1");
    assert_eq!(
        K1OutputCapacityProfile::LocalSnapshot256MV1.maximum_bytes(),
        LOCAL_SNAPSHOT_256M_CANONICAL_OUTPUT_BYTES
    );

    let r14_serializer: fn(&RepositorySnapshotV16) -> Result<Vec<u8>, RepositorySnapshotV16Error> =
        RepositorySnapshotV16::canonical_stdout;
    let r15_serializer: fn(&RepositorySnapshotV17) -> Result<Vec<u8>, RepositorySnapshotV17Error> =
        RepositorySnapshotV17::canonical_stdout;
    let _ = (r14_serializer, r15_serializer);
}
