use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::{git_command, read_repository_text, stdout_line, successful_output};

pub const ROOT_REPOSITORY_ID: &str = "urn:codenoesis:fixture:s1-gitlink-boundary-v1";
pub const ROOT_COMMIT_OID: &str = "e3a117e190a92585bcae4c49c775d310243107e7";
pub const ROOT_TREE_OID: &str = "81f40228aa5c1e9dab33d6cc3d5d90f14aeb7d2e";
pub const GITMODULES_BLOB_OID: &str = "204d65a4ed6a58dd265349f8a1579a4522dc4f7d";
pub const GITLINK_OID: &str = "6ecf94267842da776e35406a9ebcb85e058a3181";

const ROOT_MANIFEST_BLOB_OID: &str = "9a61bc964dd0b80e54880d0a40471b50324b6e15";
const CRATES_TREE_OID: &str = "3e03df22f6961d79f831318ae35e06fa026ad599";
const EXTERNAL_TREE_OID: &str = "3ee7026beba4578478d73788368adc793e975ca0";

pub struct MaterializedBoundaryRepository {
    pub base: super::s4::MaterializedRepository,
    pub commit_oid: String,
}

impl MaterializedBoundaryRepository {
    pub fn unbound() -> Self {
        let base = super::s4::MaterializedRepository::revision_a();
        let global_config = base.root.join("global.gitconfig");
        let gitmodules = read_repository_text(fixture_root().join("revision-overlay/.gitmodules"));

        let mut hash_gitmodules = git_command(&global_config);
        hash_gitmodules
            .arg("-C")
            .arg(&base.worktree)
            .args(["hash-object", "-w", "--stdin"]);
        let gitmodules_oid = stdout_line(successful_output(hash_gitmodules, Some(&gitmodules)));
        assert_eq!(
            gitmodules_oid, GITMODULES_BLOB_OID,
            "R2 .gitmodules blob identity changed"
        );

        let external_tree = write_tree(
            &base.worktree,
            &global_config,
            &format!("160000 commit {GITLINK_OID}\tnested-model\n"),
            true,
        );
        assert_eq!(
            external_tree, EXTERNAL_TREE_OID,
            "R2 external tree identity changed"
        );

        let root_tree = write_tree(
            &base.worktree,
            &global_config,
            &format!(
                "100644 blob {GITMODULES_BLOB_OID}\t.gitmodules\n\
                 100644 blob {ROOT_MANIFEST_BLOB_OID}\tCargo.toml\n\
                 040000 tree {CRATES_TREE_OID}\tcrates\n\
                 040000 tree {EXTERNAL_TREE_OID}\texternal\n"
            ),
            false,
        );
        assert_eq!(root_tree, ROOT_TREE_OID, "R2 root tree identity changed");

        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&base.worktree)
            .args(["commit-tree", ROOT_TREE_OID, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "1785628800 +0000")
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "1785628800 +0000");
        let commit_oid = stdout_line(successful_output(
            make_commit,
            Some(b"CodeNoesis R2 gitlink boundary fixture\n"),
        ));
        assert_eq!(
            commit_oid, ROOT_COMMIT_OID,
            "R2 root commit identity changed"
        );

        let mut update_ref = git_command(&global_config);
        update_ref.arg("-C").arg(&base.worktree).args([
            "update-ref",
            "refs/heads/main",
            ROOT_COMMIT_OID,
        ]);
        successful_output(update_ref, None);

