mod nested;
mod lib_authority;

#[path = "../shared/path_module.rs"]
mod path_module;

include!("../shared/included.rs");
include!(concat!(env!("OUT_DIR"), "/generated.rs"));

fn body_include_host() {
    include!("../shared/body_included.rs");
}

fn body_expression_host(text: SourceText) {
    include!("../shared/body_expression.rs");
}

fn lib_route(input: &[u8]) -> LayoutStatement {
    let _ = input;
    crate::lib_authority::decode("")
}

struct TargetServices {
    decoder: LibDecoder,
}

impl TargetServices {
    fn decoder(&self) -> &LibDecoder {
        &self.decoder
    }
}

struct LibDecoder;

impl LibDecoder {
    fn decode(&self, _raw: String) -> Analysis {
        Analysis
    }
}

fn lib_safe_field(services: &TargetServices, model: &Model) -> Analysis {
    let rendered = model.to_string();
    services.decoder.decode(rendered)
}

fn lib_safe_chain(services: &TargetServices, model: &Model) -> Analysis {
    let rendered = model.to_string();
    services.decoder().decode(rendered)
}
