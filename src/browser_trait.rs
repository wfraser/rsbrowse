pub trait Browser {
    type Item: Clone + Send + Sync;
    type ItemId: Clone + Send + Sync;
    fn list_crates(&self) -> Vec<(String, Self::ItemId)>;
    #[allow(clippy::type_complexity)]
    fn list_items(&self, parent_id: &Self::ItemId) -> Vec<(String, (Self::ItemId, Self::Item))>;
    fn get_info(&self, item: &Self::Item) -> String;
    fn get_debug_info(&self, item: &Self::Item) -> String;
    fn get_source(&self, item: &Self::Item) -> (String, Option<usize>);
}
