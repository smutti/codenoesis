use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use codenoesis_domain::s1_boundaries::{
    BoundaryLimit, MAX_GITLINK_ENTRIES, MAX_GITMODULES_BYTES, NestedAcquisitionProfile,
    NestedRepositoryAcquisitionError, RepositoryBoundaryAcquisitionError, RepositoryBoundaryError,
};
use codenoesis_domain::{
    AcquisitionError, EntryPolicy, ObjectId, RepositoryError, RepositoryIdentity, Revision,
};
use codenoesis_ports::{RepositoryBoundaryAcquirer, SafeRepositoryAcquirer};
use codenoesis_repository::LocalGitRepository;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn pt_fr_acq_005_gitlinks_have_exact_128_maximum() {
    let maximum = repository_with_gitlinks(usize::try_from(MAX_GITLINK_ENTRIES).unwrap());
    let acquired = acquire_boundaries(&maximum).expect("128 gitlinks are accepted");
    assert_eq!(acquired.gitlinks.len(), 128);

    let plus_one = repository_with_gitlinks(usize::try_from(MAX_GITLINK_ENTRIES + 1).unwrap());
    assert_eq!(
        acquire_boundaries(&plus_one),
        Err(RepositoryBoundaryAcquisitionError::Boundary(
            RepositoryBoundaryError::LimitExceeded {
                limit: BoundaryLimit::GitlinkEntries,
                maximum: MAX_GITLINK_ENTRIES,
                observed: MAX_GITLINK_ENTRIES + 1,
            }
        ))
    );
}

#[test]
fn pt_fr_acq_005_gitmodules_bytes_are_rejected_before_capture() {
    let maximum = repository_with_gitmodules(usize::try_from(MAX_GITMODULES_BYTES).unwrap());
    let acquired = acquire_boundaries(&maximum).expect("1 MiB .gitmodules is accepted");
    assert_eq!(
        acquired.gitmodules.as_ref().unwrap().bytes.len(),
        usize::try_from(MAX_GITMODULES_BYTES).unwrap()
    );

    let plus_one = repository_with_gitmodules(usize::try_from(MAX_GITMODULES_BYTES + 1).unwrap());
    assert_eq!(
        acquire_boundaries(&plus_one),
        Err(RepositoryBoundaryAcquisitionError::Boundary(
            RepositoryBoundaryError::LimitExceeded {
                limit: BoundaryLimit::GitmodulesBytes,
                maximum: MAX_GITMODULES_BYTES,
                observed: MAX_GITMODULES_BYTES + 1,
            }
        ))
    );
}

#[test]
fn reg_fr_acq_005_legacy_gitlink_rejection_is_unchanged() {
    let repository = repository_with_gitlinks(1);
    let error = LocalGitRepository::new()
        .acquire_inventory(
            repository.worktree.as_os_str(),
            identity("urn:codenoesis:repository:legacy"),
            Revision::Commit(repository.commit_oid.clone()),
        )
        .unwrap_err();
    assert_eq!(
        error,
        RepositoryError::Acquisition(AcquisitionError::EntryPolicyViolation {
            path: "external-000".to_owned(),
            entry: EntryPolicy::Gitlink,
        })
    );
}

#[test]
fn sec_fr_acq_005_unsupplied_worktree_is_never_opened() {
    let repository = repository_with_gitlinks(1);
    let ambient = repository.worktree.join("external-000");
    fs::write(&ambient, b"ambient nested canary").expect("write ambient canary");
    let acquired = acquire_boundaries(&repository).expect("acquire immutable root tree");
    assert_eq!(acquired.gitlinks.len(), 1);
    assert_eq!(
        fs::read(&ambient).unwrap(),
        b"ambient nested canary",
        "ambient worktree content must remain untouched"
    );
}

#[test]
fn gt_fr_acq_005_explicit_nested_binding_is_loose_packed_equivalent() {
    let loose = nested_repository();
    let expected = bind_nested(&loose, NestedAcquisitionProfile::VerifiedLooseSha1V1)
        .expect("bind loose nested repository");
    assert_eq!(expected.commit_oid(), &loose.commit_oid);
    assert_eq!(
        expected.tree_oid().as_str(),
        "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
    );

    git(&loose, &["repack", "-ad"]);
    let packed = bind_nested(&loose, NestedAcquisitionProfile::LocalGitSha1PackedV1)
        .expect("bind packed nested repository");
    assert_eq!(packed, expected);
}

#[test]
fn gt_fr_acq_005_nested_missing_object_is_typed() {
    let nested = nested_repository();
    let missing = ObjectId::parse_sha1("1111111111111111111111111111111111111111").unwrap();
    let error = LocalGitRepository::new()
        .bind_nested_repository(
            nested.worktree.as_os_str(),
            identity("urn:codenoesis:repository:nested"),
            Revision::Commit(missing.clone()),
            NestedAcquisitionProfile::VerifiedLooseSha1V1,
        )
        .unwrap_err();
    assert_eq!(
        error,
        NestedRepositoryAcquisitionError::Repository(RepositoryError::Acquisition(
            AcquisitionError::RevisionNotFound {
                revision: Revision::Commit(missing),
            }
        ))
    );
}

fn acquire_boundaries(
    repository: &TestRepository,
) -> Result<
    codenoesis_domain::s1_boundaries::AcquiredRepositoryBoundaries,
    RepositoryBoundaryAcquisitionError,
> {
    LocalGitRepository::new().acquire_inventory_with_boundaries(
        repository.worktree.as_os_str(),
        identity("urn:codenoesis:repository:root"),
        Revision::Commit(repository.commit_oid.clone()),
    )
}

