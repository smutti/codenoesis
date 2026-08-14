#![allow(dead_code)]

pub const DEFAULT_LIMIT: i32 = 8;

pub fn clamp(input: i32, limit: i32) -> i32 {
    if input > limit { limit } else { input }
}

pub struct Calculator {
    factor: i32,
}

impl Calculator {
    pub fn scale<T>(&self, value: i32, fallback: T) -> Result<i32, T>
    where
        T: Clone,
    {
        let scaled = value * self.factor;
        if scaled > DEFAULT_LIMIT {
            Ok(clamp(scaled, DEFAULT_LIMIT))
        } else {
            Err(fallback.clone())
        }
    }
}
