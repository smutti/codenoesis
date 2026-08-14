use codenoesis_contracts::{
    CodeNoesisErrorV26, DEFAULT_LOCAL_CLI_CONFIGURATION_V1, DistributionFileMode,
    LocalConfigurationError, LocalConfigurationSource, LocalDistributionFileV1,
    LocalDistributionManifestV1, MAX_LOCAL_CONFIGURATION_BYTES, current_local_distribution_target,
    local_configuration_report_v1, validate_local_cli_configuration_v1,
};

const EXPECTED_EMBEDDED: &[u8] = include_bytes!(
    "../../../tests/specifications/g1/local-cli-distribution-v1/expected-config-embedded.json"
);

#[test]
fn fr_cfg_001_emits_the_frozen_embedded_report() {
    let report = local_configuration_report_v1(
        DEFAULT_LOCAL_CLI_CONFIGURATION_V1,
        LocalConfigurationSource::EmbeddedDefault,
    )
    .expect("embedded configuration is valid");
    assert_eq!(report.canonical_stdout().unwrap(), EXPECTED_EMBEDDED);
}

#[test]
fn fr_cfg_001_accepts_semantic_whitespace_but_rejects_duplicate_keys() {
    let value = validate_local_cli_configuration_v1(DEFAULT_LOCAL_CLI_CONFIGURATION_V1)
        .expect("default configuration is valid");
    let pretty = serde_json::to_vec_pretty(&value).unwrap();
    assert_eq!(validate_local_cli_configuration_v1(&pretty).unwrap(), value);

    let duplicate = br#"{
        "schema_version":"codenoesis.configuration/local-cli/v1",
        "schema_version":"codenoesis.configuration/local-cli/v1"
    }"#;
    assert_eq!(
        validate_local_cli_configuration_v1(duplicate),
        Err(LocalConfigurationError::InvalidFile)
    );

    let escaped_duplicate = br#"{
        "schema_version":"codenoesis.configuration/local-cli/v1",
        "schema\u005fversion":"codenoesis.configuration/local-cli/v1"
    }"#;
    assert_eq!(
        validate_local_cli_configuration_v1(escaped_duplicate),
        Err(LocalConfigurationError::InvalidFile)
    );

    let value_named_like_a_key = DEFAULT_LOCAL_CLI_CONFIGURATION_V1
        .windows(b"\"network\":\"disabled\"".len())
        .position(|candidate| candidate == b"\"network\":\"disabled\"")
        .expect("network policy exists");
    let mut unsupported = DEFAULT_LOCAL_CLI_CONFIGURATION_V1.to_vec();
    unsupported.splice(
        value_named_like_a_key..value_named_like_a_key + b"\"network\":\"disabled\"".len(),
        b"\"network\":\"schema_version\"".iter().copied(),
    );
    assert_eq!(
        validate_local_cli_configuration_v1(&unsupported),
        Err(LocalConfigurationError::UnsupportedValue)
    );
}

#[test]
fn fr_cfg_001_enforces_closed_values_and_exact_size_boundaries() {
    let value = validate_local_cli_configuration_v1(DEFAULT_LOCAL_CLI_CONFIGURATION_V1)
        .expect("default configuration is valid");

    let mut unknown = value.clone();
    unknown
        .as_object_mut()
        .expect("configuration object")
        .insert("unknown".to_owned(), serde_json::Value::Bool(true));
    assert_eq!(
        validate_local_cli_configuration_v1(&serde_json::to_vec(&unknown).unwrap()),
        Err(LocalConfigurationError::UnsupportedValue)
    );

    let mut schema = value.clone();
    schema["schema_version"] =
        serde_json::Value::String("codenoesis.configuration/local-cli/v2".to_owned());
    assert_eq!(
        validate_local_cli_configuration_v1(&serde_json::to_vec(&schema).unwrap()),
        Err(LocalConfigurationError::UnsupportedSchema)
    );

    let mut secret = value;
    secret["secrets"]["references"] = serde_json::json!(["CODENOESIS_G1_PRIVATE_CANARY"]);
    assert_eq!(
        validate_local_cli_configuration_v1(&serde_json::to_vec(&secret).unwrap()),
        Err(LocalConfigurationError::UnsupportedValue)
    );

    let mut maximum = DEFAULT_LOCAL_CLI_CONFIGURATION_V1.to_vec();
    maximum.resize(MAX_LOCAL_CONFIGURATION_BYTES, b' ');
    validate_local_cli_configuration_v1(&maximum).expect("maximum input is valid");
    maximum.push(b' ');
    assert_eq!(
        validate_local_cli_configuration_v1(&maximum),
        Err(LocalConfigurationError::InvalidFile)
    );
}

#[test]
fn fr_cfg_001_errors_are_strict_and_private() {
    for error in [
        CodeNoesisErrorV26::configuration_invalid_arguments(),
        CodeNoesisErrorV26::configuration_invalid_file(),
        CodeNoesisErrorV26::configuration_unstable_input(),
        CodeNoesisErrorV26::from_configuration(LocalConfigurationError::UnsupportedSchema),
        CodeNoesisErrorV26::from_configuration(LocalConfigurationError::UnsupportedValue),
    ] {
        let stderr = error.canonical_stderr().expect("canonical error");
        assert!(stderr.ends_with(b"\n"));
        assert!(!stderr.windows(2).any(|window| window == b"\r\n"));
        let text = String::from_utf8(stderr).expect("UTF-8 error");
        for canary in [
            "CODENOESIS_G1_PRIVATE_CANARY",
            "/private/absolute/root",
            "https://private.invalid",
        ] {
            assert!(!text.contains(canary));
        }
    }
}

