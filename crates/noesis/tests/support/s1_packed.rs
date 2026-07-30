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

#[derive(Clone, Copy)]
pub enum DeltaEncoding {
    Ofs,
    Ref,
}

#[derive(Clone, Copy)]
pub enum ExternalBaseStorage {
    Loose,
    Packed,
}

#[derive(Clone, Copy)]
pub enum PackedMutation {
    IndexLayout,
    IndexFanout,
    IndexObjectOrder,
    IndexOffset,
    IndexChecksum,
    PackHeader,
    PackObjectCount,
    PackChecksum,
    PackIndexMismatch,
    EntryHeader,
    EntryCrc,
    ZlibStream,
}

struct IndexRecord {
    object_id: [u8; 20],
    crc32: u32,
    offset: usize,
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

    pub fn mutate(&mut self, mutation: PackedMutation) {
        match mutation {
            PackedMutation::IndexLayout => {
                self.index_bytes.push(0);
                self.write_index();
            }
            PackedMutation::IndexFanout => {
                let value = read_u32(&self.index_bytes, 8).expect("first fanout value");
                self.index_bytes[8..12].copy_from_slice(&value.saturating_add(1).to_be_bytes());
                refresh_index_checksum(&mut self.index_bytes);
                self.write_index();
            }
            PackedMutation::IndexObjectOrder => {
                let count = index_object_count(&self.index_bytes);
                assert!(count > 1, "fixture index needs at least two OIDs");
                let oid_start = 8 + 256 * 4;
                let first = self.index_bytes[oid_start..oid_start + 20].to_vec();
                self.index_bytes[oid_start + 20..oid_start + 40].copy_from_slice(&first);
                refresh_index_checksum(&mut self.index_bytes);
                self.write_index();
            }
            PackedMutation::IndexOffset => {
                let offset_start = index_offset_start(&self.index_bytes);
                self.index_bytes[offset_start..offset_start + 4]
                    .copy_from_slice(&11_u32.to_be_bytes());
                refresh_index_checksum(&mut self.index_bytes);
                self.write_index();
            }
            PackedMutation::IndexChecksum => {
                let last = self.index_bytes.len() - 1;
                self.index_bytes[last] ^= 1;
                self.write_index();
            }
            PackedMutation::PackHeader => {
                self.pack_bytes[0] ^= 1;
                self.write_pack();
            }
            PackedMutation::PackObjectCount => {
                let count = read_u32(&self.pack_bytes, 8).expect("pack object count");
                self.pack_bytes[8..12].copy_from_slice(&count.saturating_add(1).to_be_bytes());
                self.write_pack();
            }
            PackedMutation::PackChecksum => {
                let last = self.pack_bytes.len() - 1;
                self.pack_bytes[last] ^= 1;
                self.write_pack();
            }
            PackedMutation::PackIndexMismatch => {
                let checksum = self.index_bytes.len() - 40;
                self.index_bytes[checksum] ^= 1;
                refresh_index_checksum(&mut self.index_bytes);
                self.write_index();
            }
            PackedMutation::EntryHeader => {
                let offset = first_entry_offset(&self.index_bytes);
                self.pack_bytes[offset] &= 0x8f;
                self.rebind_pack();
            }
            PackedMutation::EntryCrc => {
                let crc_start = index_crc_start(&self.index_bytes);
                self.index_bytes[crc_start] ^= 1;
                refresh_index_checksum(&mut self.index_bytes);
                self.write_index();
            }
            PackedMutation::ZlibStream => {
                let mut offset = first_entry_offset(&self.index_bytes);
                let first = self.pack_bytes[offset];
                offset += 1;
                let mut byte = first;
                while byte & 0x80 != 0 {
                    byte = self.pack_bytes[offset];
                    offset += 1;
                }
                let kind = (first >> 4) & 0x07;
                match kind {
                    6 => loop {
                        let byte = self.pack_bytes[offset];
                        offset += 1;
                        if byte & 0x80 == 0 {
                            break;
                        }
                    },
                    7 => offset += 20,
                    _ => {}
                }
                self.pack_bytes[offset] ^= 0xff;
                self.rebind_pack();
            }
        }
    }

