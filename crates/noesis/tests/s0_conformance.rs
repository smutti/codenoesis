mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;
use std::process::Command;
#[cfg(target_os = "linux")]
use std::process::{Output, Stdio};
use std::thread;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use support::{
    BLOB_A_OID, BLOB_B_OID, COMMIT_A_OID, MaterializedRepository, parse_single_document, read_json,
    repository_root, scan, unique_temp_root,
};

#[test]
fn conf_nfr_mnt_001_dependency_rules() {
    let root = repository_root();
    let output = Command::new(env!("CARGO"))
        .current_dir(&root)
        .args(["metadata", "--locked", "--no-deps", "--format-version", "1"])
        .output()
        .expect("run Cargo metadata for architecture fitness");
    assert!(
        output.status.success(),
        "Cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("parse Cargo metadata");
    let first_party = BTreeSet::from([
        "codenoesis-application",
        "codenoesis-contracts",
        "codenoesis-domain",
        "codenoesis-ports",
        "codenoesis-repository",
        "noesis",
        "xtask",
    ]);
    let expected = BTreeMap::from([
        (
            "codenoesis-application",
            BTreeSet::from([
                "codenoesis-contracts",
                "codenoesis-domain",
                "codenoesis-ports",
            ]),
        ),
        (
            "codenoesis-contracts",
            BTreeSet::from(["codenoesis-domain"]),
        ),
        ("codenoesis-domain", BTreeSet::new()),
        ("codenoesis-ports", BTreeSet::from(["codenoesis-domain"])),
        (
            "codenoesis-repository",
            BTreeSet::from(["codenoesis-domain", "codenoesis-ports"]),
        ),
        (
            "noesis",
            BTreeSet::from([
                "codenoesis-application",
                "codenoesis-contracts",
                "codenoesis-domain",
                "codenoesis-ports",
                "codenoesis-repository",
            ]),
        ),
        ("xtask", BTreeSet::new()),
    ]);
    let packages = metadata["packages"]
        .as_array()
        .expect("metadata packages array");
    let mut actual = BTreeMap::new();
    for package in packages {
        let name = package["name"].as_str().expect("package name");
        if !first_party.contains(name) {
            continue;
        }
        let dependencies = package["dependencies"]
            .as_array()
            .expect("package dependencies")
            .iter()
            .filter_map(|dependency| dependency["name"].as_str())
            .filter(|dependency| first_party.contains(dependency))
            .collect::<BTreeSet<_>>();
        actual.insert(name, dependencies);

        let manifest_path = package["manifest_path"]
            .as_str()
            .expect("package manifest path");
        let manifest = fs::read_to_string(manifest_path).expect("read first-party manifest");
        assert!(manifest.contains("[lints]\nworkspace = true"));
    }
    assert!(architecture_is_valid(&actual, &expected));

    let mut seeded_forbidden_edge = actual;
    seeded_forbidden_edge
        .get_mut("codenoesis-repository")
        .expect("repository dependency set")
        .insert("codenoesis-contracts");
    assert!(!architecture_is_valid(&seeded_forbidden_edge, &expected));

    let workspace = fs::read_to_string(root.join("Cargo.toml")).expect("read workspace manifest");
    assert!(workspace.contains("unsafe_code = \"forbid\""));
}

#[test]
fn conf_nfr_tst_001_requires_fixture_oracle_evidence_links() {
    let root = repository_root();
    let specification =
        read_json(&root.join("tests/specifications/s0/e2e_fr_acq_001_immutable_commit.json"));
    let bundle = read_json(&root.join("tests/specifications/s0/contract-bundle.json"));
    let observation = read_json(
        &root.join("crates/noesis/tests/evidence/s0/red-observation-corrected-contract.json"),
    );
    let green =
        read_json(&root.join("crates/noesis/tests/evidence/s0/green-observation-local.json"));

    assert_eq!(observation["slice"], specification["slice"]);
    assert_eq!(observation["requirements"], specification["requirements"]);
    assert_eq!(observation["oracle_bundle_sha256"], bundle["bundle_sha256"]);
    assert_eq!(observation["status"], "red_observed");
    assert_eq!(observation["red"]["test_id"], specification["test_id"]);
    assert_eq!(observation["red"]["runner_exit_code"], 101);
    assert_eq!(observation["red"]["subject_exit_code"], 70);
    assert_eq!(
        observation["pre_implementation_sha"],
        "962f070adf7bdb682b0636e91153ed1177aec8b8"
    );
    assert_eq!(green["requirements"], specification["requirements"]);
    assert_eq!(green["oracle_bundle_sha256"], bundle["bundle_sha256"]);
    assert_eq!(green["verified"], false);
    assert_eq!(
        green["implementation_sha"],
        "802b90d2bb0c69aa81dfc894276294ba4c64ab32"
    );
    assert_eq!(
        green["ordered_results"]
            .as_array()
            .expect("ordered Green results")
            .iter()
            .map(|result| result["test_name"].as_str().expect("Green test name"))
            .collect::<Vec<_>>(),
        specification["scenarios"]
            .as_array()
            .expect("ordered specification scenarios")
            .iter()
            .map(|scenario| scenario["test_name"].as_str().expect("scenario test name"))
            .collect::<Vec<_>>()
    );

    for relative_path in [
        specification["fixture"].as_str().expect("fixture path"),
        specification["decision"].as_str().expect("decision path"),
        specification["schemas"]["snapshot"]
            .as_str()
            .expect("snapshot schema path"),
        specification["schemas"]["error"]
            .as_str()
            .expect("error schema path"),
        observation["evidence_manifest_schema"]
            .as_str()
            .expect("evidence schema path"),
        observation["supersedes_local_observation"]
            .as_str()
            .expect("superseded observation path"),
        "crates/noesis/tests/evidence/s0/green-observation-local.json",
    ] {
        assert!(
            root.join(relative_path).is_file(),
            "missing {relative_path}"
        );
    }

    let log_path = observation["red"]["log"]["path"]
        .as_str()
        .expect("Red log path");
    let log = fs::read(root.join(log_path)).expect("read immutable Red log");
    let log_sha256 = format!("{:x}", Sha256::digest(log));
    assert_eq!(
        observation["red"]["log"]["sha256"]
            .as_str()
            .expect("Red log digest"),
        log_sha256
    );
}

#[test]
fn pt_nfr_tst_002_replays_are_parallel_and_order_independent() {
    let mut baseline = BTreeMap::new();
    for seed in 0..10 {
        let mut cases = vec![
            ReplayCase::Success,
            ReplayCase::NonGit,
            ReplayCase::MissingObject,
            ReplayCase::InconsistentObject,
        ];
        let length = cases.len();
        cases.rotate_left(seed % length);
        if seed % 2 == 1 {
            cases.reverse();
        }
        let recorded_order = cases
            .iter()
            .copied()
            .map(ReplayCase::name)
            .collect::<Vec<_>>();
        let handles = cases
            .into_iter()
            .map(|case| thread::spawn(move || replay(case)))
            .collect::<Vec<_>>();

        for handle in handles {
            let (name, stable_result) = handle.join().unwrap_or_else(|panic| {
                panic!(
                    "parallel replay failed for seed {seed}, order {recorded_order:?}: {panic:?}"
                )
            });
            if let Some(expected) = baseline.get(name) {
                assert_eq!(
                    &stable_result, expected,
                    "replay changed for {name}, seed {seed}, order {recorded_order:?}"
                );
            } else {
                baseline.insert(name, stable_result);
            }
        }
    }
    assert_eq!(baseline.len(), 4);
}

#[test]
fn sec_nfr_sec_005_scan_launches_no_child_and_opens_no_network() {
    let repository = MaterializedRepository::commit_a();
    let sentinel = repository.root.join("target-controlled-hook-executed");
    repository.apply_isolation_variant(&sentinel);

    #[cfg(target_os = "linux")]
    let output = monitored_linux_scan(&repository, &sentinel);
    #[cfg(not(target_os = "linux"))]
    let output = support::scan_command(&repository.worktree, COMMIT_A_OID)
        .env("CODENOESIS_SENTINEL", &sentinel)
        .output()
        .expect("run portable S0 isolation smoke test");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let snapshot = parse_single_document(&output.stdout);
    assert_eq!(
        snapshot["semantic_hash"]["value"],
        "b673624a329f43fd84852bbdeefd66326a7fcb1c03fdb626e2de6bfedff11997"
    );
    assert!(!sentinel.exists(), "target-controlled hook executed");

    let policy =
        fs::read(repository_root().join("tests/specifications/s0/seccomp-capability-deny-v1.json"))
            .expect("read ratified seccomp policy");
    assert_eq!(
        format!("{:x}", Sha256::digest(policy)),
        "5664635f8ad76dff5421f5eeb1f20ffdf0450203d8cb9c692606c026a39ee1ad"
    );
}

#[cfg(target_os = "linux")]
fn monitored_linux_scan(repository: &MaterializedRepository, sentinel: &Path) -> Output {
    let trace = repository.root.join("s0-strace.log");
    let links = repository.root.join("s0-network-links.json");
    let routes = repository.root.join("s0-network-routes.json");
    let descriptors = repository.root.join("s0-inherited-descriptors.txt");
    let unprivileged = Command::new("unshare")
        .args(["--user", "--map-root-user", "--net", "--", "true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());

    let mut command = if unprivileged {
        let mut command = Command::new("unshare");
        command.args(["--user", "--map-root-user", "--net", "--"]);
        command
    } else {
        let sudo = Command::new("sudo")
            .args(["-n", "true"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("check passwordless sudo for Linux namespace evidence");
        assert!(
            sudo.success(),
            "neither unprivileged user namespaces nor passwordless sudo are available"
        );
        let mut command = Command::new("sudo");
        command.args(["-n", "unshare", "--net", "--"]);
        command
    };

    let monitor = r#"
set -eu
ip -json link show >"$1"
ip -json route show table all >"$2"
unexpected=""
for descriptor in /proc/$$/fd/*; do
  number=${descriptor##*/}
  case "$number" in
    0|1|2) ;;
    *) unexpected="$unexpected $number" ;;
  esac
done
test -z "$unexpected"
for number in 0 1 2; do
  target=$(readlink "/proc/$$/fd/$number")
  case "$target" in
    socket:*) exit 92 ;;
  esac
done
printf '0\n1\n2\n' >"$3"
export CODENOESIS_SENTINEL="$4"
shift 4
exec "$@"
"#;
    command
        .args(["sh", "-c", monitor, "s0-monitor"])
        .arg(&links)
        .arg(&routes)
        .arg(&descriptors)
        .arg(sentinel)
        .args([
            "strace",
            "-f",
            "-qq",
            "-s",
            "256",
            "-e",
            "trace=%process,%network,io_uring_setup,io_uring_enter,io_uring_register,prctl,seccomp",
            "-o",
        ])
        .arg(&trace)
        .arg(env!("CARGO_BIN_EXE_noesis"))
        .args(["scan", "--repository"])
        .arg(&repository.worktree)
        .args([
            "--repository-id",
            support::REPOSITORY_ID,
            "--revision",
            COMMIT_A_OID,
            "--format",
            "json",
        ]);
    let output = command
        .output()
        .expect("run monitored scan in an empty Linux network namespace");
    assert!(
        output.status.success(),
        "Linux monitor failed: stdout={:?}; stderr={:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert_linux_monitor_evidence(&links, &routes, &descriptors, &trace);
    output
}

#[cfg(target_os = "linux")]
fn assert_linux_monitor_evidence(links: &Path, routes: &Path, descriptors: &Path, trace: &Path) {
    let link_inventory = read_json(links);
    assert!(
        link_inventory
            .as_array()
            .expect("network link inventory")
            .iter()
            .all(|link| !link["flags"]
                .as_array()
                .is_some_and(|flags| flags.iter().any(|flag| flag == "UP")))
    );
    assert_eq!(
        read_json(routes)
            .as_array()
            .expect("network route inventory")
            .len(),
        0
    );
    assert_eq!(
        fs::read_to_string(descriptors).expect("read inherited descriptor audit"),
        "0\n1\n2\n"
    );

    let trace = fs::read_to_string(trace).expect("read syscall audit");
    assert!(trace.contains("PR_SET_NO_NEW_PRIVS"));
    assert!(trace.contains("SECCOMP_SET_MODE_FILTER"));
    assert_eq!(trace.matches("execve(").count(), 1, "{trace}");
    for syscall in [
        "execveat",
        "fork",
        "vfork",
        "clone",
        "clone3",
        "socket",
        "socketpair",
        "connect",
        "bind",
        "listen",
        "accept",
        "accept4",
        "sendto",
        "sendmsg",
        "sendmmsg",
        "recvfrom",
        "recvmsg",
        "recvmmsg",
        "shutdown",
        "io_uring_setup",
        "io_uring_enter",
        "io_uring_register",
    ] {
        assert_eq!(
            trace.matches(&format!("{syscall}(")).count(),
            0,
            "forbidden syscall {syscall} observed: {trace}"
        );
    }
}

fn architecture_is_valid<'a>(
    actual: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    expected: &BTreeMap<&'a str, BTreeSet<&'a str>>,
) -> bool {
    actual == expected
}

#[derive(Clone, Copy, Debug)]
enum ReplayCase {
    Success,
    NonGit,
    MissingObject,
    InconsistentObject,
}

impl ReplayCase {
    const fn name(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::NonGit => "non_git",
            Self::MissingObject => "missing_object",
            Self::InconsistentObject => "inconsistent_object",
        }
    }
}

