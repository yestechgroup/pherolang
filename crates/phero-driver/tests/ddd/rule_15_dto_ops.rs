//! `ddd` tests: rule 15 dto ops.

use crate::support::*;

// --- rule 15: the dto-with-ops error -----------------------------------------

/// A class carrying ops — one with an `expr` body, one body-less (the
/// abstract-hook contract) — for the dto-with-ops rule.
const OPS_DOMAIN: &str = r#"
package nz.example.library

class Line {
    String sku
    derived int weight {
        expr { 1 }
    }
    op int scaledWeight(int factor) {
        expr { factor + 1 }
    }
    op boolean blank()
}
"#;

#[test]
fn a_dto_design_with_ops_errors_per_op_and_drops_the_artifact() {
    let source = r#"
import "library.mox";

application Shop {
    module Catalog {
        dto Line
    }
}
"#;
    let compilation = compile(source, &[("library.mox", OPS_DOMAIN)]);
    let errors: Vec<&Diagnostic> = compilation
        .diagnostics
        .iter()
        .filter(|(_, diagnostic)| diagnostic.severity == phero_driver::Severity::Error)
        .map(|(_, diagnostic)| diagnostic)
        .collect();
    assert_eq!(errors.len(), 2, "one error per op: {:?}", errors);
    assert_eq!(
        errors[0].message,
        "E0365 dto design 'Line': class carries op 'scaledWeight'; a dto is a \
         transfer record and carries no behavior"
    );
    assert_eq!(
        errors[1].message,
        "E0365 dto design 'Line': class carries op 'blank'; a dto is a transfer \
         record and carries no behavior"
    );
    assert!(
        compilation.model.is_none(),
        "an error on a dto drops the design artifact"
    );
    assert_eq!(
        compilation.diagnostics.len(),
        2,
        "the op errors are the only diagnostics: {:?}",
        compilation.diagnostics
    );
}

#[test]
fn an_entity_design_with_ops_stays_clean() {
    let source = r#"
import "library.mox";

application Shop {
    module Catalog {
        entity Line
    }
}
"#;
    let compilation = compile(source, &[("library.mox", OPS_DOMAIN)]);
    assert_clean(&compilation);
}

#[test]
fn a_derived_feature_stays_legal_on_a_dto() {
    // `derived` is a computed wire field, not behavior: a mirror DTO copies
    // every *stored* feature and leaves the derived one out, with no error.
    let source = r#"
import "library.mox";

application Shop {
    module Catalog {
        dto Line
    }
}
"#;
    let domain = OPS_DOMAIN.replace(
        "    op int scaledWeight(int factor) {\n        expr { factor + 1 }\n    }\n    op boolean blank()\n",
        "",
    );
    let compilation = compile(source, &[("library.mox", &domain)]);
    assert_clean(&compilation);
    let shape = compilation.model.expect("artifact").modules[0].designs[0]
        .dto_shape
        .clone()
        .expect("the mirror shape");
    assert_eq!(
        shape
            .members
            .iter()
            .map(|member| member.name.as_str())
            .collect::<Vec<_>>(),
        ["sku"],
        "the derived feature is not a stored member"
    );
}