    pub fn resize_index(&mut self, length: u64) {
        make_writable(&self.index_path);
        let file = fs::OpenOptions::new()
            .write(true)
            .open(&self.index_path)
            .expect("open generated index for resize");
        file.set_len(length).expect("resize generated index");
    }

    pub fn resize_pack(&mut self, length: u64) {
        make_writable(&self.pack_path);
        let file = fs::OpenOptions::new()
            .write(true)
            .open(&self.pack_path)
            .expect("open generated pack for resize");
        file.set_len(length).expect("resize generated pack");
    }

    pub fn mutate_delta_base(&mut self, object_id: &str, encoding: DeltaEncoding) {
        let mut offset = entry_offset(&self.index_bytes, object_id)
            .expect("delta target is represented in the index");
        let first = self.pack_bytes[offset];
        offset += 1;
        let mut header = first;
        while header & 0x80 != 0 {
            header = self.pack_bytes[offset];
            offset += 1;
        }
        match encoding {
            DeltaEncoding::Ofs => {
                assert_eq!((first >> 4) & 0x07, 6, "target entry is OFS_DELTA");
                loop {
                    let continuation = self.pack_bytes[offset] & 0x80;
                    self.pack_bytes[offset] = continuation | 0x7f;
                    offset += 1;
                    if continuation == 0 {
                        break;
                    }
                }
            }
            DeltaEncoding::Ref => {
                assert_eq!((first >> 4) & 0x07, 7, "target entry is REF_DELTA");
                self.pack_bytes[offset..offset + 20].fill(0xff);
            }
        }
        self.refresh_entry_crc(object_id);
        self.rebind_pack();
    }

    pub fn mutate_ref_delta_to_self_cycle(&mut self, object_id: &str) {
        let mut offset =
            entry_offset(&self.index_bytes, object_id).expect("REF_DELTA cycle target offset");
        let first = self.pack_bytes[offset];
        offset += 1;
        let mut header = first;
        while header & 0x80 != 0 {
            header = self.pack_bytes[offset];
            offset += 1;
        }
        assert_eq!((first >> 4) & 0x07, 7, "cycle target is REF_DELTA");
        self.pack_bytes[offset..offset + 20].copy_from_slice(&decode_hex_object_id(object_id));
        self.refresh_entry_crc(object_id);
        self.rebind_pack();
    }

