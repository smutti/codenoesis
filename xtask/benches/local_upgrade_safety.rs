use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::json;

const REPETITIONS: usize = 30;
const BINARY_V1: &[u8] = include_bytes!(
    "../../tests/specifications/g1/local-cli-distribution-v1/fixtures/noesis-v1.bin"
);
const BINARY_V2: &[u8] = include_bytes!(
    "../../tests/specifications/g1/local-cli-distribution-v1/fixtures/noesis-v2.bin"
);

#[test]
fn nfr_per_001_observes_local_upgrade_preflight_without_threshold() {
    let target = codenoesis_contracts::current_local_distribution_target();
    if target == "unsupported-compile-target" {
        return;
    }
    let root = tempfile::tempdir().expect("temporary benchmark root");
    let current = package_fixture(root.path(), "current", BINARY_V1);
    let candidate = package_fixture(root.path(), "candidate", BINARY_V2);
    let arguments = upgrade_arguments(&current, &candidate);
    xtask::upgrade::run(arguments.clone()).expect("warm upgrade preflight");

    let mut raw_nanoseconds = Vec::with_capacity(REPETITIONS);
    let mut successes = 0_usize;
    for _ in 0..REPETITIONS {
        let started = Instant::now();
        let result = xtask::upgrade::run(arguments.clone());
        let elapsed = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        raw_nanoseconds.push(elapsed);
        if result.is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, REPETITIONS);
    let mut sorted = raw_nanoseconds.clone();
    sorted.sort_unstable();
    let observation = json!({
        "schema_version": "codenoesis.performance-observation/local-upgrade-safety/v1",
        "requirement": "NFR-PER-001",
        "issue": 184,
        "planning_item": "G7a",
        "slice": "S14",
        "authority_base_sha": "e7643d83965dca2f9342080264e7c6c58f3dd761",
        "profile_id": "local-experimental-r17",
        "platform_target": target,
        "corpus_version": "g1a-local-cli-distribution-v1/noesis-v1-to-v2",
        "host": {
            "operating_system": std::env::consts::OS,
            "architecture": std::env::consts::ARCH,
            "hostname_recorded": false
        },
        "toolchain": {"rustc": "1.97.1", "source": "rust-toolchain.toml"},
        "execution": {
            "mode": "in-process",
            "concurrency": 1,
            "cache_state": "warm-after-one-unmeasured-preflight",
            "enabled_extractors": []
        },
        "repetitions": REPETITIONS,
        "raw_nanoseconds": raw_nanoseconds,
        "percentile_method": "nearest-rank",
        "nearest_rank_nanoseconds": {
            "p50": nearest_rank(&sorted, 50),
            "p95": nearest_rank(&sorted, 95),
            "p99": nearest_rank(&sorted, 99)
        },
        "success": {"count": successes, "rate": "30/30"},
        "regression_threshold": null,
        "slo_authority": false,
        "global_benchmark_manifest_changed": false
    });
    println!(
        "{}",
        serde_json::to_string(&observation).expect("serialize observation")
    );
}

fn nearest_rank(sorted: &[u64], percentile: usize) -> u64 {
    let rank = percentile
        .checked_mul(sorted.len())
        .and_then(|value| value.checked_add(99))
        .map_or(sorted.len(), |value| value / 100);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn package_fixture(root: &Path, label: &str, bytes: &[u8]) -> PathBuf {
    let binary = root.join(format!("{label}.bin"));
    fs::write(&binary, bytes).expect("write fixture binary");
    let output = root.join(format!("{label}-output"));
    fs::create_dir(&output).expect("create package output root");
    xtask::distribution::run([
        OsString::from("xtask"),
        OsString::from("package-local-cli"),
        OsString::from("--binary"),
        binary.into_os_string(),
        OsString::from("--output"),
        output.as_os_str().to_owned(),
    ])
    .expect("package benchmark fixture");
    let entries = fs::read_dir(&output)
        .expect("read output")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect output entries");
    assert_eq!(entries.len(), 1);
    entries[0].path()
}

fn upgrade_arguments(current: &Path, candidate: &Path) -> Vec<OsString> {
    vec![
        OsString::from("xtask"),
        OsString::from("preflight-local-upgrade"),
        OsString::from("--current"),
        current.as_os_str().to_owned(),
        OsString::from("--candidate"),
        candidate.as_os_str().to_owned(),
    ]
}
