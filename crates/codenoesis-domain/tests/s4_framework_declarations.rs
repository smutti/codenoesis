use codenoesis_domain::s4_r6::{
    FRAMEWORK_DECLARATION_ID_DOMAIN, FrameworkError, FrameworkLimit, FrameworkRole,
    FrameworkSourceProfile, framework_declaration_id, framework_declaration_identity_preimage,
    framework_limit_exceeded,
};

#[test]
fn pt_dr_idn_002_r6_framework_identity_nfc() {
    let repository_identity = "urn:codenoesis:fixture:s4-framework-declarations-v1";
    let crate_id = "urn:codenoesis:entity:blake3:e9cfdaf582459abdde82d1b62d389c129ef0f5d58bf33a1927509148e87fa143";
    let lexical_owner_id = "urn:codenoesis:entity:blake3:8b84ec9d439df65f58ef72368452292e9be83810734058216c3e0ffafa080efb";
    let id = framework_declaration_id(
        repository_identity,
        crate_id,
        lexical_owner_id,
        FrameworkRole::Route,
        FrameworkSourceProfile::ExplicitBuilderRegistration,
        "builder-tail/route/v1",
        "GET /health -> handlers::health",
    );
    assert_eq!(
        framework_declaration_identity_preimage(
            repository_identity,
            crate_id,
            lexical_owner_id,
            FrameworkRole::Route,
            FrameworkSourceProfile::ExplicitBuilderRegistration,
            "builder-tail/route/v1",
            "GET /health -> handlers::health",
        ),
        format!(
            "[\"codenoesis.entity-id/framework-declaration/v1\",\"{repository_identity}\",\"{crate_id}\",\"{lexical_owner_id}\",\"route\",\"explicit-builder-registration-v1\",\"builder-tail/route/v1\",\"GET /health -> handlers::health\"]"
        )
        .into_bytes()
    );
    assert_eq!(
        FRAMEWORK_DECLARATION_ID_DOMAIN,
        "codenoesis.entity-id/framework-declaration/v1"
    );
    assert_eq!(
        id,
        "urn:codenoesis:entity:blake3:66d0fa6b5793836d5659ffdbe9ebcceddb90947fe47b4be6e2408c44b84b71b5"
    );
}

#[test]
fn pt_fr_ext_011_limits_have_max_and_plus_one() {
    for limit in FrameworkLimit::ALL {
        let maximum = limit.maximum();
        assert_eq!(
            framework_limit_exceeded(limit, maximum + 2),
            FrameworkError::LimitExceeded {
                limit,
                maximum,
                observed: maximum + 1,
            }
        );
    }
}