    pub fn externalize_ref_base(
        &mut self,
        repository: &MaterializedRepository,
        object_id: &str,
        storage: ExternalBaseStorage,
    ) -> String {
        let base_object_id = ref_delta_base_oid(&self.pack_bytes, &self.index_bytes, object_id);
        self.unpack_objects(repository);
        let loose_base_path =
            loose_object_path(&repository.worktree.join(".git/objects"), &base_object_id);
        assert!(
            loose_base_path.is_file(),
            "unpack-objects did not materialize the REF_DELTA base"
        );

        let base_bytes = decode_hex_object_id(&base_object_id);
        let records = index_records(&self.index_bytes);
        let base_record = records
            .iter()
            .find(|record| record.object_id == base_bytes)
            .expect("REF_DELTA base is represented in the source pack");
        let removed_start = base_record.offset;
        let removed_end = records
            .iter()
            .map(|record| record.offset)
            .filter(|offset| *offset > removed_start)
            .min()
            .unwrap_or(self.pack_bytes.len() - 20);
        let removed_length = removed_end - removed_start;
        self.pack_bytes.drain(removed_start..removed_end);
        let count = read_u32(&self.pack_bytes, 8).expect("pack object count");
        self.pack_bytes[8..12].copy_from_slice(&(count - 1).to_be_bytes());

        let retained_records = records
            .into_iter()
            .filter(|record| record.object_id != base_bytes)
            .map(|mut record| {
                if record.offset > removed_start {
                    record.offset -= removed_length;
                }
                record
            })
            .collect::<Vec<_>>();
        let trailer = self.pack_bytes.len() - 20;
        let digest = sha1(&self.pack_bytes[..trailer]);
        self.pack_bytes[trailer..].copy_from_slice(&digest);
        self.index_bytes = build_index_v2(&retained_records, digest);
        self.install_pair(encode_hex(&digest));

        let global_config = repository.root.join("external-ref-global.gitconfig");
        fs::write(&global_config, []).expect("create external REF Git configuration");
        if matches!(storage, ExternalBaseStorage::Packed) {
            materialize_single_object_pack(repository, &global_config, &base_object_id);
        }
        let mut prune = git_command(&global_config);
        prune
            .arg("-C")
            .arg(&repository.worktree)
            .arg("prune-packed");
        successful_output(prune, None);

        match storage {
            ExternalBaseStorage::Loose => assert!(
                loose_base_path.is_file(),
                "REF_DELTA loose base was pruned despite not being indexed"
            ),
            ExternalBaseStorage::Packed => {
                assert!(
                    !loose_base_path.exists(),
                    "REF_DELTA packed base retained a loose fallback"
                );
                assert_no_loose_objects(&repository.worktree.join(".git/objects"));
            }
        }
        assert!(
            index_row(&self.index_bytes, &base_object_id).is_none(),
            "external REF_DELTA base remained in the target index"
        );
        base_object_id
    }

    fn rebind_pack(&mut self) {
        let trailer = self.pack_bytes.len() - 20;
        let digest = sha1(&self.pack_bytes[..trailer]);
        self.pack_bytes[trailer..].copy_from_slice(&digest);
        let index_pack_checksum = self.index_bytes.len() - 40;
        self.index_bytes[index_pack_checksum..index_pack_checksum + 20].copy_from_slice(&digest);
        refresh_index_checksum(&mut self.index_bytes);
        self.install_pair(encode_hex(&digest));
    }

    fn install_pair(&mut self, new_pack_id: String) {
        let directory = self
            .pack_path
            .parent()
            .expect("pack path directory")
            .to_path_buf();
        let new_pack_path = directory.join(format!("pack-{new_pack_id}.pack"));
        let new_index_path = directory.join(format!("pack-{new_pack_id}.idx"));
        fs::rename(&self.pack_path, &new_pack_path).expect("rename mutated pack");
        fs::rename(&self.index_path, &new_index_path).expect("rename mutated index");
        self.pack_id = new_pack_id;
        self.pack_path = new_pack_path;
        self.index_path = new_index_path;
        self.write_pack();
        self.write_index();
    }

    fn unpack_objects(&self, repository: &MaterializedRepository) {
        let detached_pack = repository.root.join("detached-ref.pack");
        let detached_index = repository.root.join("detached-ref.idx");
        fs::rename(&self.pack_path, &detached_pack).expect("detach REF pack before unpacking");
        fs::rename(&self.index_path, &detached_index).expect("detach REF index before unpacking");

        let global_config = repository.root.join("unpack-ref-global.gitconfig");
        fs::write(&global_config, []).expect("create unpack Git configuration");
        let mut unpack = git_command(&global_config);
        unpack
            .arg("-C")
            .arg(&repository.worktree)
            .arg("unpack-objects");
        successful_output(unpack, Some(&self.pack_bytes));

        fs::rename(detached_pack, &self.pack_path).expect("restore REF pack after unpacking");
        fs::rename(detached_index, &self.index_path).expect("restore REF index after unpacking");
    }

    fn write_pack(&self) {
        make_writable(&self.pack_path);
        fs::write(&self.pack_path, &self.pack_bytes).expect("write mutated pack");
    }

