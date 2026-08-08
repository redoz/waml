#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagramName {
    #[allow(dead_code)]
    pub(crate) display: &'static str,
    #[allow(dead_code)]
    pub(crate) value: &'static str,
}

impl DiagramName {
    pub const ORDERS: Self = Self {
        display: "Orders",
        value: "orders",
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewKind {
    Diagram,
    Source,
}
