//! DD9 (issue pherolang#306): typed repository queries.
//!
//! A query operation is read-only by construction: a `where` predicate typed
//! against the designed class plus the operation's parameters, an explicit
//! `sort` over direct stored scalar features, and bounded `page` metadata.
//! Every negative case here is a spanned compile error, and the positive case
//! lowers the resolved shapes the consumer acts on.

use phero_driver::compile_ddd_str;
use phero_ir::ddd::{RepositoryOperation, SortDirection};
use phero_ir::{PrimitiveType, TypeRef};

const DOMAIN: &str = r#"
package demo.q

enum State {
    Open as "open" = 0
    Closed as "closed" = 1
}

class Customer {
    id readonly String customerNo
    String name
}

class Order {
    id readonly String orderNo
    int total
    date createdDate
    State state
    refers Customer[0..1] customer
    refers Customer[] watchers

    derived boolean big {
        expr { total > 1000 }
    }
}
"#;

/// A design whose single repository carries `op` as its declared operation.
fn design(op: &str) -> String {
    format!(
        r#"import "domain.mox"

application App {{
    base demo.q
    module m {{
        entity Order repository OrderRepository {{
            findById;
{op}
        }}
    }}
}}
"#
    )
}

fn compile(op: &str) -> phero_driver::DddCompilation {
    compile_ddd_str(
        "app.ddd",
        &design(op),
        &phero_driver::CompileInputs::from_domains(&[(
            "domain.mox".to_string(),
            DOMAIN.to_string(),
        )]),
    )
}

fn operation(compilation: &phero_driver::DddCompilation, name: &str) -> RepositoryOperation {
    let model = compilation.model.as_ref().expect("the design compiles");
    model.modules[0].designs[0]
        .repository
        .as_ref()
        .expect("the repository lowers")
        .operations
        .iter()
        .find(|operation| operation.name == name)
        .expect("the operation is declared")
        .clone()
}

/// Asserts one diagnostic whose message contains every needle.
fn assert_diagnostic(compilation: &phero_driver::DddCompilation, needles: &[&str]) {
    assert!(
        compilation.model.is_none(),
        "the design must not lower: {:?}",
        compilation.model
    );
    let found = compilation.diagnostics.iter().any(|(_, diagnostic)| {
        needles
            .iter()
            .all(|needle| diagnostic.message.contains(needle))
    });
    assert!(
        found,
        "no diagnostic contains {needles:?}: {:#?}",
        compilation.diagnostics
    );
}

// --- positive shapes ---------------------------------------------------------

#[test]
fn a_query_lowers_the_predicate_sort_types_and_pagination() {
    let compilation = compile(
        r#"            Order[] openFor(State targetState, int minimumTotal)
                where (self.state == targetState && self.total >= minimumTotal)
                sort createdDate desc, orderNo
                page limit 20 max 100
                capability ReadOrders;"#,
    );
    assert!(
        compilation.diagnostics.is_empty(),
        "{:?}",
        compilation.diagnostics
    );
    let operation = operation(&compilation, "openFor");
    let query = operation.query.as_ref().expect("the query lowers");
    assert_eq!(
        query.predicate, "self.state == targetState && self.total >= minimumTotal",
        "the predicate is verbatim source"
    );
    assert!(query.read_only, "readOnly is lowered, never inferred");
    assert_eq!(query.sort.len(), 2);
    assert_eq!(query.sort[0].field, "createdDate");
    assert_eq!(
        query.sort[0].field_type,
        TypeRef::Primitive(PrimitiveType::Date)
    );
    assert_eq!(query.sort[0].direction, SortDirection::Desc);
    assert_eq!(query.sort[1].field, "orderNo");
    assert_eq!(query.sort[1].direction, SortDirection::Asc);
    let pagination = query.pagination.as_ref().expect("pagination lowers");
    assert_eq!(pagination.limit, Some(20));
    assert_eq!(pagination.max, Some(100));
    assert!(!pagination.cursor);
    assert_eq!(
        operation
            .capabilities
            .iter()
            .map(|capability| capability.name.as_str())
            .collect::<Vec<_>>(),
        ["ReadOrders"],
        "a query may be capability-gated"
    );
}

#[test]
fn a_single_result_finder_lowers_the_optional_cardinality() {
    let compilation =
        compile("            Order[0..1] byNumber(String number) where (number == self.orderNo);");
    assert!(
        compilation.diagnostics.is_empty(),
        "{:?}",
        compilation.diagnostics
    );
    let operation = operation(&compilation, "byNumber");
    let multiplicity = operation
        .return_multiplicity
        .as_ref()
        .expect("the cardinality lowers");
    assert_eq!(multiplicity.lower, 0);
    assert_eq!(multiplicity.upper, phero_ir::Upper::Finite(1));
    assert!(operation.query.is_some());
}

