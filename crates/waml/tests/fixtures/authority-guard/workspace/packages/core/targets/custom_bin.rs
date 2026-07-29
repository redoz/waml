#[path = "bin_authority.rs"]
mod bin_authority;

fn bin_route(input: &[u8]) -> LayoutStatement {
    let _ = input;
    crate::bin_authority::decode("")
}

struct TargetServices {
    decoder: BinDecoder,
}

impl TargetServices {
    fn decoder(&self) -> &BinDecoder {
        &self.decoder
    }
}

struct BinDecoder;

impl BinDecoder {
    fn decode(&self, raw: String) -> Analysis {
        let _ = crate::bin_authority::decode(&raw);
        Analysis
    }
}

fn bin_unsafe_field(services: &TargetServices, model: &Model) -> Analysis {
    let rendered = model.to_string();
    services.decoder.decode(rendered)
}

fn bin_unsafe_chain(services: &TargetServices, model: &Model) -> Analysis {
    let rendered = model.to_string();
    services.decoder().decode(rendered)
}

fn main() {}
