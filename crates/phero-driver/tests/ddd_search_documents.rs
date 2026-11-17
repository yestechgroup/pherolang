//! DF3 (issue pherolang#326): a search's document shape may name a
//! same-module DTO boundary instead of repeating an inline block.
//!
//! The named form lowers to `documentRef` and leaves the inline `document`
//! list empty; the boundary's members are the shape. Every rejection is a
//! spanned error naming the reason.

use phero_driver::{compile_ddd_str, CompileInputs, DddCompilation};
use phero_ir::ddd::DddTypeRef;

const DOMAIN: &str = r#"
package demo.doc

class Order {
    id readonly String orderNo
    int total
}

class Customer {
    id readonly String customerNo
    String name
}
"#;

fn design(body: &str) -> String {
    format!(
        r#"import "domain.mox"

application App {{
    base demo.doc
    module m {{
        entity Order repository OrderRepository {{
            findById;
        }}
{body}
    }}
}}
"#
    )
}

fn compile(source: &str) -> DddCompilation {
    compile_ddd_str(
        "app.ddd",
        source,
        &CompileInputs::from_domains(&[("domain.mox".to_string(), DOMAIN.to_string())]),
    )
}

fn assert_clean(compilation: &DddCompilation) {
    assert!(
        compilation
            .diagnostics
            .iter()
            .all(|(_, diagnostic)| !diagnostic.is_error()),
        "expected a clean compile, got: {:?}",
        compilation.diagnostics
    );
}

fn assert_error(compilation: &DddCompilation, needles: &[&str]) {
    let rendered: Vec<String> = compilation
        .diagnostics
        .iter()
        .map(|(_, diagnostic)| diagnostic.message.clone())
        .collect();
    assert!(
        compilation
            .diagnostics
            .iter()
            .any(|(_, diagnostic)| diagnostic.is_error()),
        "expected an error, got: {rendered:?}"
    );
    let joined = rendered.join("\n");
    for needle in needles {
        assert!(
            joined.contains(needle),
            "expected {needle:?} in the diagnostics, got: {joined}"
        );
    }
}

fn search(compilation: &DddCompilation) -> &phero_ir::ddd::SearchDef {
    &compilation
        .model
        .as_ref()
        .expect("the design compiles")
        .modules[0]
        .searches[0]
}

#[test]
fn a_named_projection_lowers_to_document_ref_and_no_inline_entries() {
    let compilation = compile(&design(
        r#"
        dto OrderSummary from Order {
            orderNo;
            total;
        }
        search OpenOrders {
            entity Order
            text {
                orderNo
            }
            document OrderSummary;
        }"#,
    ));
    assert_clean(&compilation);
    let search = search(&compilation);
    assert_eq!(
        search.document_ref,
        Some(DddTypeRef::Dto {
            dto: "OrderSummary".to_string()
        })
    );
    assert!(
        search.document.is_empty(),
        "a named document carries no inline entries"
    );
}

#[test]
fn the_named_and_inline_forms_describe_the_same_members() {
    let named = compile(&design(
        r#"
        dto OrderSummary from Order {
            orderNo;
            total;
        }
        search OpenOrders {
            entity Order
            text {
                orderNo
            }
            document OrderSummary;
        }"#,
    ));
    let inline = compile(&design(
        r#"
        search OpenOrders {
            entity Order
            text {
                orderNo
            }
            document {
                orderNo = orderNo;
                total = total;
            }
        }"#,
    ));
    assert_clean(&named);
    assert_clean(&inline);
    let named_model = named.model.as_ref().expect("the named design compiles");
    let shape = named_model.modules[0]
        .designs
        .iter()
        .find_map(|design| design.dto_shape.as_ref())
        .expect("the boundary lowers");
    let inline_names: Vec<&str> = inline
        .model
        .as_ref()
        .expect("the inline design compiles")
        .modules[0]
        .searches[0]
        .document
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    let shape_names: Vec<&str> = shape
        .members
        .iter()
        .map(|member| member.name.as_str())
        .collect();
    assert_eq!(shape_names, inline_names);
}

#[test]
fn a_standalone_dto_is_refused() {
    let compilation = compile(&design(
        r#"
        dto Standalone {
            String orderNo;
        }
        search OpenOrders {
            entity Order
            text {
                orderNo
            }
            document Standalone;
        }"#,
    ));
    assert_error(&compilation, &["E0400", "standalone dto"]);
}

#[test]
fn a_cross_module_dto_is_refused() {
    let source = r#"import "domain.mox"

application App {
    base demo.doc
    module m {
        entity Order repository OrderRepository {
            findById;
        }
        search OpenOrders {
            entity Order
            text {
                orderNo
            }
            document OrderSummary;
        }
    }
    module other {
        dto OrderSummary from Order {
            orderNo;
        }
    }
}
"#;
    let compilation = compile(source);
    assert_error(&compilation, &["E0400", "module 'other'"]);
}

#[test]
fn a_boundary_projecting_another_entity_is_refused() {
    let compilation = compile(&design(
        r#"
        dto CustomerSummary from Customer {
            customerNo;
        }
        search OpenOrders {
            entity Order
            text {
                orderNo
            }
            document CustomerSummary;
        }"#,
    ));
    assert_error(
        &compilation,
        &["E0401", "demo.doc.Customer", "demo.doc.Order"],
    );
}

#[test]
fn an_unknown_document_name_is_refused() {
    let compilation = compile(&design(
        r#"
        search OpenOrders {
            entity Order
            text {
                orderNo
            }
            document Missing;
        }"#,
    ));
    assert_error(&compilation, &["E0400", "Missing"]);
}

#[test]
fn an_inline_and_named_document_together_are_refused() {
    let compilation = compile(&design(
        r#"
        dto OrderSummary from Order {
            orderNo;
        }
        search OpenOrders {
            entity Order
            text {
                orderNo
            }
            document {
                orderNo = orderNo;
            }
            document OrderSummary;
        }"#,
    ));
    assert_error(&compilation, &["E0402"]);
}
