use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::{git_command, read_repository_text, stdout_line, successful_output, unique_temp_root};

pub const REPOSITORY_ID: &str = "urn:codenoesis:fixture:s4-root-package-workspace-v1";
pub const GITLINK_OID: &str = "6ecf94267842da776e35406a9ebcb85e058a3181";
pub const IMPLICIT_ROOT_TREE_OID: &str = "8295d04a96f8c2af48cc7492a797080ea08cf2ea";
pub const IMPLICIT_ROOT_COMMIT_OID: &str = "37eb6d1abf25891c52fbdf9b735973c441a8598b";

pub struct MaterializedRootPackageRepository {
    pub root: PathBuf,
    pub worktree: PathBuf,
    pub store: PathBuf,
    pub documents: PathBuf,
    pub commit_oid: String,
    includes_boundary: bool,
}

impl MaterializedRootPackageRepository {
    pub fn implicit() -> Self {
        Self::materialize("implicit")
    }

    pub fn explicit_dot() -> Self {
        Self::materialize("explicit_dot")
    }

    pub fn standalone() -> Self {
        Self::materialize("standalone")
    }

    pub fn virtual_workspace() -> Self {
        Self::materialize("virtual")
    }

    pub fn member_exclude_conflict() -> Self {
        Self::materialize("member_exclude_conflict")
    }

    #[allow(clippy::too_many_lines)]
    fn materialize(variant: &str) -> Self {
        let fixture = fixture_root();
        let fixture_manifest: Value = serde_json::from_slice(
            &fs::read(fixture.join("manifest.json")).expect("read R3 fixture manifest"),
        )
        .expect("parse R3 fixture manifest");
        assert_eq!(fixture_manifest["repository_identity"], REPOSITORY_ID);
        let variant_manifest =
            fixture_manifest["materialization"]["variants"][variant]["root_manifest"]
                .as_str()
                .expect("reviewed root-manifest variant");
        let includes_boundary =
            fixture_manifest["materialization"]["variants"][variant]["includes_boundary"]
                .as_bool()
                .expect("reviewed boundary flag");

        let root = unique_temp_root();
        let worktree = root.join("repository");
        let store = root.join("store");
        let documents = root.join("documents");
        let template = root.join("template");
        let global_config = root.join("global.gitconfig");
        fs::create_dir_all(&template).expect("create empty Git template directory");
        fs::write(&global_config, []).expect("create empty global Git configuration");

        let mut init = git_command(&global_config);
        init.args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&worktree);
        successful_output(init, None);

        for file in fixture_manifest["files"]
            .as_array()
            .expect("reviewed R3 fixture files")
        {
            let fixture_path = file["path"].as_str().expect("reviewed fixture path");
            let destination = if fixture_path == variant_manifest {
                Some("Cargo.toml")
            } else {
                fixture_path.strip_prefix("shared-tree/")
            };
            let Some(destination) = destination else {
                continue;
            };
            let bytes = read_repository_text(fixture.join(fixture_path));
            assert_eq!(
                u64::try_from(bytes.len()).expect("fixture byte length"),
                file["byte_length"].as_u64().expect("reviewed byte length"),
                "R3 fixture byte length changed for {fixture_path}"
            );
            assert_eq!(
                lower_hex(&Sha256::digest(&bytes)),
                file["sha256"].as_str().expect("reviewed SHA-256"),
                "R3 fixture SHA-256 changed for {fixture_path}"
            );
            let mut hash = git_command(&global_config);
            hash.arg("-C")
                .arg(&worktree)
                .args(["hash-object", "-w", "--stdin"]);
            let blob_oid = stdout_line(successful_output(hash, Some(&bytes)));
            assert_eq!(
                blob_oid,
                file["git_blob_oid"].as_str().expect("reviewed Git blob"),
                "R3 fixture Git blob changed for {fixture_path}"
            );
            update_index(&worktree, &global_config, "100644", &blob_oid, destination);
        }

