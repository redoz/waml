#[path = "example_authority.rs"]
mod example_authority;

fn example_shadow(raw: &str) -> LayoutStatement {
    let _ = raw;
    LayoutStatement { parts: Vec::new() }
}

fn example_route(input: &[u8]) -> LayoutStatement {
    let _ = input;
    crate::example_authority::decode("")
}

fn main() {}
