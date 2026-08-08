use codenoesis_contracts::{RepositorySnapshotV11, RepositorySnapshotV11Error};
use codenoesis_domain::{K1OutputCapacityProfile, LOCAL_SNAPSHOT_64M_V1, STANDARD_LOCAL_S1_LIMITS};

#[test]
fn conf_fr_ext_012_k1_output_capacity_profile_is_additive() {
    assert_eq!(STANDARD_LOCAL_S1_LIMITS.canonical_output_bytes, 33_554_432);
    assert_eq!(LOCAL_SNAPSHOT_64M_V1, "local-snapshot-64m-v1");
    assert_eq!(
        K1OutputCapacityProfile::Standard.maximum_bytes(),
        33_554_432
    );
    assert_eq!(
        K1OutputCapacityProfile::LocalSnapshot64MV1.maximum_bytes(),
        67_108_864
    );

    let serializer: fn(
        &RepositorySnapshotV11,
        K1OutputCapacityProfile,
    ) -> Result<Vec<u8>, RepositorySnapshotV11Error> =
        RepositorySnapshotV11::canonical_stdout_with_output_capacity;
    let _ = serializer;
}