        if includes_boundary {
            update_index(
                &worktree,
                &global_config,
                "160000",
                GITLINK_OID,
                "external/model",
            );
        }

        let mut write_tree = git_command(&global_config);
        write_tree.arg("-C").arg(&worktree).arg("write-tree");
        let tree_oid = stdout_line(successful_output(write_tree, None));
        if variant == "implicit" {
            assert_eq!(tree_oid, IMPLICIT_ROOT_TREE_OID, "R3 root tree changed");
        }
        let mut make_commit = git_command(&global_config);
        make_commit
            .arg("-C")
            .arg(&worktree)
            .args(["commit-tree", &tree_oid, "-F", "-"])
            .env("GIT_AUTHOR_NAME", "CodeNoesis Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_AUTHOR_DATE", "1785628800 +0000")
            .env("GIT_COMMITTER_NAME", "CodeNoesis Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid")
            .env("GIT_COMMITTER_DATE", "1785628800 +0000");
        let commit_oid = stdout_line(successful_output(
            make_commit,
            Some(format!("CodeNoesis R3 {variant} fixture\n").as_bytes()),
        ));
        if variant == "implicit" {
            assert_eq!(
                commit_oid, IMPLICIT_ROOT_COMMIT_OID,
                "R3 root commit changed"
            );
        }
        let mut update_ref = git_command(&global_config);
        update_ref
            .arg("-C")
            .arg(&worktree)
            .args(["update-ref", "refs/heads/main", &commit_oid]);
        successful_output(update_ref, None);

        Self {
            root,
            worktree,
            store,
            documents,
            commit_oid,
            includes_boundary,
        }
    }

    pub fn scan(&self) -> Output {
        self.scan_with_profiles(Some("cargo-root-package-v1"), self.includes_boundary)
    }

    pub fn scan_without_boundary_profile(&self) -> Output {
        self.scan_with_profiles(Some("cargo-root-package-v1"), false)
    }

    pub fn scan_legacy(&self) -> Output {
        self.scan_with_profiles(None, self.includes_boundary)
    }

    pub fn scan_with_workspace_profile(&self, workspace_profile: &str) -> Output {
        self.scan_with_profiles(Some(workspace_profile), self.includes_boundary)
    }

    pub fn docs(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["docs", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--output"])
            .arg(&self.documents)
            .args(["--format", "json"])
            .output()
            .expect("launch R3 documentation subject")
    }

    pub fn query(&self, requested_id: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_noesis"))
            .args(["query", "--store"])
            .arg(&self.store)
            .args(["--repository-id", REPOSITORY_ID, "--documents"])
            .arg(&self.documents)
            .args(["--id", requested_id, "--format", "json"])
            .output()
            .expect("launch R3 query subject")
    }

    fn scan_with_profiles(
        &self,
        workspace_profile: Option<&str>,
        boundary_profile: bool,
    ) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_noesis"));
        command
            .args(["scan", "--repository"])
            .arg(&self.worktree)
            .args(["--repository-id", REPOSITORY_ID, "--revision"])
            .arg(&self.commit_oid)
            .args(["--profile", "standard-local-s4"]);
        if let Some(workspace_profile) = workspace_profile {
            command.args(["--workspace-profile", workspace_profile]);
        }
        if boundary_profile {
            command.args(["--repository-boundary-profile", "local-gitlinks-v1"]);
        }
        command
            .arg("--store")
            .arg(&self.store)
            .args(["--format", "json"])
            .output()
            .expect("launch R3 root-package subject")
    }
}

impl Drop for MaterializedRootPackageRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/s4/root-package-workspace-v1")
}

fn update_index(worktree: &Path, global_config: &Path, mode: &str, oid: &str, path: &str) {
    let mut update = git_command(global_config);
    update
        .arg("-C")
        .arg(worktree)
        .args(["update-index", "--add", "--cacheinfo"])
        .arg(format!("{mode},{oid},{path}"));
    successful_output(update, None);
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("write digest");
    }
    encoded
}
