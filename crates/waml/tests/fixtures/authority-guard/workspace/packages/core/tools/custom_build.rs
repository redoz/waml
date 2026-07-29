#[path = "build_authority.rs"]
mod build_authority;

fn build_shadow(raw: &str) -> LayoutStatement {
    let _ = raw;
    LayoutStatement { parts: Vec::new() }
}

fn build_route(input: &[u8]) -> LayoutStatement {
    let _ = input;
    crate::build_authority::decode("")
}

fn main() {}
