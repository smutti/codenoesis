pub fn generated_directory_decoy() {
    RegistrationSet::new().route("GET", "/generated-file", generated_handler);
}
