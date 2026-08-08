fn main() {
    std::fs::write("BUILD_SENTINEL_EXECUTED", "the scanner executed build.rs")
        .expect("build sentinel write");
    panic!("CodeNoesis standard profiles must never execute build.rs");
}
