pub(crate) struct ButtonNode {
    pub(crate) label: String,
    pub(crate) armed: bool,
    pub(crate) clicked: bool,
}

impl ButtonNode {
    pub(crate) fn new(label: String) -> Self {
        Self {
            label,
            armed: false,
            clicked: false,
        }
    }
}
