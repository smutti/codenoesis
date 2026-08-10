#[derive(Clone)]
pub struct Worker {
    pub name: String,
}

impl Worker {
    #[cfg(target_family = "unix")]
    pub fn run(&self, input: i32) -> i32 {
        crate::helper(input)
    }

    #[cfg(target_family = "windows")]
    pub fn run(&self, value: usize) -> i32 {
        crate::helper(value as i32)
    }
}
