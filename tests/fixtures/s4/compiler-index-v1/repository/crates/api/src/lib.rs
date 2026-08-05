pub trait Store {
    fn get(&self, id: UserId) -> Option<User>;
}

pub struct User {
    pub id: UserId,
}

pub type UserId = u64;

pub type UserAlias = User;

pub struct MemoryStore;

impl Store for MemoryStore {
    fn get(&self, id: UserId) -> Option<User> {
        Some(User { id })
    }
}

pub fn load(store: &impl Store, id: UserId) -> Option<User> {
    store.get(id)
}

pub struct Café;

macro_rules! generated_helper {
    () => {
        pub fn generated_helper() {}
    };
}

generated_helper!();
