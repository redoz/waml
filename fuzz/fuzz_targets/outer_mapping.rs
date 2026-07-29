#![no_main]

mod support;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some(value) = support::valid_utf8(data) else {
        return;
    };
    support::assert_shell_invariants(value);
});
