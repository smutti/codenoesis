use api::{load, Café, MemoryStore, Store, User, UserId};

pub fn fetch(id: UserId) -> Option<User> {
    let store = MemoryStore;
    let _marker = Café;
    load(&store, id)
}

pub fn accepts_store(_store: &dyn Store) {}