fn bind_nested(
    repository: &TestRepository,
    profile: NestedAcquisitionProfile,
) -> Result<codenoesis_domain::BoundRevision, NestedRepositoryAcquisitionError> {
    LocalGitRepository::new().bind_nested_repository(
        repository.worktree.as_os_str(),
        identity("urn:codenoesis:repository:nested"),
        Revision::Commit(repository.commit_oid.clone()),
        profile,
    )
}

fn repository_with_gitlinks(count: usize) -> TestRepository {
    let repository = TestRepository::new();
    let nested_oid = "1111111111111111111111111111111111111111";
    let mut entries = String::new();
    for index in 0..count {
        writeln!(entries, "160000 commit {nested_oid}\texternal-{index:03}")
            .expect("write gitlink fixture entry");
    }
    repository.commit_tree(entries.as_bytes(), "gitlink limit fixture")
}

fn repository_with_gitmodules(byte_length: usize) -> TestRepository {
    let repository = TestRepository::new();
    let mut bytes = vec![b' '; byte_length];
    if let Some(first) = bytes.first_mut() {
        *first = b'#';
    }
    let blob_oid = repository.hash_blob(&bytes);
    let entries = format!("100644 blob {blob_oid}\t.gitmodules\n");
    repository.commit_tree(entries.as_bytes(), "gitmodules byte limit fixture")
}

fn nested_repository() -> TestRepository {
    let mut repository = TestRepository::new();
    let empty_tree = repository.command_with_input(&["mktree"], b"", &[]);
    assert_eq!(empty_tree, "4b825dc642cb6eb9a060e54bf8d69288fbee4904");
    let commit_oid = repository.command_with_input(
        &["commit-tree", &empty_tree, "-F", "-"],
        b"CodeNoesis R2 empty nested fixture\n",
        &[
            ("GIT_AUTHOR_NAME", "CodeNoesis Fixture"),
            ("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid"),
            ("GIT_AUTHOR_DATE", "1785628800 +0000"),
            ("GIT_COMMITTER_NAME", "CodeNoesis Fixture"),
            ("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid"),
            ("GIT_COMMITTER_DATE", "1785628800 +0000"),
        ],
    );
    assert_eq!(commit_oid, "6ecf94267842da776e35406a9ebcb85e058a3181");
    git(&repository, &["update-ref", "refs/heads/main", &commit_oid]);
    repository.commit_oid = ObjectId::parse_sha1(&commit_oid).unwrap();
    repository
}

struct TestRepository {
    root: PathBuf,
    worktree: PathBuf,
    global_config: PathBuf,
    commit_oid: ObjectId,
}

impl TestRepository {
    fn new() -> Self {
        let root = unique_temp_root();
        let worktree = root.join("repository");
        let template = root.join("template");
        let global_config = root.join("global.gitconfig");
        fs::create_dir(&template).unwrap();
        fs::write(&global_config, []).unwrap();
        let repository = Self {
            root,
            worktree,
            global_config,
            commit_oid: ObjectId::parse_sha1("0000000000000000000000000000000000000000").unwrap(),
        };
        let mut command = repository.command();
        command
            .args(["init", "--quiet", "--initial-branch=main"])
            .arg(format!("--template={}", template.display()))
            .arg(&repository.worktree);
        run(command, None);
        repository
    }

    fn hash_blob(&self, bytes: &[u8]) -> String {
        self.command_with_input(&["hash-object", "-w", "--stdin"], bytes, &[])
    }

    fn commit_tree(mut self, entries: &[u8], message: &str) -> Self {
        let tree_oid = self.command_with_input(&["mktree", "--missing"], entries, &[]);
        let commit_oid = self.command_with_input(
            &["commit-tree", &tree_oid, "-F", "-"],
            format!("{message}\n").as_bytes(),
            &[
                ("GIT_AUTHOR_NAME", "CodeNoesis Fixture"),
                ("GIT_AUTHOR_EMAIL", "fixture@codenoesis.invalid"),
                ("GIT_AUTHOR_DATE", "1785628800 +0000"),
                ("GIT_COMMITTER_NAME", "CodeNoesis Fixture"),
                ("GIT_COMMITTER_EMAIL", "fixture@codenoesis.invalid"),
                ("GIT_COMMITTER_DATE", "1785628800 +0000"),
            ],
        );
        git(&self, &["update-ref", "refs/heads/main", &commit_oid]);
        self.commit_oid = ObjectId::parse_sha1(&commit_oid).unwrap();
        self
    }

    fn command(&self) -> Command {
        let mut command = Command::new("git");
        command
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &self.global_config)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE");
        command
    }

    fn command_with_input(
        &self,
        arguments: &[&str],
        input: &[u8],
        environment: &[(&str, &str)],
    ) -> String {
        let mut command = self.command();
        command.arg("-C").arg(&self.worktree).args(arguments);
        for (key, value) in environment {
            command.env(key, value);
        }
        String::from_utf8(run(command, Some(input)).stdout)
            .unwrap()
            .trim_end_matches(['\r', '\n'])
            .to_owned()
    }
}

impl Drop for TestRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn git(repository: &TestRepository, arguments: &[&str]) {
    let mut command = repository.command();
    command.arg("-C").arg(&repository.worktree).args(arguments);
    run(command, None);
}

fn run(mut command: Command, input: Option<&[u8]>) -> std::process::Output {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    let invocation = format!("{command:?}");
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        child.stdin.take().unwrap().write_all(input).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "failed {invocation}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn identity(value: &str) -> RepositoryIdentity {
    RepositoryIdentity::parse(value).unwrap()
}

fn unique_temp_root() -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "codenoesis-r2-repository-{}-{timestamp}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    root
}
