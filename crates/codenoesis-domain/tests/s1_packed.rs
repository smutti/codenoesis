use codenoesis_domain::s1_packed::{
    PACKED_LIMIT_KINDS, PackedAcquisitionError, PackedComponent, PackedIndexObjectReason,
    PackedObjectDatabaseInvalid,
};
use codenoesis_domain::{AcquisitionError, UnsupportedFeature, check_limit};

#[test]
fn pt_fr_acq_004_limits_have_max_and_plus_one() {
    assert_eq!(PACKED_LIMIT_KINDS.len(), 17);
    for limit in PACKED_LIMIT_KINDS {
        let maximum = limit.maximum();
        assert_eq!(check_limit(limit, maximum), Ok(()), "{limit:?}");
        assert_eq!(
            check_limit(limit, maximum + 1),
            Err(AcquisitionError::LimitExceeded {
                limit,
                maximum,
                observed: maximum + 1,
            }),
            "{limit:?}"
        );
        assert_eq!(
            check_limit(limit, u64::MAX),
            Err(AcquisitionError::LimitExceeded {
                limit,
                maximum,
                observed: maximum + 1,
            }),
            "{limit:?}"
        );
    }
}

#[test]
fn conf_fr_acq_004_invalid_context_is_shape_safe() {
    let pack_id =
        codenoesis_domain::ObjectId::parse_sha1("1111111111111111111111111111111111111111")
            .expect("pack ID");
    let object_oid =
        codenoesis_domain::ObjectId::parse_sha1("2222222222222222222222222222222222222222")
            .expect("object OID");
    let failure = PackedAcquisitionError::Invalid(PackedObjectDatabaseInvalid::IndexObject {
        reason: PackedIndexObjectReason::Offset,
        pack_id,
        object_oid,
    });
    assert_eq!(failure.component(), PackedComponent::Index);
    assert_eq!(failure.reason(), Some("index_offset"));
    assert!(matches!(
        UnsupportedFeature::packed_acquisition(failure),
        UnsupportedFeature::PackedAcquisition(_)
    ));
}
