fn main() {
    std::fs::write(
        "K1_BUILD_SENTINEL_EXECUTED",
        "CodeNoesis must never execute this build script",
    )
    .expect("sentinel write");
}
