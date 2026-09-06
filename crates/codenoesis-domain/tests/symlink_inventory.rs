use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, AcquiredSymlink, BoundRevision, ObjectId, RegularFileMode,
    RepositoryIdentity, RepositoryInventory, SymlinkTargetKind,
};

fn oid(digit: &str) -> ObjectId {
    ObjectId::parse_sha1(&digit.repeat(40)).expect("fixture object identity")
}

#[test]
fn conf_fr_acq_002_committed_symlinks_remain_separate_from_regular_source_inventory() {
    let bound = BoundRevision::new(
        RepositoryIdentity::parse("urn:codenoesis:test:symlink-inventory").unwrap(),
        oid("a"),
        oid("b"),
    );
    let source = AcquiredFile::new(
        "src/lib.rs".to_owned(),
        RegularFileMode::Regular,
        oid("c"),
        b"pub fn main() {}".to_vec(),
    );
    let link = AcquiredSymlink {
        path: "alias.rs".to_owned(),
        blob_oid: oid("d"),
        bytes: b"src/lib.rs".to_vec(),
        resolved_target: "src/lib.rs".to_owned(),
        target_oid: oid("c"),
        target_kind: SymlinkTargetKind::File,
    };
    let directory = AcquiredSymlink {
        path: "directory-alias".to_owned(),
        blob_oid: oid("e"),
        bytes: b"src".to_vec(),
        resolved_target: "src".to_owned(),
        target_oid: oid("f"),
        target_kind: SymlinkTargetKind::Directory,
    };
    let plain = AcquiredRepository::new(bound, 1, vec![source]);
    let baseline = RepositoryInventory::classify(plain.clone());
    let inventory =
        RepositoryInventory::classify(plain.with_symlinks(vec![directory.clone(), link.clone()]));
    assert_eq!(inventory.files(), baseline.files());
    assert_eq!(inventory.directory_count(), baseline.directory_count());
    assert_eq!(inventory.symlinks(), &[link, directory]);
    assert!(baseline.symlinks().is_empty());
}
