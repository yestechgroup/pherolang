//! DD5 (issue pherolang#302): invariants at the consistency boundary.

use crate::support::*;

const ORDER_DOMAIN: &str = r#"
package nz.example.orders

enum OrderStatus {
    Draft = 0
    Approved = 1
}

class Order {
    id readonly String orderNo
    int total
    int lineCount
    OrderStatus status
    refers Customer[0..1] customer
    contains Line[] lines
}

class Line {
    int quantity
    int unitPrice
}

class Customer {
    String name
}

class Money {
    readonly int cents
}
"#;

fn compile_orders(source: &str) -> DddCompilation {
    compile(source, &[("orders.mox", ORDER_DOMAIN)])
}

#[test]
fn aggregate_and_value_invariants_lower_verbatim() {
    let source = concat!(
        "import \"orders.mox\"\n",
        "application A {\n",
        "    base nz.example.orders\n",
        "    module m {\n",
        "        entity Order\n",
        "            invariant total == lines.map(l => l.quantity * l.unitPrice).sum();\n",
        "            invariant total >= 0;\n",
        "            repository OrderRepository {\n",
        "                findById;\n",
        "            }\n",
        "        value Money invariant cents >= 0;\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_orders(source);
    assert_clean(&compilation);
    let model = compilation.model.expect("model");
    let order = &model.modules[0].designs[0];
    assert_eq!(
        order
            .invariants
            .iter()
            .map(|invariant| invariant.expression.as_str())
            .collect::<Vec<_>>(),
        [
            "total == lines.map(l => l.quantity * l.unitPrice).sum()",
            "total >= 0"
        ]
    );
    let money = &model.modules[0].designs[1];
    assert_eq!(money.invariants[0].expression, "cents >= 0");
}

#[test]
fn machine_states_bind_in_invariants() {
    let source = concat!(
        "import \"orders.mox\"\n",
        "application A {\n",
        "    base nz.example.orders\n",
        "    module m {\n",
        "        entity Order\n",
        "            invariant status == Draft || status == Approved;\n",
        "            repository OrderRepository {\n",
        "                findById;\n",
        "            }\n",
        "            machine status: OrderStatus {\n",
        "                initial Draft;\n",
        "                Draft -> Approved via Approve;\n",
        "                terminal Approved;\n",
        "            }\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_orders(source);
    assert_clean(&compilation);
}

#[test]
fn a_non_boolean_invariant_is_refused() {
    let source = concat!(
        "import \"orders.mox\"\n",
        "application A {\n",
        "    base nz.example.orders\n",
        "    module m {\n",
        "        entity Order invariant total + 1;\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_orders(source);
    single(&compilation, "must be boolean");
}

#[test]
fn an_unknown_feature_is_refused() {
    let source = concat!(
        "import \"orders.mox\"\n",
        "application A {\n",
        "    base nz.example.orders\n",
        "    module m {\n",
        "        entity Order invariant missing > 0;\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_orders(source);
    single(&compilation, "E0388");
}

#[test]
fn optional_navigation_must_be_handled() {
    let source = concat!(
        "import \"orders.mox\"\n",
        "application A {\n",
        "    base nz.example.orders\n",
        "    module m {\n",
        "        entity Order invariant customer.name != \"\";\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_orders(source);
    single(&compilation, "E0388");
}

#[test]
fn repository_reads_are_refused() {
    let source = concat!(
        "import \"orders.mox\"\n",
        "application A {\n",
        "    base nz.example.orders\n",
        "    module m {\n",
        "        entity Order\n",
        "            invariant OrderRepository.findById(\"x\") != 0;\n",
        "            repository OrderRepository {\n",
        "                findById;\n",
        "            }\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_orders(source);
    let diagnostic = single(&compilation, "E0388");
    assert!(diagnostic
        .help
        .as_deref()
        .unwrap_or_default()
        .contains("cross-aggregate consistency"));
}

#[test]
fn a_dto_carries_no_invariants() {
    let source = concat!(
        "import \"orders.mox\"\n",
        "application A {\n",
        "    base nz.example.orders\n",
        "    module m {\n",
        "        dto OrderSummary invariant total >= 0; from Order {\n",
        "            orderNo;\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_orders(source);
    single(&compilation, "a transport shape carries no invariants");
}
