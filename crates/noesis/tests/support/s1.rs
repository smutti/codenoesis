use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::{git_command, read_repository_text, stdout_line, successful_output, unique_temp_root};

pub const COMMIT_A_OID: &str = "a72b34a03936b70511bd72bd4fa0d37a5a593386";
pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s1-safe-inventory-v1";
pub const TREE_A_OID: &str = "2a77b46a6a4a7645b7ec63831ce7e46bd7921904";

const FILES: [(&str, &str, &str); 9] = [
    (
        ".github/CODEOWNERS",
        "100644",
        "e2d63062cf5b70d85dfa800ba4acadc7b3a78262",
    ),
    (
        "Cargo.toml",
        "100644",
        "f2c75744ba1623ebd2bf64e8dda36fdb0fa86ab6",
    ),
    (
        "README.md",
        "100644",
        "bea61f8462cb66bab5eb5dbdc74b8054d0c0f116",
    ),
    (
        "api/openapi.yaml",
        "100644",
        "c55d01ab6f19f6d6d6ee6e81d40a65d73733c995",
    ),
    (
        "assets/payload.bin",
        "100644",
        "de75dc54dee262791dc95eca71814a74aae4bbce",
    ),
    (
        "build.rs",
        "100644",
        "0ef4746c8ff5e7e129364c7c88be39742ac195e2",
    ),
    (
        "rustfmt.toml",
        "100644",
        "ed49ca7b3d9a10e8bc0c7337d85ff497e83cc3bf",
    ),
    (
        "src/lib.rs",
        "100644",
        "432593368f9028b66c60c5f011dd54949c10ca81",
    ),
    (
        "tools/sentinel.sh",
        "100755",
        "9e02753a1d6a2f3d6825bb6e7107cecb1ad7e1c6",
    ),
];

pub struct MaterializedRepository {
    pub root: PathBuf,
    pub worktree: PathBuf,
    global_config: PathBuf,
}

impl MaterializedRepository {
    pub fn revision_a() -> Self {
        let root = unique_temp_root();
        let worktree = root.join("repository");
        let template = root.join("template");
        let global_config = root.join("global.gitconfig");
        fs::create_dir_all(&template).expect("create empty Git template directory");
        fs::write(&global_config, []).expect("create empty global Git configuration");

        let mut init = git_command(&global_config);
        init.args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&worktree);
        successful_output(init, None);

        for (path, _, expected_oid) in FILES {
            let bytes = read_fixture_source(path);
            let mut hash_blob = git_command(&global_config);
            hash_blob
                .arg("-C")
                .arg(&worktree)
                .args(["hash-object", "-w", "--stdin"]);
            let observed_oid = stdout_line(successful_output(hash_blob, Some(&bytes)));
            assert_eq!(
                observed_oid, expected_oid,
                "S1 fixture blob identity changed for {path}"
            );
        }

