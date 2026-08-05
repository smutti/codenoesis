use codenoesis_domain::s4_r5::{
    RustSemanticEntityKind, RustSemanticError, RustSemanticLimit, rust_semantic_limit_exceeded,
    rust_semantic_member_id,
};

const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-rust-semantic-depth-v1";
const CRATE_ID: &str =
    "urn:codenoesis:entity:blake3:2f3cd49660b1463ed14a14650a45d77315e5fd66c29edeffc3ba7f1bd46500c0";

#[test]
fn pt_dr_idn_002_r5_member_identities_and_cardinalities() {
    assert_eq!(
        rust_semantic_member_id(
            REPOSITORY_ID,
            CRATE_ID,
            "urn:codenoesis:entity:blake3:eb0d7723ffe04f7e7b1ec27a438f3949fc1332a502397a41578ba96d15842f75",
            RustSemanticEntityKind::Field,
            "key",
            None,
        ),
        "urn:codenoesis:entity:blake3:ab24c82375e533b482ef71fe657a00b190ecf49712498cdc772c8d9db946b9d4"
    );
    assert_eq!(
        rust_semantic_member_id(
            REPOSITORY_ID,
            CRATE_ID,
            CRATE_ID,
            RustSemanticEntityKind::Constant,
            "DEFAULT_LIMIT",
            None,
        ),
        "urn:codenoesis:entity:blake3:ecc0131c233fb18b11b7ff701d7304763f5483c6b21fe3675a415e31bd70e756"
    );
    let paint_render = rust_semantic_member_id(
        REPOSITORY_ID,
        CRATE_ID,
        "urn:codenoesis:entity:blake3:eb0d7723ffe04f7e7b1ec27a438f3949fc1332a502397a41578ba96d15842f75",
        RustSemanticEntityKind::Method,
        "render",
        Some(
            "urn:codenoesis:entity:blake3:e16b7d6ce6da580ba6fa30b4eab297e2d441d7e3aa524e7b2270ae4223a2e142",
        ),
    );
    let preview_render = rust_semantic_member_id(
        REPOSITORY_ID,
        CRATE_ID,
        "urn:codenoesis:entity:blake3:eb0d7723ffe04f7e7b1ec27a438f3949fc1332a502397a41578ba96d15842f75",
        RustSemanticEntityKind::Method,
        "render",
        Some(
            "urn:codenoesis:entity:blake3:37feac9e3551de1a216a7ba5dbd3264cef5d0bf084198fb4d5582b56ab0ff125",
        ),
    );
    assert_eq!(
        paint_render,
        "urn:codenoesis:entity:blake3:1f78bcb85d1c36b3b5fd7c5886002baea09b44bbfb7a0efa8d04963f3272877f"
    );
    assert_eq!(
        preview_render,
        "urn:codenoesis:entity:blake3:4f9b4995acaeb649751a33f150e20e6b70ae1550348e18e5a8eb8f14d7b00164"
    );
    assert_ne!(paint_render, preview_render);
}

#[test]
fn pt_fr_ext_010_limits_have_max_and_plus_one() {
    for (limit, maximum) in [
        (RustSemanticLimit::FieldsPerOwner, 1_024),
        (RustSemanticLimit::VariantsPerEnum, 1_024),
        (RustSemanticLimit::TupleFieldsPerOwner, 1_024),
        (RustSemanticLimit::AssociatedItemsPerContext, 1_024),
        (RustSemanticLimit::OuterAttributesPerDeclaration, 128),
        (RustSemanticLimit::AttributeTokenBytes, 16_384),
        (RustSemanticLimit::DeclaredTypeOrHeaderBytes, 4_096),
    ] {
        assert_eq!(limit.maximum(), maximum);
        assert_eq!(
            rust_semantic_limit_exceeded(limit, maximum + 1),
            RustSemanticError::LimitExceeded {
                limit,
                maximum,
                observed: maximum + 1,
            }
        );
        assert_eq!(
            rust_semantic_limit_exceeded(limit, u64::MAX),
            RustSemanticError::LimitExceeded {
                limit,
                maximum,
                observed: maximum + 1,
            }
        );
    }
}
