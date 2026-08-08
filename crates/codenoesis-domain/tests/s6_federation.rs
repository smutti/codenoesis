use codenoesis_domain::s6::{
    ClientBinding, ClientDeclaration, FederationError, FederationEvidence, FederationLimit,
    HttpMethod, JsonSchemaType, OperationField, ProviderBinding, ProviderContract,
    ProviderOperation, ResourceCounter, SourceFormat, call_site_id, client_id, federate, field_id,
    operation_id, schema_id, service_id, yaml_evidence_id,
};

#[test]
fn pt_fr_fed_001_reviewed_identity_preimages_are_exact() {
    let service = service_id("https://api.example.invalid");
    assert_eq!(
        service,
        "urn:codenoesis:service:blake3:509813cd6e049acb2c9de79cb9f0a1f385b355473512592a22d5f115ac9243cc"
    );
    let operation = operation_id(&service, HttpMethod::Get, "/users/{id}", "getUser");
    assert_eq!(
        operation,
        "urn:codenoesis:operation:blake3:071cbb8fa33a959879d7d8a2bfbbac31e1fea4850c28fdb73227c605f5974923"
    );
    assert_eq!(
        schema_id(&operation, "response", "200", "#/components/schemas/User"),
        "urn:codenoesis:schema:blake3:d5ccc3bfeafd3668993d4a6f520231d62b47ca63dbac467b5ea1d23d13032c68"
    );
    assert_eq!(
        field_id(&operation, "response", "200", "/id"),
        "urn:codenoesis:field:blake3:18ccd66ca65b4c53a46dfb46b7f376985a376e24e6d1dd1d5614dedeca73d88c"
    );
    let client = client_id("urn:codenoesis:fixture:s7-client-strict");
    assert_eq!(
        client,
        "urn:codenoesis:client:blake3:ebd716cc12c29ad94e757a84d525b057f53ab8d3e6f9e21a35ffa55a6b8057a9"
    );
    assert_eq!(
        call_site_id(
            &client,
            "fixture-client-v1",
            "src/commonMain/kotlin/dev/codenoesis/fixture/StrictUsersClient.kt",
            "getStrictUser"
        ),
        "urn:codenoesis:call-site:blake3:a59693122eeb35ee111acb47cfa2a20eafb4c75746ed60ae9ed61aa4e859884b"
    );
}

#[test]
fn conf_fr_fed_001_yaml_evidence_preimage_is_reproducible() {
    assert_eq!(
        yaml_evidence_id(
            "urn:codenoesis:fixture:s7-provider",
            "fixture-provider-a",
            "provider/openapi.yaml",
            "#/paths/~1users~1{id}/get",
            9,
            17,
            "d6decc18d428316b209aa554ee028fe9db8761df515bf34b9e92c3a369f2de3d"
        ),
        "urn:codenoesis:evidence:blake3:0eb6cb716e2451f7003c0437b339c93ba0c727bc40add79c7a6f7c99ad8e7990"
    );
}

#[test]
fn pt_fr_fed_001_every_counter_accepts_max_and_rejects_plus_one() {
    let limits = [
        FederationLimit::WorkspaceManifestBytes,
        FederationLimit::Repositories,
        FederationLimit::ContractDocuments,
        FederationLimit::ContractBytesPerDocument,
        FederationLimit::YamlNestingDepth,
        FederationLimit::LocalRefDepth,
        FederationLimit::PathItems,
        FederationLimit::Operations,
        FederationLimit::Schemas,
        FederationLimit::FieldsPerOperation,
        FederationLimit::Clients,
        FederationLimit::Declarations,
        FederationLimit::ConfirmedLinks,
        FederationLimit::Candidates,
        FederationLimit::Rejections,
        FederationLimit::EvidenceItems,
        FederationLimit::CoverageGaps,
        FederationLimit::ReportBytes,
        FederationLimit::MemoryBytes,
        FederationLimit::WallMilliseconds,
    ];
    for limit in limits {
        let mut counter = ResourceCounter::new();
        assert_eq!(counter.charge(limit, limit.maximum()), Ok(limit.maximum()));
        let failure = counter.charge(limit, 1).expect_err("maximum plus one");
        assert_eq!(failure.limit, limit);
        assert_eq!(failure.maximum, limit.maximum());
        assert_eq!(failure.observed, limit.maximum() + 1);
    }
}