#[test]
fn fr_rel_002_builds_the_frozen_current_target_manifest() {
    let target = current_local_distribution_target();
    if target == "unsupported-compile-target" {
        return;
    }
    let binary_path = if target == "x86_64-pc-windows-msvc" {
        "bin/noesis.exe"
    } else {
        "bin/noesis"
    };
    let files = vec![
        LocalDistributionFileV1::new(
            binary_path,
            43,
            "f82e988be32ec8a3f077e49f4034d42847adc415cd05ec82a9affec2fe25fb6b",
            DistributionFileMode::Executable,
        ),
        LocalDistributionFileV1::new(
            "etc/codenoesis/config.json",
            301,
            "a923dcf8937410ed942f6c6f3ec7899f9a2fcccc52b91653bf8aaa3df6e4e327",
            DistributionFileMode::Data,
        ),
        LocalDistributionFileV1::new(
            "share/codenoesis/schemas/local-cli-config-v1.schema.json",
            1_443,
            "e9a5b92168e2163533d20c974e6472fdda8fc43399cad7329d82d2d3eefc30c4",
            DistributionFileMode::Data,
        ),
        LocalDistributionFileV1::new(
            "share/doc/codenoesis/INSTALL.md",
            945,
            "7faf884c53a5c2595850636823227076ef7f71456a3856fef648e705053ee46c",
            DistributionFileMode::Data,
        ),
        LocalDistributionFileV1::new(
            "share/doc/codenoesis/LICENSE",
            11_357,
            "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4",
            DistributionFileMode::Data,
        ),
    ];
    let manifest = LocalDistributionManifestV1::new(
        target,
        "f82e988be32ec8a3f077e49f4034d42847adc415cd05ec82a9affec2fe25fb6b",
        &files,
    )
    .expect("current target manifest is valid");
    let expected = match target {
        "x86_64-unknown-linux-gnu" => include_bytes!(
            "../../../tests/specifications/g1/local-cli-distribution-v1/expected-manifest-x86_64-unknown-linux-gnu.json"
        )
        .as_slice(),
        "aarch64-apple-darwin" => include_bytes!(
            "../../../tests/specifications/g1/local-cli-distribution-v1/expected-manifest-aarch64-apple-darwin.json"
        )
        .as_slice(),
        "x86_64-pc-windows-msvc" => include_bytes!(
            "../../../tests/specifications/g1/local-cli-distribution-v1/expected-manifest-x86_64-pc-windows-msvc.json"
        )
        .as_slice(),
        _ => unreachable!(),
    };
    assert_eq!(manifest.canonical_stdout().unwrap(), expected);
}

#[test]
fn fr_rel_002_rejects_unbounded_or_noncanonical_manifest_inputs() {
    let target = current_local_distribution_target();
    if target == "unsupported-compile-target" {
        return;
    }
    let binary_path = if target == "x86_64-pc-windows-msvc" {
        "bin/noesis.exe"
    } else {
        "bin/noesis"
    };
    let frozen = |binary_length, binary_digest: &str| {
        vec![
            LocalDistributionFileV1::new(
                binary_path,
                binary_length,
                binary_digest,
                DistributionFileMode::Executable,
            ),
            LocalDistributionFileV1::new(
                "etc/codenoesis/config.json",
                301,
                "a923dcf8937410ed942f6c6f3ec7899f9a2fcccc52b91653bf8aaa3df6e4e327",
                DistributionFileMode::Data,
            ),
            LocalDistributionFileV1::new(
                "share/codenoesis/schemas/local-cli-config-v1.schema.json",
                1_443,
                "e9a5b92168e2163533d20c974e6472fdda8fc43399cad7329d82d2d3eefc30c4",
                DistributionFileMode::Data,
            ),
            LocalDistributionFileV1::new(
                "share/doc/codenoesis/INSTALL.md",
                945,
                "7faf884c53a5c2595850636823227076ef7f71456a3856fef648e705053ee46c",
                DistributionFileMode::Data,
            ),
            LocalDistributionFileV1::new(
                "share/doc/codenoesis/LICENSE",
                11_357,
                "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4",
                DistributionFileMode::Data,
            ),
        ]
    };
    let digest = "f82e988be32ec8a3f077e49f4034d42847adc415cd05ec82a9affec2fe25fb6b";
    assert!(LocalDistributionManifestV1::new(target, digest, &frozen(43, digest)).is_ok());
    assert!(
        LocalDistributionManifestV1::new(target, digest, &frozen(268_435_457, digest)).is_err()
    );
    assert!(
        LocalDistributionManifestV1::new(
            target,
            digest,
            &frozen(
                43,
                "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
            )
        )
        .is_err()
    );
    assert!(
        LocalDistributionManifestV1::new("runtime-target", digest, &frozen(43, digest)).is_err()
    );
}
