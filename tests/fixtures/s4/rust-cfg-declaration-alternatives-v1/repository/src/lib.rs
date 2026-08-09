#![allow(dead_code)]

pub struct Client;
pub struct Context;

impl Client {
    #[cfg(target_family = "unix")]
    pub fn try_start_clipboard(&self, context: Option<Context>) {}

    #[cfg(target_family = "windows")]
    pub fn try_start_clipboard(&self, value: Option<()>) {}
}

// fn try_start_clipboard(&self, decoy: Option<String>) {}
pub const METHOD_DECOY: &str =
    "#[cfg(decoy)] pub fn try_start_clipboard(&self, decoy: Option<String>) {}";

macro_rules! cfg_method_decoy {
    () => {
        #[cfg(decoy)]
        fn try_start_clipboard(&self, decoy: Option<String>) {}
    };
}
