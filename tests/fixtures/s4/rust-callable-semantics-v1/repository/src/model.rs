#[derive(Clone)]
pub struct Worker {
    pub name: String,
}

impl Worker {
    pub fn run(&self, input: i32) -> i32 {
        crate::helper(input)
    }
}
