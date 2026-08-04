pub mod attribute_style;
pub mod builder_style;

// COMMENT_ROUTE_DECOY: .route("GET", "/comment", comment_handler)
pub const STRING_ROUTE_DECOY: &str = ".route(\"GET\", \"/string\", string_handler)";
#[doc = "DOC_ROUTE_DECOY: #[route(\"/doc\")]"]
pub const DOC_ROUTE_DECOY: &str = "documentation is not a declaration";
pub use crate::builder_style::handlers as IMPORT_ROUTE_DECOY;
pub fn NAME_ONLY_ROUTE_DECOY() {}
