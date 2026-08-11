#![allow(dead_code)]

pub fn choose(value: i32, flag: bool) -> i32 {
    let mut total = value;
    if flag {
        total = total + 1;
    }
    let result = total;
    result
}
