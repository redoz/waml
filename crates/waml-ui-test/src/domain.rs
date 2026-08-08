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

    pub const fn new(display: &'static str, value: &'static str) -> Self {
        Self { display, value }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewKind {
    Diagram,
    Source,
}

#[cfg(test)]
mod tests {
    use super::DiagramName;

    #[test]
    fn diagram_name_constructor_keeps_catalog_semantics_together() {
        let diagram = DiagramName::new("Missing", "missing");

        assert_eq!(diagram.display, "Missing");
        assert_eq!(diagram.value, "missing");
    }
}