    fn write_index(&self) {
        make_writable(&self.index_path);
        fs::write(&self.index_path, &self.index_bytes).expect("write mutated index");
    }

    fn refresh_entry_crc(&mut self, object_id: &str) {
        let row = index_row(&self.index_bytes, object_id).expect("entry CRC row");
        let start = entry_offset(&self.index_bytes, object_id).expect("entry CRC offset");
        let end = all_entry_offsets(&self.index_bytes)
            .into_iter()
            .filter(|offset| *offset > start)
            .min()
            .unwrap_or(self.pack_bytes.len() - 20);
        let crc = crc32(&self.pack_bytes[start..end]);
        let crc_start = index_crc_start(&self.index_bytes) + row * 4;
        self.index_bytes[crc_start..crc_start + 4].copy_from_slice(&crc.to_be_bytes());
    }
}

pub fn materialize_base_only_pack(repository: &MaterializedRepository) -> PackedMaterialization {
    materialize_base_only_pack_at(&repository.root, &repository.worktree)
}

pub fn materialize_base_only_pack_at(root: &Path, worktree: &Path) -> PackedMaterialization {
    let global_config = root.join("packed-global.gitconfig");
    fs::write(&global_config, []).expect("create packed fixture Git configuration");

    let mut repack = git_command(&global_config);
    repack.arg("-C").arg(worktree).args([
        "repack",
        "-a",
        "-d",
        "--window=0",
        "--depth=0",
        "--no-write-bitmap-index",
    ]);
    successful_output(repack, None);

    let mut prune = git_command(&global_config);
    prune.arg("-C").arg(worktree).arg("prune-packed");
    successful_output(prune, None);

    assert_no_loose_objects(&worktree.join(".git/objects"));

    let pack_directory = worktree.join(".git/objects/pack");
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
        .arg(worktree)
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

pub fn retain_revision(repository: &MaterializedRepository, name: &str, revision: &str) {
    assert!(
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
        "fixture ref name must be a bounded safe token"
    );
    let global_config = repository.root.join("retain-revision-global.gitconfig");
    fs::write(&global_config, []).expect("create retained-revision Git configuration");
    let mut update = git_command(&global_config);
    update.arg("-C").arg(&repository.worktree).args([
        "update-ref",
        &format!("refs/heads/{name}"),
        revision,
    ]);
    successful_output(update, None);
}

pub fn materialize_duplicate_object_pack(
    repository: &MaterializedRepository,
    object_id: &str,
    salt: u64,
) -> PackedMaterialization {
    let global_config = repository
        .root
        .join(format!("duplicate-location-{salt}.gitconfig"));
    fs::write(&global_config, []).expect("create duplicate-location Git configuration");
    let payload = format!("duplicate-location-{salt}\n");
    let mut hash = git_command(&global_config);
    hash.arg("-C")
        .arg(&repository.worktree)
        .args(["hash-object", "-w", "--stdin"]);
    let unique_object_id =
        String::from_utf8(successful_output(hash, Some(payload.as_bytes())).stdout)
            .expect("duplicate-location object ID is UTF-8")
            .trim()
            .to_owned();

    let pack_directory = repository.worktree.join(".git/objects/pack");
    fs::create_dir_all(&pack_directory).expect("create duplicate-location pack directory");
    let mut pack = git_command(&global_config);
    pack.arg("-C")
        .arg(&repository.worktree)
        .args([
            "pack-objects",
            "--quiet",
            "--index-version=2",
            "--window=0",
            "--depth=0",
            "--threads=1",
            "--no-reuse-delta",
            "--no-reuse-object",
            "--no-write-bitmap-index",
        ])
        .arg(pack_directory.join("pack"));
    let object_input = format!("{object_id}\n{unique_object_id}\n");
    let pack_id = String::from_utf8(successful_output(pack, Some(object_input.as_bytes())).stdout)
        .expect("duplicate-location pack ID is UTF-8")
        .trim()
        .to_owned();
    let pack_path = pack_directory.join(format!("pack-{pack_id}.pack"));
    let index_path = pack_directory.join(format!("pack-{pack_id}.idx"));
    let pack_bytes = fs::read(&pack_path).expect("read duplicate-location pack");
    let index_bytes = fs::read(&index_path).expect("read duplicate-location index");
    assert!(
        index_row(&index_bytes, object_id).is_some(),
        "duplicate-location pack omitted the requested object"
    );

    PackedMaterialization {
        index_path,
        pack_id,
        pack_path,
        index_bytes,
        pack_bytes,
    }
}

pub fn generate_delta_candidates(repository: &MaterializedRepository) -> Vec<(String, String)> {
    (0_u64..32)
        .map(|index| {
            let mut bytes = vec![b'a'; 65_536];
            let marker = format!("candidate-{index:02}\n");
            let start = bytes.len() - marker.len();
            bytes[start..].copy_from_slice(marker.as_bytes());
            let commit =
                repository.generated_single_file_commit("payload.txt", &bytes, 978_308_000 + index);
            let global_config = repository.root.join("delta-candidate-global.gitconfig");
            fs::write(&global_config, []).expect("create delta candidate Git configuration");
            let mut tree = git_command(&global_config);
            tree.arg("-C").arg(&repository.worktree).args([
                "ls-tree",
                &commit,
                "--",
                "payload.txt",
            ]);
            let line = String::from_utf8(successful_output(tree, None).stdout)
                .expect("delta candidate ls-tree output is UTF-8");
            let object_id = line
                .split_whitespace()
                .nth(2)
                .expect("delta candidate ls-tree object ID")
                .to_owned();
            (commit, object_id)
        })
        .collect()
}

pub fn materialize_delta_pack(
    repository: &MaterializedRepository,
    candidates: &[(String, String)],
    encoding: DeltaEncoding,
) -> (PackedMaterialization, String) {
    let global_config = repository.root.join("packed-delta-global.gitconfig");
    fs::write(&global_config, []).expect("create packed delta Git configuration");
    let mut enumerate = git_command(&global_config);
    enumerate.arg("-C").arg(&repository.worktree).args([
        "cat-file",
        "--batch-all-objects",
        "--batch-check=%(objectname)",
    ]);
    let object_output = successful_output(enumerate, None);
    let mut object_ids = String::from_utf8(object_output.stdout)
        .expect("batch object IDs are UTF-8")
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    object_ids.sort();
    object_ids.dedup();
    let object_input = format!("{}\n", object_ids.join("\n"));

    let pack_directory = repository.worktree.join(".git/objects/pack");
    fs::create_dir_all(&pack_directory).expect("create delta pack directory");
    let base_name = pack_directory.join("pack");
    let mut pack = git_command(&global_config);
    pack.arg("-C").arg(&repository.worktree).args([
        "pack-objects",
        "--quiet",
        "--index-version=2",
        "--window=64",
        "--depth=50",
        "--threads=1",
        "--no-reuse-delta",
        "--no-reuse-object",
        "--no-write-bitmap-index",
    ]);
    match encoding {
        DeltaEncoding::Ofs => {
            pack.arg("--delta-base-offset");
        }
        DeltaEncoding::Ref => {
            pack.arg("--no-delta-base-offset");
        }
    }
    pack.arg(&base_name);
    let pack_id = String::from_utf8(successful_output(pack, Some(object_input.as_bytes())).stdout)
        .expect("pack-objects output is UTF-8")
        .trim()
        .to_owned();
    let pack_path = pack_directory.join(format!("pack-{pack_id}.pack"));
    let index_path = pack_directory.join(format!("pack-{pack_id}.idx"));
    let pack_bytes = fs::read(&pack_path).expect("read generated delta pack");
    let index_bytes = fs::read(&index_path).expect("read generated delta index");
    let expected_type = match encoding {
        DeltaEncoding::Ofs => 6,
        DeltaEncoding::Ref => 7,
    };
    let selected_commit = candidates
        .iter()
        .find_map(|(commit, object_id)| {
            (packed_entry_type(&pack_bytes, &index_bytes, object_id) == Some(expected_type))
                .then(|| commit.clone())
        })
        .expect("at least one reachable candidate blob uses the requested delta encoding");

    let mut prune = git_command(&global_config);
    prune
        .arg("-C")
        .arg(&repository.worktree)
        .arg("prune-packed");
    successful_output(prune, None);
    assert_no_loose_objects(&repository.worktree.join(".git/objects"));

    let mut verify = git_command(&global_config);
    verify
        .arg("-C")
        .arg(&repository.worktree)
        .args(["verify-pack", "-v"])
        .arg(&index_path);
    successful_output(verify, None);

    (
        PackedMaterialization {
            index_path,
            pack_id,
            pack_path,
            index_bytes,
            pack_bytes,
        },
        selected_commit,
    )
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

pub fn offline_verify_pack(
    repository: &MaterializedRepository,
    packed: &PackedMaterialization,
) -> Vec<u8> {
    let global_config = repository
        .root
        .join("offline-differential-global.gitconfig");
    fs::write(&global_config, []).expect("create differential Git configuration");
    let mut verify = git_command(&global_config);
    verify
        .arg("-C")
        .arg(&repository.worktree)
        .args(["verify-pack", "-v"])
        .arg(&packed.index_path);
    successful_output(verify, None).stdout
}

fn materialize_single_object_pack(
    repository: &MaterializedRepository,
    global_config: &Path,
    object_id: &str,
) {
    let pack_directory = repository.worktree.join(".git/objects/pack");
    let base_name = pack_directory.join("pack");
    let mut pack = git_command(global_config);
    pack.arg("-C")
        .arg(&repository.worktree)
        .args([
            "pack-objects",
            "--quiet",
            "--index-version=2",
            "--window=0",
            "--depth=0",
            "--threads=1",
            "--no-reuse-delta",
            "--no-reuse-object",
            "--no-write-bitmap-index",
        ])
        .arg(&base_name);
    let object_input = format!("{object_id}\n");
    let pack_id = String::from_utf8(successful_output(pack, Some(object_input.as_bytes())).stdout)
        .expect("external-base pack ID is UTF-8")
        .trim()
        .to_owned();
    assert!(
        pack_directory
            .join(format!("pack-{pack_id}.pack"))
            .is_file(),
        "external-base pack was not created"
    );
    let index = fs::read(pack_directory.join(format!("pack-{pack_id}.idx")))
        .expect("read external-base index");
    assert_eq!(
        index_object_count(&index),
        1,
        "external-base pack must contain exactly one object"
    );
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

fn ref_delta_base_oid(pack: &[u8], index: &[u8], object_id: &str) -> String {
    let mut offset = entry_offset(index, object_id).expect("REF_DELTA target offset");
    let first = pack[offset];
    offset += 1;
    let mut header = first;
    while header & 0x80 != 0 {
        header = pack[offset];
        offset += 1;
    }
    assert_eq!((first >> 4) & 0x07, 7, "target entry is REF_DELTA");
    encode_hex(&pack[offset..offset + 20])
}

fn loose_object_path(objects_directory: &Path, object_id: &str) -> PathBuf {
    objects_directory
        .join(&object_id[..2])
        .join(&object_id[2..])
}

fn index_records(index: &[u8]) -> Vec<IndexRecord> {
    let count = index_object_count(index);
    let oid_start = 8 + 256 * 4;
    let crc_start = index_crc_start(index);
    let offsets = all_entry_offsets(index);
    (0..count)
        .map(|row| IndexRecord {
            object_id: index[oid_start + row * 20..oid_start + (row + 1) * 20]
                .try_into()
                .expect("index object ID"),
            crc32: read_u32(index, crc_start + row * 4).expect("index CRC row"),
            offset: offsets[row],
        })
        .collect()
}

fn build_index_v2(records: &[IndexRecord], pack_checksum: [u8; 20]) -> Vec<u8> {
    assert!(
        records
            .windows(2)
            .all(|pair| pair[0].object_id < pair[1].object_id),
        "rebuilt index object IDs must remain strictly ordered"
    );
    assert!(
        records.iter().all(|record| record.offset <= 0x7fff_ffff),
        "bounded fixture uses only 32-bit pack offsets"
    );

    let mut fanout = [0_u32; 256];
    for record in records {
        fanout[usize::from(record.object_id[0])] += 1;
    }
    let mut cumulative = 0_u32;
    for value in &mut fanout {
        cumulative = cumulative.checked_add(*value).expect("index fanout count");
        *value = cumulative;
    }

    let mut index = Vec::with_capacity(8 + 256 * 4 + records.len() * 28 + 40);
    index.extend_from_slice(&[0xff, 0x74, 0x4f, 0x63]);
    index.extend_from_slice(&2_u32.to_be_bytes());
    for value in fanout {
        index.extend_from_slice(&value.to_be_bytes());
    }
    for record in records {
        index.extend_from_slice(&record.object_id);
    }
    for record in records {
        index.extend_from_slice(&record.crc32.to_be_bytes());
    }
    for record in records {
        index.extend_from_slice(
            &u32::try_from(record.offset)
                .expect("fixture pack offset fits u32")
                .to_be_bytes(),
        );
    }
    index.extend_from_slice(&pack_checksum);
    let checksum = sha1(&index);
    index.extend_from_slice(&checksum);
    index
}

fn decode_hex_object_id(object_id: &str) -> [u8; 20] {
    assert_eq!(object_id.len(), 40, "SHA-1 object ID length");
    let mut bytes = [0_u8; 20];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&object_id[index * 2..index * 2 + 2], 16)
            .expect("SHA-1 object ID is hexadecimal");
    }
    bytes
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

fn packed_entry_type(pack: &[u8], index: &[u8], object_id: &str) -> Option<u8> {
    if index.get(..4)? != [0xff, 0x74, 0x4f, 0x63] || read_u32(index, 4)? != 2 {
        return None;
    }
    let object_count = usize::try_from(read_u32(index, 8 + 255 * 4)?).ok()?;
    let row = index_row(index, object_id)?;
    let offset_start = 8 + 256 * 4 + object_count * 20 + object_count * 4;
    let offset_word = read_u32(index, offset_start + row * 4)?;
    let offset = if offset_word & 0x8000_0000 == 0 {
        u64::from(offset_word)
    } else {
        let large_start = offset_start + object_count * 4;
        let slot = usize::try_from(offset_word & 0x7fff_ffff).ok()?;
        read_u64(index, large_start + slot * 8)?
    };
    let offset = usize::try_from(offset).ok()?;
    Some((pack.get(offset)? >> 4) & 0x07)
}

fn entry_offset(index: &[u8], object_id: &str) -> Option<usize> {
    let object_count = index_object_count(index);
    let row = index_row(index, object_id)?;
    let offset_start = 8 + 256 * 4 + object_count * 20 + object_count * 4;
    let offset_word = read_u32(index, offset_start + row * 4)?;
    if offset_word & 0x8000_0000 == 0 {
        usize::try_from(offset_word).ok()
    } else {
        let large_start = offset_start + object_count * 4;
        let slot = usize::try_from(offset_word & 0x7fff_ffff).ok()?;
        usize::try_from(read_u64(index, large_start + slot * 8)?).ok()
    }
}

fn all_entry_offsets(index: &[u8]) -> Vec<usize> {
    let count = index_object_count(index);
    let offset_start = index_offset_start(index);
    let large_start = offset_start + count * 4;
    (0..count)
        .map(|row| {
            let value = read_u32(index, offset_start + row * 4).expect("index offset row");
            if value & 0x8000_0000 == 0 {
                usize::try_from(value).expect("small pack offset")
            } else {
                let slot = usize::try_from(value & 0x7fff_ffff).expect("large offset slot");
                usize::try_from(
                    read_u64(index, large_start + slot * 8).expect("large offset value"),
                )
                .expect("large offset fits usize")
            }
        })
        .collect()
}

fn index_row(index: &[u8], object_id: &str) -> Option<usize> {
    let object_count = index_object_count(index);
    let oid_start = 8 + 256 * 4;
    (0..object_count).find(|row| {
        let start = oid_start + row * 20;
        index
            .get(start..start + 20)
            .is_some_and(|bytes| encode_hex(bytes) == object_id)
    })
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_be_bytes(
        bytes.get(offset..offset + 8)?.try_into().ok()?,
    ))
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("write object ID");
    }
    output
}

