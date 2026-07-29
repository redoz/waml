mod nested;

#[path = "../shared/path_module.rs"]
mod path_module;

include!("../shared/included.rs");
include!(concat!(env!("OUT_DIR"), "/generated.rs"));
