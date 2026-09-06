use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use codenoesis_domain::{
    AcquisitionError, EntryPolicy, ObjectId, RepositoryError, RepositoryIdentity,
    RepositoryInventory, Revision, SymlinkTargetKind,
};
use codenoesis_ports::{RepositoryBoundaryAcquirer, SafeRepositoryAcquirer};
use codenoesis_repository::LocalGitRepository;
use flate2::{Compression, write::ZlibEncoder};
use sha1::{Digest, Sha1};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "codenoesis-symlinks-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join(".git/objects")).unwrap();
        fs::write(
            root.join(".git/config"),
            b"[core]\nrepositoryformatversion = 0\nbare = false\n",
        )
        .unwrap();
        fs::write(root.join(".git/HEAD"), b"ref: refs/heads/main\n").unwrap();
        Self {
            root: fs::canonicalize(root).unwrap(),
        }
    }

    fn object(&self, kind: &str, bytes: &[u8]) -> ObjectId {
        let mut raw = format!("{kind} {}\0", bytes.len()).into_bytes();
        raw.extend(bytes);
        let mut id = String::with_capacity(40);
        for byte in Sha1::digest(&raw) {
            write!(id, "{byte:02x}").unwrap();
        }
        let path = self.root.join(".git/objects").join(&id[..2]).join(&id[2..]);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw).unwrap();
        fs::write(path, encoder.finish().unwrap()).unwrap();
        ObjectId::parse_sha1(&id).unwrap()
    }

    fn tree(&self, entries: &[(&str, &str, ObjectId)]) -> ObjectId {
        let mut raw = Vec::new();
        for (mode, name, oid) in entries {
            raw.extend(format!("{mode} {name}\0").bytes());
            for pair in oid.as_str().as_bytes().chunks_exact(2) {
                raw.push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap());
            }
        }
        self.object("tree", &raw)
    }

    fn acquire(
        &self,
        tree: &ObjectId,
        legacy: bool,
    ) -> Result<RepositoryInventory, RepositoryError> {
        let commit = self.object(
            "commit",
            format!("tree {}\n\nsymlink fixture\n", tree.as_str()).as_bytes(),
        );
        let adapter = if legacy {
            LocalGitRepository::new_packed_sha1_rust_8m()
        } else {
            LocalGitRepository::new_packed_sha1_internal_symlinks()
        };
        adapter
            .acquire_inventory(
                self.root.as_os_str(),
                RepositoryIdentity::parse("urn:codenoesis:test:symlink").unwrap(),
                Revision::Commit(commit),
            )
            .map(RepositoryInventory::classify)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn e2e_fr_acq_002_internal_links_retain_git_evidence_without_alias_files() {
    let f = Fixture::new();
    let file = f.object("blob", b"pub fn source() {}\n");
    let directory = f.tree(&[("100644", "source.rs", file.clone())]);
    let alias = f.object("blob", b"src/");
    let link = f.object("blob", b"alias/source.rs");
    let tree = f.tree(&[
        ("120000", "alias", alias.clone()),
        ("120000", "link.rs", link.clone()),
        ("40000", "src", directory.clone()),
    ]);
    fs::write(f.root.join("link.rs"), b"HOST CANARY MUST NOT BE READ").unwrap();
    let inventory = f.acquire(&tree, false).unwrap();
    assert_eq!(inventory.files().len(), 1);
    assert_eq!(inventory.files()[0].path(), "src/source.rs");
    assert_eq!(inventory.symlinks().len(), 2);
    let links = inventory.symlinks();
    assert_eq!(links[0].path, "alias");
    assert_eq!(links[0].blob_oid, alias);
    assert_eq!(links[0].bytes, b"src/");
    assert_eq!(links[0].target_kind, SymlinkTargetKind::Directory);
    assert_eq!(links[0].target_oid, directory);
    assert_eq!(links[1].resolved_target, "src/source.rs");
    assert_eq!(links[1].target_kind, SymlinkTargetKind::File);
    assert_eq!(links[1].target_oid, file);
    assert_eq!(links[1].blob_oid, link);
    assert_eq!(
        fs::read(f.root.join("link.rs")).unwrap(),
        b"HOST CANARY MUST NOT BE READ"
    );
    assert!(matches!(
        f.acquire(&tree, true),
        Err(RepositoryError::Acquisition(
            AcquisitionError::EntryPolicyViolation {
                entry: EntryPolicy::Symlink,
                ..
            }
        ))
    ));
}

#[test]
fn sec_nfr_sec_001_internal_link_invalid_targets_fail_closed() {
    for target in [
        b"/etc/passwd".as_slice(),
        b"../escape",
        b"missing",
        b"link",
        b"src/../../escape",
        b"src\\source.rs",
        b"src/\0source.rs",
        b"src//source.rs",
        b"",
        b"C:/Windows",
        b"src/source.rs/",
        b"src/source.rs/..",
        b".",
        b"\xff",
    ] {
        let f = Fixture::new();
        let file = f.object("blob", b"source");
        let dir = f.tree(&[("100644", "source.rs", file)]);
        let link = f.object("blob", target);
        let tree = f.tree(&[("120000", "link", link), ("40000", "src", dir)]);
        assert!(
            matches!(
                f.acquire(&tree, false),
                Err(RepositoryError::Acquisition(
                    AcquisitionError::EntryPolicyViolation {
                        entry: EntryPolicy::Symlink,
                        ..
                    }
                ))
            ),
            "target {target:?}"
        );
    }
}

#[test]
fn sec_nfr_sec_001_internal_links_resolve_parent_after_directory_alias() {
    let f = Fixture::new();
    let file = f.object("blob", b"source");
    let empty = f.tree(&[]);
    let nested = f.tree(&[
        ("40000", "nested", empty),
        ("100644", "source.rs", file.clone()),
    ]);
    let alias = f.object("blob", b"src/nested");
    let link = f.object("blob", b"alias/../source.rs");
    let tree = f.tree(&[
        ("120000", "alias", alias),
        ("120000", "link", link),
        ("40000", "src", nested),
    ]);
    let inventory = f.acquire(&tree, false).unwrap();
    assert_eq!(inventory.symlinks()[1].resolved_target, "src/source.rs");
    assert_eq!(inventory.symlinks()[1].target_oid, file);

    let alias = f.object("blob", b"src");
    let link = f.object("blob", b"alias/../alias/source.rs");
    let tree = f.tree(&[
        ("120000", "alias", alias),
        ("120000", "link", link),
        (
            "40000",
            "src",
            f.tree(&[("100644", "source.rs", file.clone())]),
        ),
    ]);
    let inventory = f.acquire(&tree, false).unwrap();
    assert_eq!(inventory.symlinks()[1].resolved_target, "src/source.rs");
    assert_eq!(inventory.symlinks()[1].target_oid, file);
}

#[test]
fn sec_nfr_sec_001_internal_links_reject_cycles_and_bound_expansions() {
    for count in [32, 33] {
        let f = Fixture::new();
        let mut names: Vec<String> = (0..count).map(|i| format!("link{i:02}")).collect();
        names.push("target".into());
        let mut objects: Vec<(&str, String, ObjectId)> = (0..count)
            .map(|i| {
                (
                    "120000",
                    names[i].clone(),
                    f.object("blob", names[i + 1].as_bytes()),
                )
            })
            .collect();
        objects.push(("100644", "target".into(), f.object("blob", b"source")));
        let entries: Vec<_> = objects
            .iter()
            .map(|(mode, name, oid)| (*mode, name.as_str(), oid.clone()))
            .collect();
        let result = f.acquire(&f.tree(&entries), false);
        assert_eq!(result.is_ok(), count == 32);
    }
    let f = Fixture::new();
    let a = f.object("blob", b"b");
    let b = f.object("blob", b"a");
    assert!(
        f.acquire(&f.tree(&[("120000", "a", a), ("120000", "b", b)]), false)
            .is_err()
    );
}

#[test]
fn pt_fr_acq_002_internal_link_target_bytes_have_exact_bound() {
    for length in [1_024, 1_025] {
        let f = Fixture::new();
        let mut target = "./".repeat(508);
        target.push_str(if length == 1_024 {
            "file.bin"
        } else {
            "file.binx"
        });
        assert_eq!(target.len(), length);
        let file = f.object("blob", b"source");
        let link = f.object("blob", target.as_bytes());
        let tree = f.tree(&[("100644", "file.bin", file), ("120000", "link", link)]);
        let result = f.acquire(&tree, false);
        if length == 1_024 {
            assert!(result.is_ok());
        } else {
            assert!(
                matches!(
                    result,
                    Err(RepositoryError::Acquisition(
                        AcquisitionError::LimitExceeded {
                            maximum: 1_024,
                            observed: 1_025,
                            ..
                        }
                    ))
                ),
                "unexpected boundary result: {result:?}"
            );
        }
    }
}

#[test]
fn sec_nfr_sec_001_internal_link_cannot_cross_gitlink_boundary() {
    let f = Fixture::new();
    let oid = ObjectId::parse_sha1("1111111111111111111111111111111111111111").unwrap();
    let link = f.object("blob", b"nested/src/lib.rs");
    let tree = f.tree(&[("120000", "link", link), ("160000", "nested", oid)]);
    let commit = f.object(
        "commit",
        format!("tree {}\n\nboundary\n", tree.as_str()).as_bytes(),
    );
    let result = LocalGitRepository::new_packed_sha1_internal_symlinks()
        .acquire_inventory_with_boundaries(
            f.root.as_os_str(),
            RepositoryIdentity::parse("urn:codenoesis:test:symlink").unwrap(),
            Revision::Commit(commit),
        );
    assert!(matches!(
        result,
        Err(
            codenoesis_domain::s1_boundaries::RepositoryBoundaryAcquisitionError::Repository(
                RepositoryError::Acquisition(AcquisitionError::EntryPolicyViolation {
                    entry: EntryPolicy::Symlink,
                    ..
                })
            )
        )
    ));
}