fn index_object_count(index: &[u8]) -> usize {
    usize::try_from(read_u32(index, 8 + 255 * 4).expect("index object count"))
        .expect("index object count fits usize")
}

fn index_crc_start(index: &[u8]) -> usize {
    8 + 256 * 4 + index_object_count(index) * 20
}

fn index_offset_start(index: &[u8]) -> usize {
    index_crc_start(index) + index_object_count(index) * 4
}

fn first_entry_offset(index: &[u8]) -> usize {
    let offset = read_u32(index, index_offset_start(index)).expect("first index offset");
    assert_eq!(offset & 0x8000_0000, 0, "small fixture uses 32-bit offsets");
    usize::try_from(offset).expect("entry offset fits usize")
}

fn refresh_index_checksum(index: &mut [u8]) {
    let checksum = index.len() - 20;
    let digest = sha1(&index[..checksum]);
    index[checksum..].copy_from_slice(&digest);
}

#[allow(clippy::many_single_char_names)]
fn sha1(bytes: &[u8]) -> [u8; 20] {
    let bit_length = u64::try_from(bytes.len())
        .expect("fixture bytes fit u64")
        .checked_mul(8)
        .expect("fixture bit length");
    let mut message = bytes.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());
    let mut state = [
        0x6745_2301_u32,
        0xefcd_ab89,
        0x98ba_dcfe,
        0x1032_5476,
        0xc3d2_e1f0,
    ];
    for block in message.chunks_exact(64) {
        let mut words = [0_u32; 80];
        for (index, word) in words[..16].iter_mut().enumerate() {
            *word = u32::from_be_bytes(
                block[index * 4..index * 4 + 4]
                    .try_into()
                    .expect("SHA-1 word"),
            );
        }
        for index in 16..80 {
            words[index] =
                (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                    .rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = state;
        for (index, word) in words.iter().enumerate() {
            let (function, constant) = match index {
                0..=19 => ((b & c) | ((!b) & d), 0x5a82_7999),
                20..=39 => (b ^ c ^ d, 0x6ed9_eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
                _ => (b ^ c ^ d, 0xca62_c1d6),
            };
            let next = a
                .rotate_left(5)
                .wrapping_add(function)
                .wrapping_add(e)
                .wrapping_add(constant)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = next;
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
    }
    let mut digest = [0_u8; 20];
    for (index, word) in state.iter().enumerate() {
        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ 0xedb8_8320
            };
        }
    }
    !crc
}

fn make_writable(path: &Path) {
    let mut permissions = fs::metadata(path)
        .expect("read generated pack permissions")
        .permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        permissions.set_mode(permissions.mode() | 0o200);
    }
    #[cfg(windows)]
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions).expect("make generated pack writable");
}
