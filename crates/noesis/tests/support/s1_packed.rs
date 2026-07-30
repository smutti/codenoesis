use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::s1::{MaterializedRepository, REPOSITORY_ID};
use super::{git_command, successful_output};

const PACK_PREFIX: &str = "pack-";
const PACK_ID_BYTES: usize = 40;

pub struct PackedMaterialization {
    pub index_path: PathBuf,
    pub pack_id: String,
    pub pack_path: PathBuf,
    index_bytes: Vec<u8>,
    pack_bytes: Vec<u8>,
}

impl PackedMaterialization {
    pub fn assert_unchanged(&self) {
        assert_eq!(
            fs::read(&self.index_path).expect("read packed fixture index after subject"),
            self.index_bytes,
            "monitored subject changed the packed fixture index"
        );
        assert_eq!(
            fs::read(&self.pack_path).expect("read packed fixture pack after subject"),
            self.pack_bytes,
            "monitored subject changed the packed fixture pack"
        );
    }
}

pub fn materialize_base_only_pack(repository: &MaterializedRepository) -> PackedMaterialization {
    let global_config = repository.root.join("packed-global.gitconfig");
    fs::write(&global_config, []).expect("create packed fixture Git configuration");

    let mut repack = git_command(&global_config);
    repack.arg("-C").arg(&repository.worktree).args([
        "repack",
        "-a",
        "-d",
        "--window=0",
        "--depth=0",
        "--no-write-bitmap-index",
    ]);
    successful_output(repack, None);

    let mut prune = git_command(&global_config);
    prune
        .arg("-C")
        .arg(&repository.worktree)
        .arg("prune-packed");
    successful_output(prune, None);

    assert_no_loose_objects(&repository.worktree.join(".git/objects"));

    let pack_directory = repository.worktree.join(".git/objects/pack");
    let mut pack_paths = Vec::new();
    let mut index_paths = Vec::new();
    for entry in fs::read_dir(&pack_directory).expect("enumerate packed fixture directory") {
        let entry = entry.expect("read packed fixture directory entry");
        let path = entry.path();
        match path.extension().and_then(OsStr::to_str) {
            Some("pack") => pack_paths.push(path),
            Some("idx") => index_paths.push(path),
            _ => {}
        }
    }
    pack_paths.sort();
    index_paths.sort();
    assert_eq!(pack_paths.len(), 1, "fixture must contain exactly one pack");
    assert_eq!(
        index_paths.len(),
        1,
        "fixture must contain exactly one index"
    );

    let pack_path = pack_paths.remove(0);
    let index_path = index_paths.remove(0);
    let pack_id = authoritative_pack_id(&pack_path, "pack");
    assert_eq!(
        authoritative_pack_id(&index_path, "idx"),
        pack_id,
        "fixture pack and index IDs differ"
    );

    let pack_bytes = fs::read(&pack_path).expect("read packed fixture pack");
    let index_bytes = fs::read(&index_path).expect("read packed fixture index");
    assert_eq!(&pack_bytes[..4], b"PACK", "fixture pack signature changed");
    assert_eq!(
        u32::from_be_bytes(pack_bytes[4..8].try_into().expect("pack version bytes")),
        2,
        "fixture pack must use version 2"
    );
    assert_eq!(
        &index_bytes[..4],
        &[0xff, 0x74, 0x4f, 0x63],
        "fixture index signature changed"
    );
    assert_eq!(
        u32::from_be_bytes(index_bytes[4..8].try_into().expect("index version bytes")),
        2,
        "fixture index must use version 2"
    );

    let mut verify = git_command(&global_config);
    verify
        .arg("-C")
        .arg(&repository.worktree)
        .args(["verify-pack", "-v"])
        .arg(&index_path);
    successful_output(verify, None);

    PackedMaterialization {
        index_path,
        pack_id,
        pack_path,
        index_bytes,
        pack_bytes,
    }
}

pub fn scan_packed(repository: &Path, revision: &str) -> Output {
    scan_packed_command(repository, revision)
        .output()
        .expect("launch selected packed noesis scan")
}

pub fn scan_packed_command(repository: &Path, revision: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
    command
        .args(["scan", "--repository"])
        .arg(repository)
        .args([
            "--repository-id",
            REPOSITORY_ID,
            "--revision",
            revision,
            "--profile",
            "standard-local-s1",
            "--acquisition-profile",
            "local-git-sha1-packed-v1",
            "--format",
            "json",
        ]);
    command
}

fn assert_no_loose_objects(objects_directory: &Path) {
    for entry in fs::read_dir(objects_directory).expect("enumerate fixture object directory") {
        let entry = entry.expect("read fixture object directory entry");
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.len() != 2 || !name.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path()).expect("inspect loose object directory");
        assert!(
            metadata.is_dir(),
            "loose object fanout entry is not a directory"
        );
        assert_eq!(
            fs::read_dir(entry.path())
                .expect("enumerate loose object fanout directory")
                .count(),
            0,
            "packed fixture retained a loose object fallback"
        );
    }
}

fn authoritative_pack_id(path: &Path, extension: &str) -> String {
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .expect("fixture pack name must be UTF-8");
    let suffix = format!(".{extension}");
    let pack_id = name
        .strip_prefix(PACK_PREFIX)
        .and_then(|value| value.strip_suffix(&suffix))
        .expect("fixture pack name must be authoritative");
    assert_eq!(pack_id.len(), PACK_ID_BYTES);
    assert!(
        pack_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "fixture pack ID must be lowercase hexadecimal"
    );
    pack_id.to_owned()
}
