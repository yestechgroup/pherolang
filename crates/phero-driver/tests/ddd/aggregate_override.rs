//! DD4 (issue pherolang#301): authored aggregate boundaries.

use crate::support::*;

const NO_CONTAINMENT: &str = r#"
package nz.example.sales

class Order {
    id readonly String orderNo
    int total
}

class OrderLine {
    id readonly String lineId
    refers Order[0..1] order
    int quantity
    int unitPrice
}
"#;

fn compile_sales(source: &str) -> DddCompilation {
    compile(source, &[("sales.mox", NO_CONTAINMENT)])
}

#[test]
fn authored_root_without_containment_is_a_root() {
    let source = concat!(
        "import \"sales.mox\"\n",
        "application A {\n",
        "    base nz.example.sales\n",
        "    module m {\n",
        "        entity Order root repository OrderRepository { findById; }\n",
        "        entity OrderLine root\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_sales(source);
    assert_clean(&compilation);
    let model = compilation.model.expect("model");
    let designs = &model.modules[0].designs;
    assert_eq!(designs[0].is_aggregate_root, Some(true));
    assert!(designs[0].aggregate.is_none());
    assert_eq!(designs[1].is_aggregate_root, Some(true));
}

#[test]
fn authored_membership_makes_a_non_root_without_containment() {
    let source = concat!(
        "import \"sales.mox\"\n",
        "application A {\n",
        "    base nz.example.sales\n",
        "    module m {\n",
        "        entity Order repository OrderRepository { findById; }\n",
        "        entity OrderLine aggregate Order\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_sales(source);
    assert_clean(&compilation);
    let model = compilation.model.expect("model");
    let line = &model.modules[0].designs[1];
    assert_eq!(line.is_aggregate_root, Some(false));
    let membership = line.aggregate.as_ref().expect("membership");
    assert_eq!(
        membership.root,
        TypeRef::Class {
            package: "nz.example.sales".to_string(),
            name: "Order".to_string(),
        }
    );
    assert_eq!(membership.origin, phero_ir::ddd::AggregateOrigin::Authored);

    let repository = concat!(
        "import \"sales.mox\"\n",
        "application A {\n",
        "    base nz.example.sales\n",
        "    module m {\n",
        "        entity Order repository OrderRepository { findById; }\n",
        "        entity OrderLine aggregate Order repository LineRepository { findById; }\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_sales(repository);
    let diagnostic = single(
        &compilation,
        "only aggregate roots may declare a repository",
    );
    assert!(diagnostic.message.contains("belongs to aggregate 'Order'"));
}

#[test]
fn a_contained_class_cannot_be_declared_a_root() {
    let source = concat!(
        "import \"library.mox\"\n",
        "application A {\n",
        "    base nz.example.library\n",
        "    module m {\n",
        "        entity Library repository LibraryRepository { findById; }\n",
        "        entity Book root repository BookRepository { findById; }\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_domain(source);
    let diagnostic = single(
        &compilation,
        "an owned class cannot be an independent aggregate root",
    );
    assert!(diagnostic
        .help
        .as_deref()
        .unwrap_or_default()
        .contains("remove `root`"));
}

#[test]
fn contradictory_boundary_declarations_are_refused() {
    let source = concat!(
        "import \"library.mox\"\n",
        "application A {\n",
        "    base nz.example.library\n",
        "    module m {\n",
        "        entity Library root aggregate Book\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_domain(source);
    single(&compilation, "E0379");
}

#[test]
fn an_aggregate_target_must_be_an_entity_design_in_the_same_module() {
    let source = concat!(
        "import \"library.mox\"\n",
        "application A {\n",
        "    base nz.example.library\n",
        "    module m {\n",
        "        entity Library repository LibraryRepository { findById; }\n",
        "        entity Book aggregate Library\n",
        "    }\n",
        "    module other {\n",
        "        entity Money aggregate Library\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_domain(source);
    single(&compilation, "belongs to module 'm', not 'other'");
}

#[test]
fn boundary_declarations_are_entity_only() {
    let source = concat!(
        "import \"library.mox\"\n",
        "application A {\n",
        "    base nz.example.library\n",
        "    module m {\n",
        "        value Money root\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_domain(source);
    single(&compilation, "only an entity has an aggregate boundary");
}

#[test]
fn derived_members_carry_the_derived_origin() {
    let source = concat!(
        "import \"library.mox\"\n",
        "application A {\n",
        "    base nz.example.library\n",
        "    module m {\n",
        "        entity Library repository LibraryRepository { findById; }\n",
        "        entity Book\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile_domain(source);
    assert_clean(&compilation);
    let model = compilation.model.expect("model");
    let book = &model.modules[0].designs[1];
    assert_eq!(book.is_aggregate_root, Some(false));
    assert_eq!(
        book.aggregate.as_ref().expect("membership").origin,
        phero_ir::ddd::AggregateOrigin::Derived
    );
}
