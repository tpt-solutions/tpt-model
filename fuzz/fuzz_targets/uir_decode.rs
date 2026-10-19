#![no_main]

use libfuzzer_sys::fuzz_target;

// Garbage bytes must produce a UirError, never a panic or an out-of-bounds read.
fuzz_target!(|data: &[u8]| {
    if let Ok(node) = tpt_uir_stub::decode(data) {
        // Anything that decodes must re-encode to the same bytes.
        assert_eq!(tpt_uir_stub::encode(&node), data);
    }
});
