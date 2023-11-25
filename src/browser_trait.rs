pub trait Browser {
    type CrateId: Clone;
    type Item: Item + Clone;
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
    fn crate_root() -> Self;
}
