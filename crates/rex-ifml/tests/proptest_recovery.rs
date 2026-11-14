//! Recovery termination proof: arbitrary editor text must never panic and
//! must always return. This is the structural guarantee behind the lenient
//! (LSP) path, not a fuzz test for semantic correctness.

use proptest::prelude::*;
use rex_ifml::{compile_ifml_lenient, IfmlImports};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn lenient_compilation_never_panics(source in "\\PC*") {
        let _ = compile_ifml_lenient("proptest.ifml", &source, &IfmlImports::default());
    }
}
