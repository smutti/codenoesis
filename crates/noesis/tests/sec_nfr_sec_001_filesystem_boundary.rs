#[cfg(target_os = "linux")]
mod linux {
    use std::fs;
    use std::os::fd::AsRawFd as _;
    use std::os::unix::fs::symlink;

    #[test]
    fn sec_nfr_sec_001_scan_stays_inside_repository_root() {
        let parent = unique_root();
        let repository = parent.join("repository");
        let outside = parent.join("outside-sentinel");
        fs::create_dir(&repository).expect("create Landlock repository root");
        fs::write(repository.join("inside"), b"inside\n").expect("write inside read sentinel");
        fs::write(&outside, b"outside\n").expect("write outside read sentinel");
        symlink(&outside, repository.join("escape")).expect("create escape symlink");
        let outside_descriptor = fs::File::open(&outside).expect("open proc-fd escape sentinel");
        let proc_fd_escape = format!("/proc/self/fd/{}", outside_descriptor.as_raw_fd());
        std::env::set_current_dir(&repository).expect("enter repository for relative probe");

        noesis::install_s1_filesystem_boundary(repository.as_os_str())
            .expect("Landlock must fully enforce the S1 filesystem boundary");

        assert_eq!(
            fs::read(repository.join("inside")).expect("read inside repository root"),
            b"inside\n"
        );
        assert_permission_denied(fs::read(&outside));
        assert_permission_denied(fs::read("../outside-sentinel"));
        assert_permission_denied(fs::read(repository.join("../outside-sentinel")));
        assert_permission_denied(fs::read(repository.join("escape")));
        assert_permission_denied(fs::read(proc_fd_escape));
        assert_permission_denied(fs::write(repository.join("write-attempt"), b"denied\n"));
    }

    #[test]
    fn sec_fr_sto_001_s3_writes_only_explicit_store_root() {
        let parent = unique_root();
        let repository = parent.join("repository");
        let store = parent.join("store");
        let outside = parent.join("outside-sentinel");
        fs::create_dir(&repository).expect("create S3 repository root");
        fs::create_dir(&store).expect("create S3 store root");
        fs::write(repository.join("inside"), b"inside\n").expect("write repository sentinel");
        fs::write(&outside, b"outside\n").expect("write outside sentinel");

        noesis::install_s3_filesystem_boundary(repository.as_os_str(), store.as_os_str())
            .expect("Landlock must fully enforce the S3 filesystem boundary");

        assert_eq!(
            fs::read(repository.join("inside")).expect("read inside repository root"),
            b"inside\n"
        );
        fs::write(store.join("allowed"), b"allowed\n").expect("write inside explicit store");
        assert_permission_denied(fs::write(repository.join("denied"), b"denied\n"));
        assert_permission_denied(fs::read(&outside));
        assert_permission_denied(fs::write(parent.join("outside-write"), b"denied\n"));
    }

    #[test]
    fn sec_fr_fed_001_s6_reads_only_manifest_and_explicit_roots() {
        let parent = unique_root();
        let manifest = parent.join("workspace.json");
        let provider = parent.join("provider");
        let client = parent.join("client");
        let outside = parent.join("outside");
        fs::create_dir(&provider).expect("create provider root");
        fs::create_dir(&client).expect("create client root");
        fs::write(&manifest, b"{}\n").expect("write manifest sentinel");
        fs::write(provider.join("openapi.yaml"), b"openapi: 3.1.0\n")
            .expect("write provider sentinel");
        fs::write(client.join("federation.json"), b"{}\n").expect("write client sentinel");
        fs::write(&outside, b"outside\n").expect("write outside sentinel");

        noesis::install_s6_filesystem_boundary(
            manifest.as_os_str(),
            &[provider.clone(), client.clone()],
        )
        .expect("Landlock must fully enforce the S6 filesystem boundary");

        assert_eq!(fs::read(&manifest).expect("read manifest"), b"{}\n");
        assert_eq!(
            fs::read(provider.join("openapi.yaml")).expect("read provider"),
            b"openapi: 3.1.0\n"
        );
        assert_eq!(
            fs::read(client.join("federation.json")).expect("read client"),
            b"{}\n"
        );
        assert_permission_denied(fs::read(&outside));
        assert_permission_denied(fs::write(provider.join("denied"), b"denied\n"));
        assert_permission_denied(fs::write(client.join("denied"), b"denied\n"));
        assert_permission_denied(fs::write(&manifest, b"denied\n"));
    }

    fn assert_permission_denied<T>(result: std::io::Result<T>) {
        let error = result
            .map(|_| ())
            .expect_err("Landlock operation must be denied");
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    }

    fn unique_root() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};

        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "codenoesis-s1-landlock-{}-{timestamp}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create Landlock self-test root");
        root
    }
}
