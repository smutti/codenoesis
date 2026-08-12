use codenoesis_domain::{
    K1OutputCapacityProfile, LOCAL_SNAPSHOT_256M_CANONICAL_OUTPUT_BYTES, LOCAL_SNAPSHOT_256M_V1,
};

#[test]
fn conf_fr_cli_001_r14_r15_256m_capacity_is_exact_and_explicit() {
    assert_eq!(LOCAL_SNAPSHOT_256M_V1, "local-snapshot-256m-v1");
    assert_eq!(LOCAL_SNAPSHOT_256M_CANONICAL_OUTPUT_BYTES, 268_435_456);
    assert_eq!(
        K1OutputCapacityProfile::LocalSnapshot256MV1.maximum_bytes(),
        LOCAL_SNAPSHOT_256M_CANONICAL_OUTPUT_BYTES
    );
}
