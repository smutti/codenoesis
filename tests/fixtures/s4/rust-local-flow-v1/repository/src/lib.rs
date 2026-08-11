#![allow(dead_code)]

pub fn classify(input: i32, enabled: bool) -> i32 {
    let mut total = input;
    if enabled {
        total = total + 1;
    } else {
        total = total + 2;
    }
    let result = total;
    result
}
