pub mod model {
    pub struct ImportedLocal;
}

use model::ImportedLocal;

pub trait LocalTrait {
    fn local(&self) -> u8;
}

impl ImportedLocal {
    pub fn inherent(&self) -> u8 {
        1
    }
}

impl LocalTrait for ImportedLocal {
    fn local(&self) -> u8 {
        self.inherent()
    }
}
