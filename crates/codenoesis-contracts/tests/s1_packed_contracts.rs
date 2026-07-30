use codenoesis_contracts::CodeNoesisErrorV6;
use codenoesis_domain::s1_packed::{
    PackedAcquisitionError, PackedComponent, PackedIndexObjectReason, PackedObjectDatabaseInvalid,
};
use codenoesis_domain::{AcquisitionError, ObjectId, UnsupportedFeature};
use serde_json::{Value, json};

#[test]
fn conf_fr_acq_004_error_v6_exact_contexts() {
    assert_error(
        &CodeNoesisErrorV6::invalid_acquisition_profile(),
        &json!({
            "schema_version": "codenoesis.error/v6",
            "code": "input.invalid_acquisition_profile",
            "stage": "input",
            "message": "invalid acquisition profile",
            "retryable": false,
            "context": {}
        }),
    );

    let pack_id = oid("1111111111111111111111111111111111111111");
    let object_oid = oid("2222222222222222222222222222222222222222");
    let invalid = packed_error(PackedAcquisitionError::Invalid(
        PackedObjectDatabaseInvalid::IndexObject {
            reason: PackedIndexObjectReason::Offset,
            pack_id,
            object_oid,
        },
    ));
    assert_error(
        &CodeNoesisErrorV6::from_acquisition(&invalid),
        &json!({
            "schema_version": "codenoesis.error/v6",
            "code": "acquisition.object_database_invalid",
            "stage": "acquisition",
            "message": "Git object database is invalid",
            "retryable": false,
            "context": {
                "component": "index",
                "reason": "index_offset",
                "pack_id": "1111111111111111111111111111111111111111",
                "object_oid": "2222222222222222222222222222222222222222"
            }
        }),
    );

    let changed = packed_error(PackedAcquisitionError::Changed(PackedComponent::Catalog));
    assert_error(
        &CodeNoesisErrorV6::from_acquisition(&changed),
        &json!({
            "schema_version": "codenoesis.error/v6",
            "code": "acquisition.object_database_changed",
            "stage": "acquisition",
            "message": "Git object database changed during acquisition",
            "retryable": true,
            "context": {"component": "catalog"}
        }),
    );

    let unavailable = packed_error(PackedAcquisitionError::Unavailable(PackedComponent::Pack));
    assert_error(
        &CodeNoesisErrorV6::from_acquisition(&unavailable),
        &json!({
            "schema_version": "codenoesis.error/v6",
            "code": "acquisition.object_database_unavailable",
            "stage": "acquisition",
            "message": "Git object database is unavailable",
            "retryable": false,
            "context": {"component": "pack"}
        }),
    );
}

fn packed_error(error: PackedAcquisitionError) -> AcquisitionError {
    AcquisitionError::UnsupportedRepositoryShape {
        feature: UnsupportedFeature::packed_acquisition(error),
    }
}

fn oid(value: &str) -> ObjectId {
    ObjectId::parse_sha1(value).expect("valid OID")
}

fn assert_error(error: &CodeNoesisErrorV6, expected: &Value) {
    let bytes = error.canonical_stderr().expect("serialize V6");
    assert_eq!(bytes.last(), Some(&b'\n'));
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).expect("strict JSON"),
        *expected
    );
}