#[test]
fn a_query_returns_a_dto_projection() {
    let source = concat!(
        "import \"domain.mox\"\n",
        "\n",
        "application App {\n",
        "    base demo.q\n",
        "    module m {\n",
        "        entity Order repository OrderRepository {\n",
        "            findById;\n",
        "            OrderSummary[] summaries(String number) where (number == self.orderNo);\n",
        "        }\n",
        "        dto OrderSummary from Order {\n",
        "            orderNo;\n",
        "            total;\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_ddd_str(
        "app.ddd",
        source,
        &phero_driver::CompileInputs::from_domains(&[(
            "domain.mox".to_string(),
            DOMAIN.to_string(),
        )]),
    );
    assert!(
        compilation.diagnostics.is_empty(),
        "{:?}",
        compilation.diagnostics
    );
    let operation = operation(&compilation, "summaries");
    assert!(operation.query.is_some());
    assert!(
        matches!(
            operation.return_type,
            Some(phero_ir::ddd::DddTypeRef::Dto { .. })
        ),
        "the DTO boundary is the return type: {:?}",
        operation.return_type
    );
}

#[test]
fn a_cursor_query_lowers_the_cursor_hint() {
    let compilation = compile(
        "            Order[] shipped(State targetState) where (self.state == targetState) page cursor;",
    );
    assert!(
        compilation.diagnostics.is_empty(),
        "{:?}",
        compilation.diagnostics
    );
    let operation = operation(&compilation, "shipped");
    let pagination = operation
        .query
        .as_ref()
        .expect("query")
        .pagination
        .as_ref()
        .expect("pagination");
    assert!(pagination.cursor);
    assert_eq!(pagination.limit, None);
    assert_eq!(pagination.max, None);
}

// --- negative matrix ---------------------------------------------------------

#[test]
fn a_non_boolean_predicate_is_refused() {
    let compilation = compile("            Order[] o(int total) where (total);");
    assert_diagnostic(&compilation, &["E0397", "must be boolean"]);
}

#[test]
fn an_unknown_feature_in_the_predicate_is_refused() {
    let compilation = compile("            Order[] o() where (missing == 1);");
    assert_diagnostic(&compilation, &["E0397"]);
}

#[test]
fn a_parameter_feature_collision_is_refused_with_the_self_help() {
    let compilation = compile("            Order[] o(int total) where (total == 1);");
    assert_diagnostic(&compilation, &["E0397", "collides"]);
    let help = compilation
        .diagnostics
        .iter()
        .find_map(|(_, diagnostic)| diagnostic.help.as_deref())
        .unwrap_or_default();
    assert!(
        help.contains("self.total"),
        "the help names the self. spelling: {help}"
    );
}

#[test]
fn sorting_on_a_class_reference_is_refused() {
    let compilation = compile("            Order[] o() where (self.total > 0) sort customer;");
    assert_diagnostic(&compilation, &["E0398", "class reference", "customer"]);
}

#[test]
fn sorting_on_a_derived_feature_is_refused() {
    let compilation = compile("            Order[] o() where (self.total > 0) sort big;");
    assert_diagnostic(&compilation, &["E0398", "derived", "big"]);
}

#[test]
fn sorting_on_a_collection_is_refused() {
    let compilation = compile("            Order[] o() where (self.total > 0) sort watchers;");
    assert_diagnostic(&compilation, &["E0398", "collection", "watchers"]);
}

#[test]
fn a_limit_above_the_max_is_refused() {
    let compilation =
        compile("            Order[] o() where (self.total > 0) page limit 200 max 100;");
    assert_diagnostic(&compilation, &["E0399", "limit", "max"]);
}

#[test]
fn a_non_positive_page_size_is_refused() {
    let compilation = compile("            Order[] o() where (self.total > 0) page limit 0;");
    assert_diagnostic(&compilation, &["E0399", "positive"]);
}

#[test]
fn a_query_cannot_carry_a_body() {
    // The grammar has no body form after query clauses, so this is a named
    // parse error rather than a silently dropped body.
    let compilation = compile("            Order[] o() where (self.total > 0) { return Order[]; }");
    assert!(
        compilation.model.is_none() && !compilation.diagnostics.is_empty(),
        "a query body is refused: {:?}",
        compilation.diagnostics
    );
}

#[test]
fn a_query_cannot_carry_a_when_guard() {
    let compilation =
        compile("            Order[] o() where (self.total > 0) when (self.total > 1);");
    assert_diagnostic(&compilation, &["query", "when"]);
}

#[test]
fn a_query_cannot_carry_throws() {
    let compilation = compile("            Order[] o() throws Conflict where (self.total > 0);");
    assert_diagnostic(&compilation, &["E0396", "throws"]);
}
