use codenoesis_domain::{
    K1OutputCapacityProfile, LOCAL_SNAPSHOT_1G_CANONICAL_OUTPUT_BYTES, LOCAL_SNAPSHOT_1G_V1,
    LOCAL_SNAPSHOT_2G_CANONICAL_OUTPUT_BYTES, LOCAL_SNAPSHOT_2G_V1,
    LOCAL_SNAPSHOT_256M_CANONICAL_OUTPUT_BYTES, LOCAL_SNAPSHOT_256M_V1,
    LOCAL_SNAPSHOT_512M_CANONICAL_OUTPUT_BYTES, LOCAL_SNAPSHOT_512M_V1,
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

#[test]
fn conf_fr_cli_001_rust_512m_capacity_is_exact_and_explicit() {
    assert_eq!(LOCAL_SNAPSHOT_512M_V1, "local-snapshot-512m-v1");
    assert_eq!(LOCAL_SNAPSHOT_512M_CANONICAL_OUTPUT_BYTES, 536_870_912);
    assert_eq!(
        K1OutputCapacityProfile::LocalSnapshot512MV1.maximum_bytes(),
        LOCAL_SNAPSHOT_512M_CANONICAL_OUTPUT_BYTES
    );
}

#[test]
fn conf_fr_cli_001_rust_1g_capacity_is_exact_and_explicit() {
    assert_eq!(LOCAL_SNAPSHOT_1G_V1, "local-snapshot-1g-v1");
    assert_eq!(LOCAL_SNAPSHOT_1G_CANONICAL_OUTPUT_BYTES, 1_073_741_824);
    assert_eq!(
        K1OutputCapacityProfile::LocalSnapshot1GV1.maximum_bytes(),
        LOCAL_SNAPSHOT_1G_CANONICAL_OUTPUT_BYTES
    );
}

#[test]
fn conf_fr_cli_001_rust_2g_capacity_is_exact_and_explicit() {
    assert_eq!(LOCAL_SNAPSHOT_2G_V1, "local-snapshot-2g-v1");
    assert_eq!(LOCAL_SNAPSHOT_2G_CANONICAL_OUTPUT_BYTES, 2_147_483_648);
    assert_eq!(
        K1OutputCapacityProfile::LocalSnapshot2GV1.maximum_bytes(),
        LOCAL_SNAPSHOT_2G_CANONICAL_OUTPUT_BYTES
    );
}
