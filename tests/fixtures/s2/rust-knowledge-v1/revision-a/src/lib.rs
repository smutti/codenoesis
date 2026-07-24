use crate::catalog::{Item, Store};

pub type ItemId = u64;

pub trait Describable {
    fn describe(&self) -> String;
}

pub mod catalog {
    use super::{Describable, ItemId};

    pub struct Item {
        pub id: ItemId,
        name: String,
    }

    pub enum Store {
        Memory,
        Disk,
    }

    impl Describable for Item {
        fn describe(&self) -> String {
            self.name.clone()
        }
    }

    pub fn make_item(id: ItemId) -> Item {
        Item {
            id,
            name: String::new(),
        }
    }
}

pub fn café_label(item: &catalog::Item) -> String {
    item.describe()
}
