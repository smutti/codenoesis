#![allow(dead_code)]

pub mod model;

pub const DEFAULT_LIMIT: usize = 64;
pub static PRODUCT_NAME: &str = "rust-semantic-depth-fixture";
pub static mut LEGACY_COUNTER: u64 = 0;

pub trait Paint {
    fn render(&self) -> String;
}

pub trait Preview {
    fn render(&self) -> String;
}

pub trait Descriptor {
    type Output;
    const KIND: &'static str;

    fn describe(&self) -> Self::Output;

    fn label(&self) -> &'static str {
        "descriptor"
    }
}

impl model::Record {
    pub const EMPTY_KEY: &'static str = "";

    pub fn new(key: String, value: Option<String>) -> Self {
        Self {
            key,
            value,
            revision: 0,
            r#type: "record".to_owned(),
        }
    }
}

impl model::Record {
    pub fn key(&self) -> &str {
        &self.key
    }
}

impl Paint for model::Record {
    fn render(&self) -> String {
        self.key.clone()
    }
}

impl Preview for model::Record {
    fn render(&self) -> String {
        format!("preview:{}", self.key)
    }
}

impl Descriptor for model::Record {
    type Output = String;
    const KIND: &'static str = "record";

    fn describe(&self) -> Self::Output {
        self.value.clone().unwrap_or_default()
    }
}

// struct CommentFieldDecoy { hidden: String }
pub const DECLARATION_STRING_DECOY: &str = "enum StringVariantDecoy { Hidden(u8) }";

macro_rules! declaration_decoys {
    () => {
        const MACRO_CONSTANT_DECOY: usize = 1;
        struct MacroFieldDecoy {
            hidden: String,
        }
    };
}
