pub trait TestId {
    fn test_id(self, id: &str) -> Self;
}

impl TestId for egui::Response {
    fn test_id(self, id: &str) -> Self {
        self.ctx
            .accesskit_node_builder(self.id, |node| node.set_author_id(id.to_owned()));
        self
    }
}
