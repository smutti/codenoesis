use codenoesis_domain::s4_k1::{
    CallableRelationshipKind, CallableSemanticEntityKind, CallableSemanticsError,
    CallableSemanticsLimit, callable_body_fact_id, callable_parameter_id, callable_relationship_id,
    callable_signature_id, declared_value_id, enforce_limit,
};

#[test]
fn pt_dr_idn_002_k1_identity_preimages_and_collisions() {
    let repository = "urn:codenoesis:test:k1-identities";
    let callable = format!("urn:codenoesis:entity:blake3:{}", "1".repeat(64));
    let declaration = format!("urn:codenoesis:entity:blake3:{}", "2".repeat(64));
    let evidence = format!("urn:codenoesis:evidence:blake3:{}", "3".repeat(64));
    let signature = callable_signature_id(repository, &callable);
    let parameter = callable_parameter_id(repository, &callable, 0, "café");
    let normalized_parameter = callable_parameter_id(repository, &callable, 0, "café");
    let value = declared_value_id(repository, &declaration);
    let body = callable_body_fact_id(
        repository,
        &callable,
        CallableSemanticEntityKind::CallSite,
        &evidence,
    );
    let relationship = callable_relationship_id(
        CallableRelationshipKind::HasSignature,
        &callable,
        &signature,
    );

    for identity in [signature, parameter.clone(), value, body, relationship] {
        assert!(identity.rsplit(':').next().is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
        }));
    }
    assert_eq!(parameter, normalized_parameter);
    assert_ne!(
        callable_parameter_id(repository, &callable, 0, "café"),
        callable_parameter_id(repository, &callable, 1, "café")
    );
    assert_ne!(
        callable_parameter_id(repository, &callable, 0, "café"),
        callable_parameter_id(repository, &callable, 0, "cafe")
    );
}

#[test]
fn pt_fr_ext_012_all_limits_have_maximum_plus_one() {
    for limit in CallableSemanticsLimit::ALL {
        let maximum = limit.maximum();
        let maximum_usize = usize::try_from(maximum).expect("K1 limit fits usize");
        assert_eq!(enforce_limit(limit, maximum_usize), Ok(()));
        assert_eq!(
            enforce_limit(limit, maximum_usize + 1),
            Err(CallableSemanticsError::LimitExceeded {
                limit,
                maximum,
                observed: maximum + 1,
            })
        );
    }
}
