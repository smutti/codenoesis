mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

use codenoesis_domain::s1_packed::PACKED_LIMIT_KINDS;
use codenoesis_domain::{AcquisitionError, LimitKind, check_limit};
use support::s1::{COMMIT_A_OID, MaterializedRepository, fixture_root};
use support::s1_packed::{
    DeltaEncoding, ExternalBaseStorage, PackedMutation, generate_delta_candidates,
    materialize_base_only_pack, materialize_base_only_pack_at, materialize_delta_pack,
    materialize_duplicate_object_pack, offline_verify_pack, retain_revision, scan_packed,
    scan_packed_command,
};

#[test]
fn e2e_fr_acq_004_packed_sha1_equivalence() {
    let repository = MaterializedRepository::revision_a();
    let packed = materialize_base_only_pack(&repository);
    let sentinel = repository.root.join("outside-sentinel");
    let sentinel_bytes = b"must-not-be-read-or-changed\n";
    fs::write(&sentinel, sentinel_bytes).expect("write packed acquisition sentinel");
    repository.apply_isolation_variant(&sentinel);

    let output = scan_packed_command(&repository.worktree, COMMIT_A_OID)
        .env("CODENOESIS_SENTINEL", &sentinel)
        .output()
        .expect("launch packed acquisition subject");

    packed.assert_unchanged();
    assert_eq!(
        fs::read(&sentinel).expect("read packed acquisition sentinel after subject"),
        sentinel_bytes,
        "packed acquisition executed a hook or changed the outside sentinel"
    );
    assert!(
        output.status.success(),
        "expected selected packed S1 scan success; status={:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let snapshot: Value =
        serde_json::from_slice(&output.stdout).expect("parse packed RepositorySnapshotV2");
    let mut expected_semantic =
        fs::read(fixture_root().join("expected-semantic-a.jcs")).expect("read S1 semantic golden");
    assert_eq!(expected_semantic.pop(), Some(b'\n'));
    assert_eq!(
        serde_json::to_vec(&snapshot["semantic"]).expect("serialize packed semantic value"),
        expected_semantic
    );
    assert_eq!(
        snapshot["semantic_hash"],
        json!({
            "algorithm": "blake3-256",
            "value": "236b231c3154f9be56130ddc8dfb39bb482af10330f7c6757597ad22c006e9e7"
        })
    );
    assert_eq!(
        snapshot["schema_version"],
        "codenoesis.repository-snapshot/v2"
    );
}

#[test]
fn conf_fr_acq_004_ofs_delta() {
    assert_delta_equivalence(DeltaEncoding::Ofs);
}

#[test]
fn conf_fr_acq_004_ref_delta() {
    assert_delta_equivalence(DeltaEncoding::Ref);
    assert_external_ref_equivalence(ExternalBaseStorage::Loose);
    assert_external_ref_equivalence(ExternalBaseStorage::Packed);
}

#[test]
fn conf_fr_acq_004_index_v2() {
    for (mutation, component, reason, object_context) in [
        (PackedMutation::IndexLayout, "index", "index_layout", false),
        (PackedMutation::IndexFanout, "index", "index_fanout", false),
        (
            PackedMutation::IndexObjectOrder,
            "index",
            "index_object_order",
            true,
        ),
        (PackedMutation::IndexOffset, "index", "index_offset", true),
        (
            PackedMutation::IndexChecksum,
            "index",
            "index_checksum",
            false,
        ),
    ] {
        assert_mutation_error(mutation, component, reason, object_context);
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn conf_fr_acq_004_pack_catalog_v1() {
    let sidecar_repository = MaterializedRepository::revision_a();
    let sidecar_pack = materialize_base_only_pack(&sidecar_repository);
    let pack_directory = sidecar_pack.pack_path.parent().expect("pack directory");
    for extension in ["bitmap", "keep", "mtimes", "rev"] {
        let path = pack_directory.join(format!("pack-{}.{}", sidecar_pack.pack_id, extension));
        if !path.exists() {
            fs::write(path, []).expect("write accepted sidecar");
        }
    }
    fs::write(
        pack_directory.join("multi-pack-index"),
        b"ignored paired MIDX",
    )
    .expect("write paired MIDX");
    let output = scan_packed(&sidecar_repository.worktree, COMMIT_A_OID);
    assert!(
        output.status.success(),
        "accepted sidecars changed packed acquisition: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let unpaired_repository = MaterializedRepository::revision_a();
    let unpaired = materialize_base_only_pack(&unpaired_repository);
    fs::remove_file(&unpaired.index_path).expect("remove paired index");
    assert_error_context(
        scan_packed(&unpaired_repository.worktree, COMMIT_A_OID),
        "acquisition.object_database_changed",
        json!({"component": "catalog"}),
        true,
    );

    let unsafe_repository = MaterializedRepository::revision_a();
    let unsafe_pack = materialize_base_only_pack(&unsafe_repository);
    fs::write(
        unsafe_pack
            .pack_path
            .parent()
            .expect("unsafe pack directory")
            .join(format!("pack-{}.unknown", unsafe_pack.pack_id)),
        [],
    )
    .expect("write unknown pack entry");
    assert_error_context(
        scan_packed(&unsafe_repository.worktree, COMMIT_A_OID),
        "acquisition.object_database_invalid",
        json!({"component": "catalog", "reason": "catalog_entry"}),
        false,
    );

    let promisor_repository = MaterializedRepository::revision_a();
    let promisor = materialize_base_only_pack(&promisor_repository);
    fs::write(
        promisor
            .pack_path
            .parent()
            .expect("promisor pack directory")
            .join(format!("pack-{}.promisor", promisor.pack_id)),
        [],
    )
    .expect("write promisor marker");
    assert_error_context(
        scan_packed(&promisor_repository.worktree, COMMIT_A_OID),
        "acquisition.unsupported_repository_shape",
        json!({"feature": "promisor_object_database"}),
        false,
    );

    let midx_repository = MaterializedRepository::revision_a();
    let midx = materialize_base_only_pack(&midx_repository);
    let midx_directory = midx.pack_path.parent().expect("MIDX pack directory");
    fs::remove_file(&midx.pack_path).expect("remove MIDX-only pack");
    fs::remove_file(&midx.index_path).expect("remove MIDX-only index");
    fs::write(midx_directory.join("multi-pack-index"), b"MIDX").expect("write MIDX-only marker");
    assert_error_context(
        scan_packed(&midx_repository.worktree, COMMIT_A_OID),
        "acquisition.unsupported_repository_shape",
        json!({"feature": "multi_pack_index_only"}),
        false,
    );

    for arguments in [
        vec![
            "scan",
            "--repository",
            midx_repository
                .worktree
                .to_str()
                .expect("fixture path is UTF-8"),
            "--repository-id",
            support::s1::REPOSITORY_ID,
            "--revision",
            COMMIT_A_OID,
            "--profile",
            "standard-local-s1",
            "--acquisition-profile",
            "invalid",
            "--format",
            "json",
        ],
        vec![
            "scan",
            "--repository",
            midx_repository
                .worktree
                .to_str()
                .expect("fixture path is UTF-8"),
            "--repository-id",
            support::s1::REPOSITORY_ID,
            "--revision",
            COMMIT_A_OID,
            "--acquisition-profile",
            "local-git-sha1-packed-v1",
            "--format",
            "json",
        ],
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(arguments)
            .output()
            .expect("launch invalid acquisition selector");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).expect("strict input ErrorV6");
        assert_eq!(
            error,
            json!({
                "schema_version": "codenoesis.error/v6",
                "code": "input.invalid_acquisition_profile",
                "stage": "input",
                "message": "invalid acquisition profile",
                "retryable": false,
                "context": {}
            })
        );
    }
}

#[test]
fn sec_fr_acq_004_pack_integrity() {
    for (mutation, reason) in [
        (PackedMutation::PackHeader, "pack_header"),
        (PackedMutation::PackObjectCount, "object_count"),
        (PackedMutation::PackChecksum, "pack_checksum"),
        (PackedMutation::PackIndexMismatch, "pack_index_mismatch"),
    ] {
        assert_mutation_error(mutation, "pack", reason, false);
    }
}

#[test]
fn conf_fr_acq_004_entry_integrity() {
    for (mutation, reason) in [
        (PackedMutation::EntryHeader, "entry_header"),
        (PackedMutation::EntryCrc, "entry_crc"),
        (PackedMutation::ZlibStream, "zlib_stream"),
    ] {
        assert_mutation_error(mutation, "entry", reason, true);
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn pt_fr_acq_004_limits_have_max_and_plus_one() {
    for limit in PACKED_LIMIT_KINDS {
        let maximum = limit.maximum();
        assert_eq!(check_limit(limit, maximum), Ok(()));
        assert_eq!(
            check_limit(limit, maximum + 1),
            Err(AcquisitionError::LimitExceeded {
                limit,
                maximum,
                observed: maximum + 1,
            })
        );
    }

    let directory_repository = MaterializedRepository::revision_a();
    let directory_pack = materialize_base_only_pack(&directory_repository);
    let directory = directory_pack
        .pack_path
        .parent()
        .expect("limit pack directory");
    let existing = fs::read_dir(directory)
        .expect("enumerate initial pack entries")
        .count();
    for index in 0..(513_usize - existing) {
        fs::write(directory.join(format!("pack-{index:040x}.keep")), [])
            .expect("write pack directory limit sidecar");
    }
    assert_limit_error(
        scan_packed(&directory_repository.worktree, COMMIT_A_OID),
        LimitKind::PackDirectoryEntries,
    );

    let pairs_repository = MaterializedRepository::revision_a();
    let pairs = materialize_base_only_pack(&pairs_repository);
    let pair_directory = pairs.pack_path.parent().expect("pair limit directory");
    for index in 0..64_u64 {
        let pack_id = format!("{:040x}", index + 1);
        if pack_id == pairs.pack_id {
            continue;
        }
        fs::copy(
            &pairs.pack_path,
            pair_directory.join(format!("pack-{pack_id}.pack")),
        )
        .expect("copy pair-limit pack");
        fs::copy(
            &pairs.index_path,
            pair_directory.join(format!("pack-{pack_id}.idx")),
        )
        .expect("copy pair-limit index");
    }
    assert_limit_error(
        scan_packed(&pairs_repository.worktree, COMMIT_A_OID),
        LimitKind::PackPairs,
    );

    let index_repository = MaterializedRepository::revision_a();
    let mut index_pack = materialize_base_only_pack(&index_repository);
    index_pack.resize_index(LimitKind::SinglePackIndexBytes.maximum() + 1);
    assert_limit_error(
        scan_packed(&index_repository.worktree, COMMIT_A_OID),
        LimitKind::SinglePackIndexBytes,
    );

    let pack_repository = MaterializedRepository::revision_a();
    let mut oversized_pack = materialize_base_only_pack(&pack_repository);
    oversized_pack.resize_pack(LimitKind::SinglePackBytes.maximum() + 1);
    assert_limit_error(
        scan_packed(&pack_repository.worktree, COMMIT_A_OID),
        LimitKind::SinglePackBytes,
    );

    let inherited_repository = MaterializedRepository::revision_a();
    let maximum_blob = vec![
        b'm';
        usize::try_from(LimitKind::SingleFileBytes.maximum())
            .expect("single-file maximum fits usize")
    ];
    let oversized_blob = vec![b'o'; maximum_blob.len() + 1];
    let maximum_revision = inherited_repository.generated_single_file_commit(
        "maximum.bin",
        &maximum_blob,
        978_309_000,
    );
    let oversized_revision = inherited_repository.generated_single_file_commit(
        "oversized.bin",
        &oversized_blob,
        978_309_001,
    );
    retain_revision(&inherited_repository, "maximum", &maximum_revision);
    retain_revision(&inherited_repository, "oversized", &oversized_revision);
    let inherited_pack = materialize_base_only_pack(&inherited_repository);
    let maximum_output = scan_packed(&inherited_repository.worktree, &maximum_revision);
    assert!(
        maximum_output.status.success(),
        "packed inherited maximum failed: {}",
        String::from_utf8_lossy(&maximum_output.stderr)
    );
    assert_limit_error(
        scan_packed(&inherited_repository.worktree, &oversized_revision),
        LimitKind::SingleFileBytes,
    );
    inherited_pack.assert_unchanged();

    let locations_repository = MaterializedRepository::revision_a();
    let mut location_packs = Vec::new();
    for salt in 0..7_u64 {
        location_packs.push(materialize_duplicate_object_pack(
            &locations_repository,
            COMMIT_A_OID,
            salt,
        ));
    }
    let maximum_locations = scan_packed(&locations_repository.worktree, COMMIT_A_OID);
    assert!(
        maximum_locations.status.success(),
        "eight equivalent loose/pack locations failed: {}",
        String::from_utf8_lossy(&maximum_locations.stderr)
    );
    location_packs.push(materialize_duplicate_object_pack(
        &locations_repository,
        COMMIT_A_OID,
        7,
    ));
    assert_limit_error(
        scan_packed(&locations_repository.worktree, COMMIT_A_OID),
        LimitKind::ObjectLocations,
    );
    for pack in location_packs {
        pack.assert_unchanged();
    }
}

#[test]
fn reg_fr_acq_004_legacy_profiles_unchanged() {
    let loose_repository = MaterializedRepository::revision_a();
    let legacy_loose = support::s1::scan(&loose_repository.worktree, COMMIT_A_OID);
    let selected_loose = scan_packed(&loose_repository.worktree, COMMIT_A_OID);
    assert!(legacy_loose.status.success());
    assert!(selected_loose.status.success());
    let legacy: Value = serde_json::from_slice(&legacy_loose.stdout).expect("legacy loose S1");
    let selected: Value =
        serde_json::from_slice(&selected_loose.stdout).expect("selected loose S1");
    assert_eq!(selected["semantic"], legacy["semantic"]);
    assert_eq!(selected["semantic_hash"], legacy["semantic_hash"]);

    let packed_repository = MaterializedRepository::revision_a();
    let packed = materialize_base_only_pack(&packed_repository);
    let legacy_packed = support::s1::scan(&packed_repository.worktree, COMMIT_A_OID);
    packed.assert_unchanged();
    assert_eq!(legacy_packed.status.code(), Some(10));
    assert!(legacy_packed.stdout.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&legacy_packed.stderr).expect("legacy packed ErrorV2"),
        json!({
            "schema_version": "codenoesis.error/v2",
            "code": "acquisition.unsupported_repository_shape",
            "stage": "acquisition",
            "message": "unsupported Git repository shape",
            "retryable": false,
            "context": {"feature": "packed_object_database"}
        })
    );

    assert_selected_profile_equivalence();
}

#[test]
fn sec_fr_acq_004_delta_adversarial() {
    for encoding in [DeltaEncoding::Ofs, DeltaEncoding::Ref] {
        let repository = MaterializedRepository::revision_a();
        let candidates = generate_delta_candidates(&repository);
        let (mut packed, revision) = materialize_delta_pack(&repository, &candidates, encoding);
        let target_oid = candidates
            .iter()
            .find(|(commit, _)| commit == &revision)
            .map(|(_, oid)| oid)
            .expect("selected delta candidate OID");
        packed.mutate_delta_base(target_oid, encoding);
        let output = scan_packed(&repository.worktree, &revision);
        assert_eq!(output.status.code(), Some(10));
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).expect("delta ErrorV6");
        assert_eq!(error["code"], "acquisition.object_database_invalid");
        assert_eq!(error["context"]["component"], "delta");
        assert_eq!(error["context"]["reason"], "delta_base");
        assert_eq!(error["context"]["pack_id"], packed.pack_id);
        assert_eq!(error["context"]["object_oid"], target_oid.as_str());
    }

    let cycle_repository = MaterializedRepository::revision_a();
    let cycle_candidates = generate_delta_candidates(&cycle_repository);
    let (mut cycle_pack, cycle_revision) =
        materialize_delta_pack(&cycle_repository, &cycle_candidates, DeltaEncoding::Ref);
    let cycle_oid = cycle_candidates
        .iter()
        .find(|(commit, _)| commit == &cycle_revision)
        .map(|(_, oid)| oid)
        .expect("selected REF_DELTA cycle object ID");
    cycle_pack.mutate_ref_delta_to_self_cycle(cycle_oid);
    assert_error_context(
        scan_packed(&cycle_repository.worktree, &cycle_revision),
        "acquisition.object_database_invalid",
        json!({
            "component": "delta",
            "reason": "delta_cycle",
            "pack_id": cycle_pack.pack_id,
            "object_oid": cycle_oid
        }),
        false,
    );

    assert_eq!(
        check_limit(LimitKind::DeltaDepth, 51),
        Err(AcquisitionError::LimitExceeded {
            limit: LimitKind::DeltaDepth,
            maximum: 50,
            observed: 51,
        })
    );
    assert_eq!(
        check_limit(LimitKind::DeltaInstructions, 4_194_305),
        Err(AcquisitionError::LimitExceeded {
            limit: LimitKind::DeltaInstructions,
            maximum: 4_194_304,
            observed: 4_194_305,
        })
    );
}

#[test]
fn pt_fr_acq_004_location_and_order_invariant() {
    let repository = MaterializedRepository::revision_a();
    let packed = materialize_base_only_pack(&repository);
    let expected = semantic_fingerprint(scan_packed(&repository.worktree, COMMIT_A_OID));
    let directory = packed.pack_path.parent().expect("pack directory");
    let sidecars = (0_u64..4)
        .map(|index| directory.join(format!("pack-{:040x}.keep", index + 100)))
        .collect::<Vec<_>>();
    for seed in 0..50_usize {
        for path in &sidecars {
            let _ = fs::remove_file(path);
        }
        for offset in 0..sidecars.len() {
            let index = (seed + offset) % sidecars.len();
            fs::write(&sidecars[index], []).expect("create catalog permutation sidecar");
        }
        assert_eq!(
            semantic_fingerprint(scan_packed(&repository.worktree, COMMIT_A_OID)),
            expected,
            "catalog permutation {seed}"
        );
    }

    let worktree = repository.worktree.clone();
    let parallel = std::thread::scope(|scope| {
        (0..10)
            .map(|_| {
                let worktree = worktree.clone();
                scope.spawn(move || semantic_fingerprint(scan_packed(&worktree, COMMIT_A_OID)))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| handle.join().expect("parallel packed scan"))
            .collect::<Vec<_>>()
    });
    for (repetition, observed) in parallel.into_iter().enumerate() {
        assert_eq!(observed, expected, "parallel repetition {repetition}");
    }
    packed.assert_unchanged();
}

#[test]
fn race_fr_acq_004_pack_replacement() {
    let repository = MaterializedRepository::revision_a();
    let packed = materialize_base_only_pack(&repository);
    let replacement = repository.root.join("race-pack-replacement");
    let mut command = scan_packed_command(&repository.worktree, COMMIT_A_OID);
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let child = command.spawn().expect("launch race subject");
    fs::rename(&packed.pack_path, &replacement).expect("schedule pack rename");
    let output = child.wait_with_output().expect("wait for race subject");
    fs::rename(&replacement, &packed.pack_path).expect("restore raced pack");
    match output.status.code() {
        Some(0) => {
            assert!(output.stderr.is_empty());
            assert!(!output.stdout.is_empty());
        }
        Some(10) => {
            assert!(output.stdout.is_empty());
            let error: Value = serde_json::from_slice(&output.stderr).expect("race ErrorV6");
            assert_eq!(error["code"], "acquisition.object_database_changed");
            assert_eq!(error["retryable"], true);
            assert!(matches!(
                error["context"]["component"].as_str(),
                Some("catalog" | "pack")
            ));
        }
        status => panic!(
            "race produced forbidden status {status:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    }
    packed.assert_unchanged();
}

#[test]
fn sec_fr_acq_004_scan_has_no_new_authority() {
    let repository = MaterializedRepository::revision_a();
    let packed = materialize_base_only_pack(&repository);
    let sentinel = repository.root.join("packed-authority-sentinel");
    let expected = b"no process, network, target execution, or outside write\n";
    fs::write(&sentinel, expected).expect("write authority sentinel");
    repository.apply_isolation_variant(&sentinel);
    let output = scan_packed_command(&repository.worktree, COMMIT_A_OID)
        .env("CODENOESIS_SENTINEL", &sentinel)
        .output()
        .expect("launch isolated packed scan");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read(&sentinel).expect("read authority sentinel"),
        expected
    );
    packed.assert_unchanged();
}

#[test]
fn fz_fr_acq_004_pack_index_delta_seed_corpus() {
    let mutations = [
        PackedMutation::IndexLayout,
        PackedMutation::IndexFanout,
        PackedMutation::IndexObjectOrder,
        PackedMutation::IndexOffset,
        PackedMutation::IndexChecksum,
        PackedMutation::PackHeader,
        PackedMutation::PackObjectCount,
        PackedMutation::PackChecksum,
        PackedMutation::PackIndexMismatch,
        PackedMutation::EntryHeader,
        PackedMutation::EntryCrc,
        PackedMutation::ZlibStream,
    ];
    for (seed, mutation) in mutations.into_iter().enumerate() {
        let repository = MaterializedRepository::revision_a();
        let mut packed = materialize_base_only_pack(&repository);
        packed.mutate(mutation);
        let output = scan_packed(&repository.worktree, COMMIT_A_OID);
        assert_eq!(output.status.code(), Some(10), "seed {seed}");
        assert!(output.stdout.is_empty(), "seed {seed}");
        let error: Value =
            serde_json::from_slice(&output.stderr).expect("bounded seed emits one ErrorV6");
        assert_eq!(error["schema_version"], "codenoesis.error/v6");
        assert_ne!(error["code"], "internal.unexpected", "seed {seed}");
    }
}

#[test]
fn diff_fr_acq_004_offline_reference_readers() {
    let repository = MaterializedRepository::revision_a();
    let packed = materialize_base_only_pack(&repository);
    let git = offline_verify_pack(&repository, &packed);
    assert!(
        String::from_utf8_lossy(&git).contains(COMMIT_A_OID),
        "offline Git oracle did not enumerate the selected commit"
    );
    let product = scan_packed(&repository.worktree, COMMIT_A_OID);
    assert!(
        product.status.success(),
        "product diverged from the reviewed valid pack: {}",
        String::from_utf8_lossy(&product.stderr)
    );
    assert_eq!(
        semantic_fingerprint(product),
        (
            serde_json::from_slice::<Value>(
                &fs::read(fixture_root().join("expected-semantic-a.jcs"))
                    .expect("read semantic differential golden")
            )
            .expect("semantic differential golden JSON"),
            json!({
                "algorithm": "blake3-256",
                "value": "236b231c3154f9be56130ddc8dfb39bb482af10330f7c6757597ad22c006e9e7"
            }),
        )
    );
    packed.assert_unchanged();
}

fn assert_delta_equivalence(encoding: DeltaEncoding) {
    let loose_repository = MaterializedRepository::revision_a();
    let packed_repository = MaterializedRepository::revision_a();
    let loose_candidates = generate_delta_candidates(&loose_repository);
    let packed_candidates = generate_delta_candidates(&packed_repository);
    assert_eq!(loose_candidates, packed_candidates);
    let (packed, revision) =
        materialize_delta_pack(&packed_repository, &packed_candidates, encoding);

    let loose = scan_packed(&loose_repository.worktree, &revision);
    let packed_output = scan_packed(&packed_repository.worktree, &revision);
    packed.assert_unchanged();
    assert!(
        loose.status.success(),
        "selected loose control failed: {}",
        String::from_utf8_lossy(&loose.stderr)
    );
    assert!(
        packed_output.status.success(),
        "selected packed delta scan failed: {}",
        String::from_utf8_lossy(&packed_output.stderr)
    );
    let loose: Value = serde_json::from_slice(&loose.stdout).expect("loose snapshot");
    let packed: Value = serde_json::from_slice(&packed_output.stdout).expect("packed snapshot");
    assert_eq!(packed["semantic"], loose["semantic"]);
    assert_eq!(packed["semantic_hash"], loose["semantic_hash"]);
}

fn assert_external_ref_equivalence(storage: ExternalBaseStorage) {
    let loose_repository = MaterializedRepository::revision_a();
    let packed_repository = MaterializedRepository::revision_a();
    let loose_candidates = generate_delta_candidates(&loose_repository);
    let packed_candidates = generate_delta_candidates(&packed_repository);
    assert_eq!(loose_candidates, packed_candidates);
    let (mut packed, revision) =
        materialize_delta_pack(&packed_repository, &packed_candidates, DeltaEncoding::Ref);
    let target_object_id = packed_candidates
        .iter()
        .find(|(commit, _)| commit == &revision)
        .map(|(_, object_id)| object_id)
        .expect("selected REF_DELTA candidate object ID");
    let base_object_id = packed.externalize_ref_base(&packed_repository, target_object_id, storage);

    assert_ne!(
        target_object_id, &base_object_id,
        "REF_DELTA target and external base must differ"
    );
    let loose = scan_packed(&loose_repository.worktree, &revision);
    let packed_output = scan_packed(&packed_repository.worktree, &revision);
    packed.assert_unchanged();
    assert!(
        loose.status.success(),
        "selected loose external-base control failed: {}",
        String::from_utf8_lossy(&loose.stderr)
    );
    assert!(
        packed_output.status.success(),
        "selected external-base REF_DELTA scan failed: {}",
        String::from_utf8_lossy(&packed_output.stderr)
    );
    assert_eq!(
        semantic_fingerprint(packed_output),
        semantic_fingerprint(loose)
    );
}

fn assert_selected_profile_equivalence() {
    let loose_s2 = support::s2::MaterializedRepository::revision_a();
    let packed_s2 = support::s2::MaterializedRepository::revision_a();
    let packed_s2_files = materialize_base_only_pack_at(&packed_s2.root, &packed_s2.worktree);
    let loose_s2_output = support::s2::scan(&loose_s2.worktree, support::s2::COMMIT_A_OID);
    let packed_s2_output = scan_selected_profile(
        &packed_s2.worktree,
        support::s2::REPOSITORY_ID,
        support::s2::COMMIT_A_OID,
        "standard-local-s2",
        None,
    );
    assert_equivalent_profile_outputs("S2", loose_s2_output, packed_s2_output);
    packed_s2_files.assert_unchanged();

    let loose_s3 = support::s3::MaterializedRepository::revisions();
    let packed_s3 = support::s3::MaterializedRepository::revisions();
    let packed_s3_files = materialize_base_only_pack_at(&packed_s3.root, &packed_s3.worktree);
    let loose_s3_output = support::s3::scan(
        &loose_s3.worktree,
        &loose_s3.store,
        support::s3::COMMIT_A_OID,
    );
    let packed_s3_output = scan_selected_profile(
        &packed_s3.worktree,
        support::s3::REPOSITORY_ID,
        support::s3::COMMIT_A_OID,
        "standard-local-s3",
        Some(&packed_s3.store),
    );
    assert_equivalent_profile_outputs("S3", loose_s3_output, packed_s3_output);
    assert_eq!(
        directory_snapshot(&packed_s3.store),
        directory_snapshot(&loose_s3.store),
        "S3 storage artifacts changed with physical object representation"
    );
    packed_s3_files.assert_unchanged();

    let loose_s4 = support::s4::MaterializedRepository::revision_a();
    let packed_s4 = support::s4::MaterializedRepository::revision_a();
    let packed_s4_files = materialize_base_only_pack_at(&packed_s4.root, &packed_s4.worktree);
    let loose_s4_output = support::s4::scan(&loose_s4);
    let packed_s4_output = scan_selected_profile(
        &packed_s4.worktree,
        support::s4::REPOSITORY_ID,
        support::s4::COMMIT_A_OID,
        "standard-local-s4",
        Some(&packed_s4.store),
    );
    assert_equivalent_profile_outputs("S4", loose_s4_output, packed_s4_output);
    assert_eq!(
        directory_snapshot(&packed_s4.store),
        directory_snapshot(&loose_s4.store),
        "S4 storage artifacts changed with physical object representation"
    );
    packed_s4_files.assert_unchanged();
}

fn scan_selected_profile(
    repository: &Path,
    repository_id: &str,
    revision: &str,
    profile: &str,
    store: Option<&Path>,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
    command
        .args(["scan", "--repository"])
        .arg(repository)
        .args([
            "--repository-id",
            repository_id,
            "--revision",
            revision,
            "--profile",
            profile,
            "--acquisition-profile",
            "local-git-sha1-packed-v1",
        ]);
    if let Some(store) = store {
        command.arg("--store").arg(store);
    }
    command
        .args(["--format", "json"])
        .output()
        .expect("launch selected packed standard profile")
}

#[allow(clippy::needless_pass_by_value)]
fn assert_equivalent_profile_outputs(profile: &str, loose: Output, packed: Output) {
    assert!(
        loose.status.success(),
        "{profile} loose control failed: {}",
        String::from_utf8_lossy(&loose.stderr)
    );
    assert!(
        packed.status.success(),
        "{profile} selected packed scan failed: {}",
        String::from_utf8_lossy(&packed.stderr)
    );
    assert_eq!(packed.stderr, loose.stderr, "{profile} stderr changed");
    let mut loose: Value =
        serde_json::from_slice(&loose.stdout).expect("parse loose profile output");
    let mut packed: Value =
        serde_json::from_slice(&packed.stdout).expect("parse selected packed profile output");
    remove_operational_envelopes(&mut loose);
    remove_operational_envelopes(&mut packed);
    assert_eq!(packed, loose, "{profile} output changed");
}

fn directory_snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(root: &Path, directory: &Path, output: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries = fs::read_dir(directory)
            .expect("enumerate profile artifact directory")
            .map(|entry| entry.expect("read profile artifact entry"))
            .collect::<Vec<_>>();
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).expect("inspect profile artifact");
            assert!(!metadata.file_type().is_symlink());
            if metadata.is_dir() {
                visit(root, &path, output);
            } else {
                assert!(metadata.is_file());
                output.push((
                    path.strip_prefix(root)
                        .expect("profile artifact stays under store")
                        .to_path_buf(),
                    normalized_artifact_bytes(&path),
                ));
            }
        }
    }

    let mut output = Vec::new();
    visit(root, root, &mut output);
    output
}

fn normalized_artifact_bytes(path: &Path) -> Vec<u8> {
    let bytes = fs::read(path).expect("read profile artifact");
    let Ok(mut value) = serde_json::from_slice::<Value>(&bytes) else {
        return bytes;
    };
    remove_operational_envelopes(&mut value);
    serde_json::to_vec(&value).expect("serialize normalized profile artifact")
}

fn remove_operational_envelopes(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("envelope");
            for value in object.values_mut() {
                remove_operational_envelopes(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                remove_operational_envelopes(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn assert_mutation_error(
    mutation: PackedMutation,
    component: &str,
    reason: &str,
    object_context: bool,
) {
    let repository = MaterializedRepository::revision_a();
    let mut packed = materialize_base_only_pack(&repository);
    packed.mutate(mutation);
    let output = scan_packed(&repository.worktree, COMMIT_A_OID);
    assert_eq!(output.status.code(), Some(10));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("strict ErrorV6");
    assert_eq!(error["schema_version"], "codenoesis.error/v6");
    assert_eq!(error["code"], "acquisition.object_database_invalid");
    assert_eq!(error["stage"], "acquisition");
    assert_eq!(error["retryable"], false);
    assert_eq!(error["context"]["component"], component);
    assert_eq!(error["context"]["reason"], reason);
    let context = error["context"].as_object().expect("V6 context object");
    assert_eq!(
        context.get("pack_id").and_then(Value::as_str),
        Some(packed.pack_id.as_str())
    );
    if object_context {
        let object_oid = context
            .get("object_oid")
            .and_then(Value::as_str)
            .expect("object-specific V6 context");
        assert_eq!(object_oid.len(), 40);
        assert!(object_oid.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(context.len(), 4);
    } else {
        assert_eq!(context.len(), 3);
    }
}

#[allow(clippy::needless_pass_by_value)]
fn assert_error_context(output: std::process::Output, code: &str, context: Value, retryable: bool) {
    assert_eq!(output.status.code(), Some(10));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("strict ErrorV6");
    assert_eq!(error["schema_version"], "codenoesis.error/v6");
    assert_eq!(error["code"], code);
    assert_eq!(error["stage"], "acquisition");
    assert_eq!(error["retryable"], retryable);
    assert_eq!(error["context"], context);
}

fn assert_limit_error(output: std::process::Output, limit: LimitKind) {
    assert_error_context(
        output,
        "acquisition.limit_exceeded",
        json!({
            "limit": limit.as_str(),
            "maximum": limit.maximum(),
            "observed": limit.maximum() + 1
        }),
        false,
    );
}

#[allow(clippy::needless_pass_by_value)]
fn semantic_fingerprint(output: std::process::Output) -> (Value, Value) {
    assert!(
        output.status.success(),
        "packed scan failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let snapshot: Value = serde_json::from_slice(&output.stdout).expect("packed snapshot JSON");
    (
        snapshot["semantic"].clone(),
        snapshot["semantic_hash"].clone(),
    )
}
