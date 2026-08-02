pub mod model;

pub struct RootService;

impl RootService {
    pub fn name(&self) -> &'static str {
        "root-service"
    }
}