fn replay(case: ReplayCase) -> (&'static str, Value) {
    match case {
        ReplayCase::Success => {
            let repository = MaterializedRepository::commit_a();
            let output = scan(&repository.worktree, COMMIT_A_OID);
            assert_eq!(output.status.code(), Some(0));
            assert!(output.stderr.is_empty());
            let snapshot = parse_single_document(&output.stdout);
            (
                case.name(),
                json!({
                    "semantic": snapshot["semantic"],
                    "semantic_hash": snapshot["semantic_hash"]
                }),
            )
        }
        ReplayCase::NonGit => {
            let root = unique_temp_root();
            let plain_directory = root.join("plain-directory");
            fs::create_dir(&plain_directory).expect("create replay plain directory");
            let output = scan(&plain_directory, COMMIT_A_OID);
            let error = parse_single_document(&output.stderr);
            assert_eq!(output.status.code(), Some(10));
            assert!(output.stdout.is_empty());
            fs::remove_dir_all(root).expect("remove replay plain directory");
            (case.name(), error)
        }
        ReplayCase::MissingObject => {
            let repository = MaterializedRepository::commit_a();
            fs::remove_file(repository.object_path(BLOB_A_OID)).expect("remove replay blob object");
            let output = scan(&repository.worktree, COMMIT_A_OID);
            assert_eq!(output.status.code(), Some(10));
            assert!(output.stdout.is_empty());
            (case.name(), parse_single_document(&output.stderr))
        }
        ReplayCase::InconsistentObject => {
            let repository = MaterializedRepository::commit_a();
            fs::remove_file(repository.object_path(BLOB_A_OID))
                .expect("remove replay blob before corruption");
            fs::copy(
                repository.object_path(BLOB_B_OID),
                repository.object_path(BLOB_A_OID),
            )
            .expect("install replay inconsistent blob");
            let output = scan(&repository.worktree, COMMIT_A_OID);
            assert_eq!(output.status.code(), Some(10));
            assert!(output.stdout.is_empty());
            (case.name(), parse_single_document(&output.stderr))
        }
    }
}
