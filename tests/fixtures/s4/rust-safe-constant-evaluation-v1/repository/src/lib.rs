#![allow(dead_code)]

pub const BASE: i32 = 2 + 3 * 4;
pub const OFFSET: i32 = BASE - 5;
pub const ENABLED: bool = true && !false;
pub static LIMIT: u16 = 255 + 1;

#[repr(u8)]
pub enum Mode {
    Off,
    Warm = 3,
    Hot,
}

pub const TARGET_WIDTH: usize = 4;

const fn helper() -> i32 {
    7
}

pub const CALL_RESULT: i32 = helper();

pub enum PlatformSized {
    First,
    Second,
}
