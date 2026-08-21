use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};

use codenoesis_application::{
    GitImpactAcquisitionError, GitImpactAcquisitionService, GitImpactRepositoryRequest,
};
use codenoesis_contracts::MAX_R19_SOURCE_BYTES_PER_FILE;
use codenoesis_domain::{
    AcquiredFile, AcquiredRepository, BoundRevision, ObjectId, RegularFileMode, RepositoryError,
    RepositoryIdentity, Revision,
};
use codenoesis_ports::{RepositoryAcquirer, SafeRepositoryAcquirer};

#[test]
fn gt_fr_imp_006_application_selects_only_requested_git_files_canonically() {
    let service = GitImpactAcquisitionService::new(MockAcquirer::reviewed());
    let requests = [
        request("urn:codenoesis:test:r19-b", 'b', &["src/z.rs", "src/a.rs"]),
        request("urn:codenoesis:test:r19-a", 'a', &["src/a.rs"]),
    ];
    let selected = service.acquire(&requests).expect("R19 selected sources");
    let projection = selected
        .iter()
        .map(|source| {
            (
                source.repository_identity.as_str(),
                source.commit_oid.as_str(),
                source.path.as_str(),
                source.bytes.as_slice(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        projection,
        vec![
            (
                "urn:codenoesis:test:r19-a",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "src/a.rs",
                b"a-source".as_slice(),
            ),
            (
                "urn:codenoesis:test:r19-b",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "src/a.rs",
                b"b-source-a".as_slice(),
            ),
            (
                "urn:codenoesis:test:r19-b",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "src/z.rs",
                b"b-source-z".as_slice(),
            ),
        ]
    );
}

#[test]
fn pt_fr_imp_006_application_file_limit_has_maximum_and_plus_one() {
    let maximum = usize::try_from(MAX_R19_SOURCE_BYTES_PER_FILE).unwrap();
    let service = GitImpactAcquisitionService::new(MockAcquirer::single(vec![b'x'; maximum]));
    let selected = service
        .acquire(&[request(
            "urn:codenoesis:test:r19-limit",
            'd',
            &["src/limit.rs"],
        )])
        .expect("R19 exact source maximum");
    assert_eq!(selected[0].bytes.len(), maximum);

    let service = GitImpactAcquisitionService::new(MockAcquirer::single(vec![b'x'; maximum + 1]));
    assert_eq!(
        service.acquire(&[request(
            "urn:codenoesis:test:r19-limit",
            'd',
            &["src/limit.rs"],
        )]),
        Err(GitImpactAcquisitionError::LimitExceeded)
    );
}

#[test]
fn ct_fr_imp_006_application_rejects_missing_and_duplicate_selection() {
    let service = GitImpactAcquisitionService::new(MockAcquirer::reviewed());
    assert_eq!(
        service.acquire(&[request(
            "urn:codenoesis:test:r19-a",
            'a',
            &["src/missing.rs"],
        )]),
        Err(GitImpactAcquisitionError::InvalidSelection)
    );
    assert_eq!(
        service.acquire(&[request(
            "urn:codenoesis:test:r19-a",
            'a',
            &["src/a.rs", "src/a.rs"],
        )]),
        Err(GitImpactAcquisitionError::InvalidSelection)
    );
}

struct MockAcquirer {
    repositories: BTreeMap<String, Vec<AcquiredFile>>,
}

impl MockAcquirer {
    fn reviewed() -> Self {
        Self {
            repositories: BTreeMap::from([
                (
                    "urn:codenoesis:test:r19-a".to_owned(),
                    vec![file("src/a.rs", '1', b"a-source".to_vec())],
                ),
                (
                    "urn:codenoesis:test:r19-b".to_owned(),
                    vec![
                        file("src/z.rs", '2', b"b-source-z".to_vec()),
                        file("src/a.rs", '3', b"b-source-a".to_vec()),
                    ],
                ),
            ]),
        }
    }

    fn single(bytes: Vec<u8>) -> Self {
        Self {
            repositories: BTreeMap::from([(
                "urn:codenoesis:test:r19-limit".to_owned(),
                vec![file("src/limit.rs", '4', bytes)],
            )]),
        }
    }
}

impl RepositoryAcquirer for MockAcquirer {
    fn bind(
        &self,
        _repository: &OsStr,
        _identity: RepositoryIdentity,
        _revision: Revision,
    ) -> Result<BoundRevision, RepositoryError> {
        Err(RepositoryError::Unexpected)
    }
}

impl SafeRepositoryAcquirer for MockAcquirer {
    fn acquire_inventory(
        &self,
        _repository: &OsStr,
        identity: RepositoryIdentity,
        revision: Revision,
    ) -> Result<AcquiredRepository, RepositoryError> {
        let Revision::Commit(commit_oid) = revision else {
            return Err(RepositoryError::Unexpected);
        };
        let files = self
            .repositories
            .get(identity.as_str())
            .cloned()
            .ok_or(RepositoryError::Unexpected)?;
        Ok(AcquiredRepository::new(
            BoundRevision::new(identity, commit_oid, oid('e')),
            2,
            files,
        ))
    }
}

fn request(identity: &str, revision: char, paths: &[&str]) -> GitImpactRepositoryRequest {
    GitImpactRepositoryRequest::new(
        OsString::from("repository"),
        RepositoryIdentity::parse(identity).expect("R19 application identity"),
        Revision::Commit(oid(revision)),
        paths.iter().map(|path| (*path).to_owned()).collect(),
    )
}

fn file(path: &str, blob: char, bytes: Vec<u8>) -> AcquiredFile {
    AcquiredFile::new(path.to_owned(), RegularFileMode::Regular, oid(blob), bytes)
}

fn oid(value: char) -> ObjectId {
    ObjectId::parse_sha1(&value.to_string().repeat(40)).expect("R19 synthetic object ID")
}