        let root_tree = write_revision_a_tree(&worktree, &global_config);
        assert_eq!(root_tree, TREE_A_OID, "S1 root tree identity changed");

        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&worktree)
            .args(["commit-tree", TREE_A_OID, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "978307200 +0000")
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "978307200 +0000");
        let commit_oid = stdout_line(successful_output(
            make_commit,
            Some(b"fixture: S1 revision A\n"),
        ));
        assert_eq!(commit_oid, COMMIT_A_OID, "S1 commit identity changed");

        let mut update_ref = git_command(&global_config);
        update_ref
            .arg("-C")
            .arg(&worktree)
            .args(["update-ref", "refs/heads/main", COMMIT_A_OID]);
        successful_output(update_ref, None);

        Self {
            root,
            worktree,
            global_config,
        }
    }

    pub fn traversal_commit(&self) -> &'static str {
        let blob_oid = self.write_blob(b"outside\n");
        assert_eq!(
            blob_oid, "06d10a57a75dc0d5d1fd0fb2df7ec6fbe9c6ddaa",
            "S1 traversal blob identity changed"
        );
        let mut tree_body = raw_tree_entry("100644", "..", &blob_oid);
        tree_body.extend_from_slice(&raw_tree_entry(
            "40000",
            ".github",
            "5f093732bf441a06ff1af692b108674a9e585f31",
        ));
        tree_body.extend_from_slice(&raw_tree_entry(
            "100644",
            "Cargo.toml",
            "f2c75744ba1623ebd2bf64e8dda36fdb0fa86ab6",
        ));
        tree_body.extend_from_slice(&raw_tree_entry(
            "100644",
            "README.md",
            "bea61f8462cb66bab5eb5dbdc74b8054d0c0f116",
        ));
        tree_body.extend_from_slice(&raw_tree_entry(
            "40000",
            "api",
            "b6aef728b9c98d60001c3c02ed8b18a907a0e266",
        ));
        tree_body.extend_from_slice(&raw_tree_entry(
            "40000",
            "assets",
            "5507e3f2d1cb90e4751ab11bb1896d46c9ec1840",
        ));
        tree_body.extend_from_slice(&raw_tree_entry(
            "100644",
            "build.rs",
            "0ef4746c8ff5e7e129364c7c88be39742ac195e2",
        ));
        tree_body.extend_from_slice(&raw_tree_entry(
            "100644",
            "rustfmt.toml",
            "ed49ca7b3d9a10e8bc0c7337d85ff497e83cc3bf",
        ));
        tree_body.extend_from_slice(&raw_tree_entry(
            "40000",
            "src",
            "f41cdbd8214aeb85cd9b1eb07d45308fed184f9d",
        ));
        tree_body.extend_from_slice(&raw_tree_entry(
            "40000",
            "tools",
            "197d47856ba9946649aa3999766a77ea8537703a",
        ));
        let tree_oid = self.write_raw_tree(&tree_body);
        assert_eq!(
            tree_oid, "ea634ab609ef56e15b767dde57f22b1fac30cee7",
            "S1 traversal tree identity changed"
        );
        self.write_commit(
            &tree_oid,
            "978307201 +0000",
            b"fixture: S1 traversal\n",
            "65068ecfe754607d23d2e7efe6d5619aca64c7c7",
        )
    }

    pub fn symlink_escape_commit(&self) -> &'static str {
        let blob_oid = self.write_blob(b"../outside-sentinel");
        assert_eq!(
            blob_oid, "e6d8ca6033a7357250f66a9df966a29504a33579",
            "S1 symlink blob identity changed"
        );
        let tree_oid = self.write_tree(
            &format!(
                "{}120000 blob {blob_oid}\tescape\n{}",
                base_root_entries_before("escape"),
                base_root_entries_after("escape")
            ),
            false,
        );
        assert_eq!(
            tree_oid, "bd686e43a7cc45c408ed2df94e29621c88fe5a68",
            "S1 symlink tree identity changed"
        );
        self.write_commit(
            &tree_oid,
            "978307202 +0000",
            b"fixture: S1 symlink_escape\n",
            "5db07d07ce15c78e21d477598ab46a2f189520bc",
        )
    }

    pub fn gitlink_commit(&self) -> &'static str {
        let tree_oid = self.write_tree(
            &format!(
                "{}160000 commit 1111111111111111111111111111111111111111\tvendor\n",
                base_root_entries()
            ),
            true,
        );
        assert_eq!(
            tree_oid, "535b883084a107924f2c3a8e9c0731e5c2459946",
            "S1 gitlink tree identity changed"
        );
        self.write_commit(
            &tree_oid,
            "978307204 +0000",
            b"fixture: S1 gitlink\n",
            "53833f243be61158b99f862915f9ace009962aa7",
        )
    }

    pub fn single_file_at_limit_commit(&self) -> &'static str {
        self.single_file_limit_commit(
            4_194_304,
            "978307205 +0000",
            b"fixture: S1 file bytes 4194304\n",
            "e32d5a9747f1e4bb06757725c1bb347d5e5b04da",
            "c16764bdde430cc6512fc3b7b4e4b7dd83e797b2",
            "529cf9c6d307151fb01d2f7499641d207c371761",
        )
    }

    pub fn single_file_over_limit_commit(&self) -> &'static str {
        self.single_file_limit_commit(
            4_194_305,
            "978307206 +0000",
            b"fixture: S1 file bytes 4194305\n",
            "d67fcb8ab2ad693a0b65d6ec0559ffe487b2e721",
            "2cad2243340438456423a8af3ba9eb3451154157",
            "e02e36b7c78758b78e810af34198410633006968",
        )
    }

    pub fn generated_single_file_commit(&self, path: &str, bytes: &[u8], timestamp: u64) -> String {
        let blob_oid = self.write_blob(bytes);
        let tree_oid = self.write_tree(&format!("100644 blob {blob_oid}\t{path}\n"), false);
        self.write_generated_commit(
            &tree_oid,
            timestamp,
            format!("fixture: generated S1 file {path}\n").as_bytes(),
        )
    }

    pub fn tree_fanout_over_limit_commit(&self) -> String {
        let empty_tree = self.write_tree("", false);
        let mut entries = String::with_capacity(1_700_000);
        for index in 0..=25_000 {
            use std::fmt::Write as _;

            writeln!(entries, "040000 tree {empty_tree}\tdirectory-{index:05}")
                .expect("write fanout tree entry");
        }
        let tree_oid = self.write_tree(&entries, false);
        self.write_generated_commit(&tree_oid, 978_307_210, b"fixture: S1 tree fanout 25001\n")
    }

    pub fn apply_isolation_variant(&self, sentinel: &Path) {
        let hook = self.worktree.join(".git/hooks/post-checkout");
        fs::create_dir_all(hook.parent().expect("hook parent"))
            .expect("create S1 isolation hook directory");
        fs::write(
            &hook,
            "#!/bin/sh\nprintf 'hook-executed\\n' > \"$CODENOESIS_SENTINEL\"\n",
        )
        .expect("write S1 isolation sentinel hook");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))
                .expect("make S1 isolation sentinel executable");
        }
        let mut set_remote = git_command(&self.global_config);
        set_remote.arg("-C").arg(&self.worktree).args([
            "config",
            "remote.origin.url",
            "https://127.0.0.1:9/codenoesis-sentinel.git",
        ]);
        successful_output(set_remote, None);
        assert!(
            sentinel.exists(),
            "outside-root sentinel must be pre-created"
        );
    }

    fn single_file_limit_commit(
        &self,
        byte_length: usize,
        timestamp: &str,
        message: &[u8],
        expected_blob: &str,
        expected_tree: &str,
        expected_commit: &'static str,
    ) -> &'static str {
        let blob_oid = self.write_blob(&vec![b'a'; byte_length]);
        assert_eq!(blob_oid, expected_blob, "S1 limit blob identity changed");
        let tree_oid = self.write_tree(&format!("100644 blob {blob_oid}\toversized.bin\n"), false);
        assert_eq!(tree_oid, expected_tree, "S1 limit tree identity changed");
        self.write_commit(&tree_oid, timestamp, message, expected_commit)
    }

    fn write_blob(&self, bytes: &[u8]) -> String {
        let mut command = git_command(&self.global_config);
        command
            .arg("-C")
            .arg(&self.worktree)
            .args(["hash-object", "-w", "--stdin"]);
        stdout_line(successful_output(command, Some(bytes)))
    }

    fn write_raw_tree(&self, bytes: &[u8]) -> String {
        let mut command = git_command(&self.global_config);
        command.arg("-C").arg(&self.worktree).args([
            "hash-object",
            "-w",
            "--stdin",
            "-t",
            "tree",
            "--literally",
        ]);
        stdout_line(successful_output(command, Some(bytes)))
    }

    fn write_tree(&self, input: &str, allow_missing: bool) -> String {
        let mut command = git_command(&self.global_config);
        command.arg("-C").arg(&self.worktree).arg("mktree");
        if allow_missing {
            command.arg("--missing");
        }
        stdout_line(successful_output(command, Some(input.as_bytes())))
    }

    fn write_commit(
        &self,
        tree_oid: &str,
        timestamp: &str,
        message: &[u8],
        expected_commit: &'static str,
    ) -> &'static str {
        let mut command = git_command(&self.global_config);
        command
            .arg("-C")
            .arg(&self.worktree)
            .args(["commit-tree", tree_oid, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", timestamp)
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", timestamp);
        let observed = stdout_line(successful_output(command, Some(message)));
        assert_eq!(
            observed, expected_commit,
            "S1 variant commit identity changed"
        );
        expected_commit
    }

    fn write_generated_commit(&self, tree_oid: &str, timestamp: u64, message: &[u8]) -> String {
        let mut command = git_command(&self.global_config);
        command
            .arg("-C")
            .arg(&self.worktree)
            .args(["commit-tree", tree_oid, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", format!("{timestamp} +0000"))
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", format!("{timestamp} +0000"));
        stdout_line(successful_output(command, Some(message)))
    }
}

impl Drop for MaterializedRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s1/safe-inventory-v1")
}

fn read_fixture_source(path: &str) -> Vec<u8> {
    let source = fixture_root().join("revision-a").join(path);
    if path == "assets/payload.bin" {
        fs::read(&source).unwrap_or_else(|error| panic!("read S1 fixture source {path}: {error}"))
    } else {
        read_repository_text(source)
    }
}

pub fn scan(repository: &Path, revision: &str) -> Output {
    scan_command(repository, revision)
        .output()
        .expect("launch S1 noesis scan")
}

pub fn scan_command(repository: &Path, revision: &str) -> Command {
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
            "--format",
            "json",
        ]);
    command
}

