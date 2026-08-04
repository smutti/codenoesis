#[derive(Debug)]
pub struct Record {
    pub key: String,
    pub value: Option<String>,
    pub(crate) revision: u64,
    pub r#type: String,
}

pub struct Coordinates(pub i32, pub i32);

pub struct Marker;

#[repr(u8)]
pub enum Status {
    Ready = 1,
    Waiting,
}

pub enum Message {
    Empty,
    Data(u8, String),
    Named { key: String, value: Option<String> },
}

pub const UNICODE_Δ: &str = "delta";

#[cfg(feature = "experimental")]
pub struct ConditionalRecord {
    pub value: u32,
}

#[cfg_attr(feature = "cloneable", derive(Clone))]
pub struct Transformable(pub u8);

#[derive(Clone)]
pub struct Derived {
    pub id: u64,
}

#[framework::component(role = "service", endpoint = "/syntactic-only")]
pub struct SyntacticComponent {
    pub endpoint: &'static str,
}

#[doc = "Raw and Unicode identifiers remain lexical declarations."]
pub struct LexicalNames {
    pub r#type: String,
    pub Δ: u32,
}
