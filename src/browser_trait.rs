pub trait Browser {
    type CrateId: Clone;
    type Item: Item<CrateId = Self::CrateId> + Clone;
    fn list_crates(&self) -> Vec<(String, Self::CrateId)>;
    fn list_items(
        &self,
        crate_id: &Self::CrateId,
        parent: &Self::Item,
    ) -> Vec<(String, Self::Item)>;
    fn get_info(&self, crate_id: &Self::CrateId, item: &Self::Item) -> String;
    fn get_debug_info(&self, crate_id: &Self::CrateId, item: &Self::Item) -> String;
    fn get_source(&self, item: &Self::Item) -> (String, Option<usize>);
}

pub trait Item {
    type CrateId;
    fn crate_root() -> Self;
    fn crate_id<'a>(&'a self, crate_id: &'a Self::CrateId) -> &'a Self::CrateId;
}