fn tree_entry(path: &str) -> String {
    let (_, mode, oid) = FILES
        .iter()
        .find(|(candidate, _, _)| *candidate == path)
        .unwrap_or_else(|| panic!("missing S1 fixture tree entry for {path}"));
    let name = path.rsplit('/').next().expect("fixture path basename");
    format!("{mode} blob {oid}\t{name}\n")
}

fn write_tree(worktree: &Path, global_config: &Path, entries: &[String]) -> String {
    let input = entries.concat();
    let mut make_tree = git_command(global_config);
    make_tree.arg("-C").arg(worktree).arg("mktree");
    stdout_line(successful_output(make_tree, Some(input.as_bytes())))
}

fn write_revision_a_tree(worktree: &Path, global_config: &Path) -> String {
    let trees = [
        (
            ".github",
            ".github/CODEOWNERS",
            "5f093732bf441a06ff1af692b108674a9e585f31",
        ),
        (
            "api",
            "api/openapi.yaml",
            "b6aef728b9c98d60001c3c02ed8b18a907a0e266",
        ),
        (
            "assets",
            "assets/payload.bin",
            "5507e3f2d1cb90e4751ab11bb1896d46c9ec1840",
        ),
        (
            "src",
            "src/lib.rs",
            "f41cdbd8214aeb85cd9b1eb07d45308fed184f9d",
        ),
        (
            "tools",
            "tools/sentinel.sh",
            "197d47856ba9946649aa3999766a77ea8537703a",
        ),
    ];
    for (_, path, expected_oid) in trees {
        assert_eq!(
            write_tree(worktree, global_config, &[tree_entry(path)]),
            expected_oid,
            "S1 subtree identity changed for {path}"
        );
    }
    write_tree(
        worktree,
        global_config,
        &[
            format!("040000 tree {}\t.github\n", trees[0].2),
            tree_entry("Cargo.toml"),
            tree_entry("README.md"),
            format!("040000 tree {}\tapi\n", trees[1].2),
            format!("040000 tree {}\tassets\n", trees[2].2),
            tree_entry("build.rs"),
            tree_entry("rustfmt.toml"),
            format!("040000 tree {}\tsrc\n", trees[3].2),
            format!("040000 tree {}\ttools\n", trees[4].2),
        ],
    )
}

