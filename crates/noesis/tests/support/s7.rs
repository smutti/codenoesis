use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::{repository_root, unique_temp_root};

const FEDERATION_REPORT_SHA256: &str =
    "7a301b0eb5d0e5e702a647422e57f42e1611d9e1cac5aedb39be26c7f1064628";

pub struct MaterializedImpactWorkspace {
    root: PathBuf,
    pub manifest: PathBuf,
}

impl MaterializedImpactWorkspace {
    pub fn reviewed() -> Self {
        let root = unique_temp_root();
        let fixture = repository_root().join("tests/fixtures/s7/implementation-aware-api-v1");
        for relative in [
            "provider/revision-a/openapi.yaml",
            "provider/revision-a/src/user_response.rs",
            "provider/revision-b/openapi.yaml",
            "provider/revision-b/src/user_response.rs",
            "clients/decoy/src/commonMain/kotlin/dev/codenoesis/fixture/DecoyAccountClient.kt",
            "clients/safe/src/commonMain/kotlin/dev/codenoesis/fixture/SafeUsersClient.kt",
            "clients/strict/src/commonMain/kotlin/dev/codenoesis/fixture/StrictUsersClient.kt",
        ] {
            copy_file(&fixture.join(relative), &root.join(relative));
        }
        copy_file(
            &repository_root()
                .join("tests/fixtures/s6/openapi-federation-v1/expected-federation-report.json"),
            &root.join("federation-report.json"),
        );

        let manifest = root.join("impact-workspace.json");
        let value = json!({
            "schema_version": "codenoesis.impact-workspace/v1",
            "analysis_profile": "implementation-aware-http-json/v1",
            "pipeline": "codenoesis.pipeline/s7-v1",
            "contract_capability": "codenoesis.contract-capability/openapi-3.1-http-json/v1",
            "provider_capability": "rust-direct-json-map/v1",
            "client_capability": "kotlin-direct-json-access/v1",
            "provider": {
                "repository_identity": "urn:codenoesis:fixture:s7-provider",
                "baseline": {
                    "revision": "fixture-provider-a",
                    "root": "provider/revision-a",
                    "contract": {
                        "path": "openapi.yaml",
                        "sha256": "d6decc18d428316b209aa554ee028fe9db8761df515bf34b9e92c3a369f2de3d"
                    },
                    "source": {
                        "path": "src/user_response.rs",
                        "sha256": "cec091bf3c88ab912fb824fa38c0789795d7f460b808a06b36f463f69ebc3413"
                    },
                    "callable_symbol": "user_response"
                },
                "target": {
                    "revision": "fixture-provider-b",
                    "root": "provider/revision-b",
                    "contract": {
                        "path": "openapi.yaml",
                        "sha256": "d6decc18d428316b209aa554ee028fe9db8761df515bf34b9e92c3a369f2de3d"
                    },
                    "source": {
                        "path": "src/user_response.rs",
                        "sha256": "f92348e5cd2270c575bef29379def98adba4fa09577726e3f259f4f2559b6632"
                    },
                    "callable_symbol": "user_response"
                }
            },
            "clients": [
                client(
                    "decoy",
                    "urn:codenoesis:fixture:s7-client-decoy",
                    "clients/decoy",
                    "src/commonMain/kotlin/dev/codenoesis/fixture/DecoyAccountClient.kt",
                    "dc9436422660a090372a81763702ce4dd719390d9f8ae5d1bdfb9f642cb4fbb7",
                    "decodeAccount",
                    "getAccount"
                ),
                client(
                    "safe",
                    "urn:codenoesis:fixture:s7-client-safe",
                    "clients/safe",
                    "src/commonMain/kotlin/dev/codenoesis/fixture/SafeUsersClient.kt",
                    "bbb4e2a92b01c230d21c5bba7c8b2ec6ac04da0a3bc884caa7d32d09a7ce083c",
                    "decodeSafeUser",
                    "getSafeUser"
                ),
                client(
                    "strict",
                    "urn:codenoesis:fixture:s7-client-strict",
                    "clients/strict",
                    "src/commonMain/kotlin/dev/codenoesis/fixture/StrictUsersClient.kt",
                    "d44a6b6485fe20ff4ea4d1146bd09a4439aa8d6a495320c4b549859943624920",
                    "decodeStrictUser",
                    "getStrictUser"
                )
            ],
            "federation_report": {
                "path": "federation-report.json",
                "sha256": FEDERATION_REPORT_SHA256
            }
        });
        fs::write(
            &manifest,
            serde_json::to_vec(&value).expect("serialize S7 workspace"),
        )
        .expect("write S7 workspace");

        Self { root, manifest }
    }
}

impl Drop for MaterializedImpactWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn reviewed_golden() -> Vec<u8> {
    fs::read(repository_root().join(
        "tests/fixtures/s7/implementation-aware-api-v1/expected-semantic-compatibility-report.json",
    ))
    .expect("read reviewed S7 golden")
}

fn client(
    role: &str,
    repository_identity: &str,
    root: &str,
    path: &str,
    sha256: &str,
    decoder_symbol: &str,
    call_symbol: &str,
) -> serde_json::Value {
    json!({
        "role": role,
        "repository_identity": repository_identity,
        "revision": "fixture-client-v1",
        "root": root,
        "source": {
            "path": path,
            "sha256": sha256
        },
        "decoder_symbol": decoder_symbol,
        "call_symbol": call_symbol
    })
}

fn copy_file(source: &Path, target: &Path) {
    fs::create_dir_all(target.parent().expect("S7 target parent"))
        .expect("create S7 target directory");
    fs::copy(source, target).unwrap_or_else(|error| {
        panic!(
            "copy reviewed S7 input {} to {}: {error}",
            source.display(),
            target.display()
        )
    });
}