#[test]
fn pt_fr_fed_001_provider_collection_orders_are_invariant() {
    let expected = federate(
        "urn:codenoesis:test:workspace".to_owned(),
        permutation_provider(false),
        Vec::new(),
    )
    .unwrap();
    for seed in 0..50 {
        let report = federate(
            "urn:codenoesis:test:workspace".to_owned(),
            permutation_provider(seed % 2 == 1),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(report, expected, "seed {seed}");
    }
}

#[test]
fn pt_fr_fed_001_conflicts_and_multi_call_sites_are_order_invariant() {
    let first = declaration("clients/base.json", "callA", "/a", "getA");
    let second = declaration("variants/conflict.json", "callA", "/b", "postB");
    let expected = FederationError::IdentityConflict {
        path: "variants/conflict.json".to_owned(),
        subject_id: client_id("urn:codenoesis:test:client"),
    };
    for declarations in [vec![first.clone(), second.clone()], vec![second, first]] {
        assert_eq!(
            federate(
                "urn:codenoesis:test:workspace".to_owned(),
                permutation_provider(false),
                declarations,
            ),
            Err(expected.clone())
        );
    }

    let report = federate(
        "urn:codenoesis:test:workspace".to_owned(),
        permutation_provider(false),
        vec![
            declaration("clients/a.json", "callA", "/a", "getA"),
            declaration("clients/b.json", "callB", "/b", "postB"),
        ],
    )
    .unwrap();
    assert_eq!(report.clients.len(), 2);
    assert_eq!(report.clients[0].client_id, report.clients[1].client_id);
    assert!(report.clients[0].call_site_id < report.clients[1].call_site_id);
}

fn declaration(
    path: &str,
    symbol: &str,
    path_template: &str,
    operation_name: &str,
) -> ClientDeclaration {
    let method = if path_template == "/a" {
        HttpMethod::Get
    } else {
        HttpMethod::Post
    };
    ClientDeclaration {
        role: symbol.to_ascii_lowercase(),
        repository_identity: "urn:codenoesis:test:client".to_owned(),
        revision: "v1".to_owned(),
        source_path: "src/client.rs".to_owned(),
        symbol_identity: symbol.to_owned(),
        binding: ClientBinding::ExplicitOperationIdentity {
            service_authority: "https://api.example.invalid".to_owned(),
            method,
            path_template: path_template.to_owned(),
            operation_id: operation_name.to_owned(),
        },
        declaration_path: path.to_owned(),
        declaration_sha256: "1".repeat(64),
    }
}

fn permutation_provider(reverse: bool) -> ProviderContract {
    let binding = ProviderBinding {
        repository_identity: "urn:codenoesis:test:provider".to_owned(),
        revision: "v1".to_owned(),
        contract_path: "provider/openapi.json".to_owned(),
        contract_sha256: "0".repeat(64),
        service_authority: "https://api.example.invalid".to_owned(),
        source_format: SourceFormat::Json,
    };
    let service = service_id(&binding.service_authority);
    let server_evidence = FederationEvidence::openapi_json(&binding, "/servers/0/url");
    let mut operations = [
        operation_fixture(
            &binding,
            &service,
            HttpMethod::Get,
            "/a",
            "getA",
            ["x", "y"],
        ),
        operation_fixture(
            &binding,
            &service,
            HttpMethod::Post,
            "/b",
            "postB",
            ["m", "n"],
        ),
    ];
    let mut evidence = vec![server_evidence.clone()];
    for operation in &operations {
        evidence.push(FederationEvidence::openapi_json(
            &binding,
            &format!(
                "/paths/{}/{}",
                operation
                    .path_template
                    .replace('~', "~0")
                    .replace('/', "~1"),
                operation.method.as_str().to_ascii_lowercase()
            ),
        ));
        evidence.push(FederationEvidence::openapi_json(
            &binding,
            &format!("/components/schemas/{}", operation.explicit_operation_id),
        ));
    }
    let mut provider_evidence_ids = evidence
        .iter()
        .map(|item| item.evidence_id.clone())
        .collect::<Vec<_>>();
    if reverse {
        operations.reverse();
        for operation in &mut operations {
            operation.fields.reverse();
            operation.evidence_ids.reverse();
        }
        evidence.reverse();
        provider_evidence_ids.reverse();
    }
    ProviderContract {
        binding,
        service_id: service,
        title: "Permutation Service".to_owned(),
        operations: operations.into_iter().collect(),
        evidence,
        evidence_ids: provider_evidence_ids,
        coverage_gaps: Vec::new(),
    }
}

fn operation_fixture(
    binding: &ProviderBinding,
    service: &str,
    method: HttpMethod,
    path: &str,
    explicit_name: &str,
    field_names: [&str; 2],
) -> ProviderOperation {
    let operation = operation_id(service, method, path, explicit_name);
    let method_name = method.as_str().to_ascii_lowercase();
    let operation_pointer = format!(
        "/paths/{}/{method_name}",
        path.replace('~', "~0").replace('/', "~1")
    );
    let operation_evidence = FederationEvidence::openapi_json(binding, &operation_pointer);
    let schema_pointer = format!("/components/schemas/{explicit_name}");
    let schema_evidence = FederationEvidence::openapi_json(binding, &schema_pointer);
    ProviderOperation {
        operation_id: operation.clone(),
        service_id: service.to_owned(),
        method,
        path_template: path.to_owned(),
        explicit_operation_id: explicit_name.to_owned(),
        response_status: "200".to_owned(),
        schema_id: schema_id(&operation, "response", "200", &format!("#{schema_pointer}")),
        fields: field_names
            .into_iter()
            .map(|name| OperationField {
                field_id: field_id(&operation, "response", "200", &format!("/{name}")),
                json_pointer: format!("/{name}"),
                required: false,
                schema_type: JsonSchemaType::String,
                evidence_ids: vec![schema_evidence.evidence_id.clone()],
            })
            .collect(),
        evidence_ids: vec![
            operation_evidence.evidence_id.clone(),
            schema_evidence.evidence_id,
        ],
        primary_evidence_id: operation_evidence.evidence_id,
    }
}
