use duplicate_a::*;
use duplicate_b::*;

pub struct RegistrationSet;

impl RegistrationSet {
    pub fn new() -> Self { Self }
    pub fn component<T>(self, _target: T) -> Self { self }
    pub fn service<T>(self, _target: T) -> Self { self }
    pub fn configuration<T>(self, _key: &str, _target: T) -> Self { self }
    pub fn endpoint<T>(self, _path: &str, _target: T) -> Self { self }
    pub fn route<T>(self, _method: &str, _path: &str, _target: T) -> Self { self }
    pub fn group<T>(self, _path: &str, _target: T) -> Self { self }
    pub fn handler<T>(self, _target: T) -> Self { self }
}

pub fn application() -> RegistrationSet {
    RegistrationSet::new()
        .component(components::shell)
        .service(services::worker)
        .configuration("mode", config::mode)
        .endpoint("/metrics", handlers::metrics)
        .route("GET", "/health", handlers::health)
        .route("GET", "/items", handlers::list_items)
        .route("POST", "/items", handlers::create_item)
        .group("/api", nested_group())
        .handler(handlers::fallback)
        .route("GET", "/external", external::missing)
        .handler(duplicate)
}

pub fn nested_group() -> RegistrationSet {
    RegistrationSet::new()
        .route("GET", "/users", handlers::list_users)
}

pub fn unused_builder_decoy() {
    let _unused_builder = RegistrationSet::new()
        .route("GET", "/unused", handlers::unused);
}

pub mod components { pub fn shell() {} }
pub mod services { pub fn worker() {} }
pub mod config { pub fn mode() {} }
pub mod handlers {
    pub fn metrics() {}
    pub fn health() {}
    pub fn list_items() {}
    pub fn create_item() {}
    pub fn fallback() {}
    pub fn list_users() {}
    pub fn unused() {}
}
pub mod duplicate_a { pub fn duplicate() {} }
pub mod duplicate_b { pub fn duplicate() {} }
