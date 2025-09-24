use common::error::AResult;

pub trait QueueBackend {
    fn load_queue(path: &Address) -> AResult<Vec<(ItemId, Item)>>;
    fn push_item(i: &Item, path: &Address) -> AResult<ItemId>;
    fn upd_item(i: &ItemId, i: &Item, path: &Address) -> AResult<()>;
    fn remove_item(i: &ItemId, path: &Address) -> AResult<()>;
    fn save_items(items: Vec<Item>, path: &Address) -> AResult<()>;
}

pub struct Address {
    // todo: put the fields actually usually needed to know the physical queue
    // location or address.
}

pub struct Item {
    pub id: ItemId,
}

pub struct ItemId(u64);
