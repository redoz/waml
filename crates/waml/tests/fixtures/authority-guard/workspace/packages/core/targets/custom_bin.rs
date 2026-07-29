#[path = "bin_authority.rs"]
mod bin_authority;

fn bin_route(input: &[u8]) -> LayoutStatement {
    let _ = input;
    crate::bin_authority::decode("")
}

fn main() {}