fn decode_oid(value: &str) -> [u8; 20] {
    let mut bytes = [0_u8; 20];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = u8::from_str_radix(&value[offset..offset + 2], 16)
            .expect("fixture object ID is lowercase hexadecimal");
    }
    bytes
}

fn raw_tree_entry(mode: &str, name: &str, object_id: &str) -> Vec<u8> {
    let mut entry = format!("{mode} {name}\0").into_bytes();
    entry.extend_from_slice(&decode_oid(object_id));
    entry
}

fn base_root_entries() -> String {
    [
        "040000 tree 5f093732bf441a06ff1af692b108674a9e585f31\t.github\n",
        "100644 blob f2c75744ba1623ebd2bf64e8dda36fdb0fa86ab6\tCargo.toml\n",
        "100644 blob bea61f8462cb66bab5eb5dbdc74b8054d0c0f116\tREADME.md\n",
        "040000 tree b6aef728b9c98d60001c3c02ed8b18a907a0e266\tapi\n",
        "040000 tree 5507e3f2d1cb90e4751ab11bb1896d46c9ec1840\tassets\n",
        "100644 blob 0ef4746c8ff5e7e129364c7c88be39742ac195e2\tbuild.rs\n",
        "100644 blob ed49ca7b3d9a10e8bc0c7337d85ff497e83cc3bf\trustfmt.toml\n",
        "040000 tree f41cdbd8214aeb85cd9b1eb07d45308fed184f9d\tsrc\n",
        "040000 tree 197d47856ba9946649aa3999766a77ea8537703a\ttools\n",
    ]
    .concat()
}

fn base_root_entries_before(name: &str) -> String {
    filtered_base_root_entries(name, std::cmp::Ordering::Less)
}

fn base_root_entries_after(name: &str) -> String {
    filtered_base_root_entries(name, std::cmp::Ordering::Greater)
}

fn filtered_base_root_entries(name: &str, ordering: std::cmp::Ordering) -> String {
    let mut output = String::new();
    for line in base_root_entries().lines() {
        if tree_entry_name(line).as_bytes().cmp(name.as_bytes()) == ordering {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

fn tree_entry_name(line: &str) -> &str {
    line.split_once('\t')
        .map(|(_, name)| name)
        .expect("fixture mktree line contains a tab")
}