        Self {
            base,
            commit_oid: ROOT_COMMIT_OID.to_owned(),
        }
    }

    pub fn scan_unbound(&self) -> Output {
        self.scan_command(&self.base.store, None)
            .output()
            .expect("launch R2 boundary subject")
    }

    pub fn scan_with_manifest(&self, manifest: &Path) -> Output {
        self.scan_command(&self.base.store, Some(manifest))
            .output()
            .expect("launch R2 bound subject")
    }

    pub fn scan_to_store(&self, store: &Path, manifest: Option<&Path>) -> Output {
        self.scan_command(store, manifest)
            .output()
            .expect("launch R2 boundary subject")
    }

    pub fn scan_command(&self, store: &Path, manifest: Option<&Path>) -> Command {
        scan_command(&self.base.worktree, store, &self.commit_oid, manifest)
    }

    pub fn copy_manifest(&self, name: &str) -> PathBuf {
        let destination = self.base.root.join(name);
        fs::copy(fixture_root().join(name), &destination).expect("copy R2 boundary input");
        destination
    }

    pub fn write_matching_manifest(&self, packed: bool) -> PathBuf {
        let mut manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(fixture_root().join("boundary-input-matching.json"))
                .expect("read matching R2 input"),
        )
        .expect("parse matching R2 input");
        if packed {
            manifest["nested_repositories"][0]["acquisition_profile"] =
                serde_json::Value::String("local-git-sha1-packed-v1".to_owned());
        }
        let destination = self.base.root.join(if packed {
            "boundary-input-packed.json"
        } else {
            "boundary-input-matching.json"
        });
        fs::write(&destination, serde_json::to_vec(&manifest).unwrap())
            .expect("write runtime R2 input");
        destination
    }

    pub fn materialize_nested(&self, relative: &str) -> PathBuf {
        let root = self.base.root.join(relative);
        fs::create_dir_all(root.parent().expect("nested root parent"))
            .expect("create nested root parent");
        let template = self
            .base
            .root
            .join(format!("nested-template-{}", relative.replace('/', "-")));
        fs::create_dir_all(&template).expect("create nested Git template");
        let global_config = self.base.root.join("global.gitconfig");
        let mut init = git_command(&global_config);
        init.args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&root);
        successful_output(init, None);

        let mut make_tree = git_command(&global_config);
        make_tree.arg("-C").arg(&root).arg("mktree");
        let tree_oid = stdout_line(successful_output(make_tree, Some(b"")));
        assert_eq!(tree_oid, "4b825dc642cb6eb9a060e54bf8d69288fbee4904");
        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&root)
            .args(["commit-tree", &tree_oid, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "1785628800 +0000")
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "1785628800 +0000");
        let commit_oid = stdout_line(successful_output(
            make_commit,
            Some(b"CodeNoesis R2 empty nested fixture\n"),
        ));
        assert_eq!(commit_oid, GITLINK_OID);
        let mut update_ref = git_command(&global_config);
        update_ref
            .arg("-C")
            .arg(&root)
            .args(["update-ref", "refs/heads/main", &commit_oid]);
        successful_output(update_ref, None);
        root
    }

    pub fn repack_nested(&self, root: &Path) {
        let global_config = self.base.root.join("global.gitconfig");
        let mut repack = git_command(&global_config);
        repack.arg("-C").arg(root).args(["repack", "-ad"]);
        successful_output(repack, None);
    }

    pub fn repack_root(&self) {
        let global_config = self.base.root.join("global.gitconfig");
        let mut repack = git_command(&global_config);
        repack
            .arg("-C")
            .arg(&self.base.worktree)
            .args(["repack", "-ad"]);
        successful_output(repack, None);
    }

    pub fn scan_packed_root(&self, store: &Path) -> Output {
        let mut command = self.scan_command(store, None);
        command.args(["--acquisition-profile", "local-git-sha1-packed-v1"]);
        command.output().expect("launch packed-root R2 scan")
    }

    pub fn docs(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["docs", "--store"])
            .arg(&self.base.store)
            .args(["--repository-id", ROOT_REPOSITORY_ID, "--output"])
            .arg(&self.base.documents)
            .args(["--format", "json"])
            .output()
            .expect("launch R2 docs")
    }

    pub fn query(&self, stable_id: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["query", "--store"])
            .arg(&self.base.store)
            .args(["--repository-id", ROOT_REPOSITORY_ID, "--documents"])
            .arg(&self.base.documents)
            .args(["--id", stable_id, "--format", "json"])
            .output()
            .expect("launch R2 query")
    }

    pub fn custom(gitmodules: Option<&[u8]>, gitlinks: &[(String, String)]) -> Self {
        let base = super::s4::MaterializedRepository::revision_a();
        let global_config = base.root.join("global.gitconfig");
        let mut entries = vec![
            (
                "Cargo.toml".to_owned(),
                format!("100644 blob {ROOT_MANIFEST_BLOB_OID}\tCargo.toml\n"),
            ),
            (
                "crates".to_owned(),
                format!("040000 tree {CRATES_TREE_OID}\tcrates\n"),
            ),
        ];
        if let Some(bytes) = gitmodules {
            let mut hash = git_command(&global_config);
            hash.arg("-C")
                .arg(&base.worktree)
                .args(["hash-object", "-w", "--stdin"]);
            let blob_oid = stdout_line(successful_output(hash, Some(bytes)));
            entries.push((
                ".gitmodules".to_owned(),
                format!("100644 blob {blob_oid}\t.gitmodules\n"),
            ));
        }
        for (path, object_id) in gitlinks {
            entries.push((path.clone(), format!("160000 commit {object_id}\t{path}\n")));
        }
        entries.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
        let root_tree = write_tree(
            &base.worktree,
            &global_config,
            &entries
                .into_iter()
                .map(|(_, entry)| entry)
                .collect::<String>(),
            true,
        );
        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&base.worktree)
            .args(["commit-tree", &root_tree, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "1785628802 +0000")
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "1785628802 +0000");
        let commit_oid = stdout_line(successful_output(
            make_commit,
            Some(b"CodeNoesis R2 generated boundary variant\n"),
        ));
        let mut update_ref = git_command(&global_config);
        update_ref.arg("-C").arg(&base.worktree).args([
            "update-ref",
            "refs/heads/main",
            &commit_oid,
        ]);
        successful_output(update_ref, None);
        Self { base, commit_oid }
    }
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s1/gitlink-boundary-v1")
}

pub fn scan_command(
    repository: &Path,
    store: &Path,
    revision: &str,
    manifest: Option<&Path>,
) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
    command
        .args(["scan", "--repository"])
        .arg(repository)
        .args([
            "--repository-id",
            ROOT_REPOSITORY_ID,
            "--revision",
            revision,
            "--profile",
            "standard-local-s4",
            "--repository-boundary-profile",
            "local-gitlinks-v1",
            "--store",
        ])
        .arg(store);
    if let Some(manifest) = manifest {
        command.arg("--repository-boundary-manifest").arg(manifest);
    }
    command.args(["--format", "json"]);
    command
}

pub fn expected_bound_boundaries() -> serde_json::Value {
    serde_json::from_slice(
        &fs::read(fixture_root().join("expected-boundaries-bound.json"))
            .expect("read approved bound boundary golden"),
    )
    .expect("parse approved bound boundary golden")
}

pub fn expected_unbound_boundaries() -> serde_json::Value {
    serde_json::from_slice(
        &fs::read(fixture_root().join("expected-boundaries-unbound.json"))
            .expect("read approved unbound boundary golden"),
    )
    .expect("parse approved unbound boundary golden")
}

fn write_tree(worktree: &Path, global_config: &Path, entries: &str, allow_missing: bool) -> String {
    let mut command = git_command(global_config);
    command.arg("-C").arg(worktree).arg("mktree");
    if allow_missing {
        command.arg("--missing");
    }
    stdout_line(successful_output(command, Some(entries.as_bytes())))
}
