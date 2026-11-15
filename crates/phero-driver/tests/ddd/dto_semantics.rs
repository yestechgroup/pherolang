//! `ddd` tests: DTO boundaries (DD3, issue pherolang#300) — the mirror,
//! projection, and standalone forms, their member typing and direction, the
//! input/output rule, and the DTO error surface.

use crate::support::*;
use phero_ir::ddd::{
    DddTypeRef, Design, DtoDirection, DtoMember, DtoMemberKind, DtoShape, Stereotype,
};
use phero_ir::{Multiplicity, PrimitiveType, TypeRef};

/// A catalog domain exercising inheritance, an enum, a value-shaped class, a
/// derived feature, class-typed features, and an operation (so the dto-op
/// rule and the projection exemption are both reachable).
const DTO_DOMAIN: &str = r#"
package nz.example.catalog

enum Category {
    Fiction as "F" = 0
}

enum OrderState {
    New as "new" = 0
}

class Author {
    String fullName
}

class Base {
    String title
}

class Book extends Base {
    id readonly String bookId
    String isbn
    int playLength
    Category category
    refers Author author
    refers Author[] coauthors
    derived String label {
        expr { title }
    }
    op boolean longEnough(int minutes) {
        expr { playLength > minutes }
    }
}

class Loan {
    String code
    refers Book book
}

class Order {
    OrderState status
}

class Stamp {
    String code
    derived String label {
        expr { code }
    }
}

class User {
    String name
    boolean active
}
"#;

fn compile_dto(source: &str) -> DddCompilation {
    let source = format!("import \"dto.mox\";\n{source}");
    compile(&source, &[("dto.mox", DTO_DOMAIN)])
}

fn design_by_dto<'a>(compilation: &'a DddCompilation, dto: &str) -> &'a Design {
    compilation
        .model
        .as_ref()
        .expect("artifact")
        .modules
        .iter()
        .flat_map(|module| &module.designs)
        .find(|design| {
            design
                .dto_shape
                .as_ref()
                .is_some_and(|shape| shape.name == dto)
        })
        .unwrap_or_else(|| panic!("expected the dto design `{dto}`"))
}

fn shape<'a>(compilation: &'a DddCompilation, dto: &str) -> &'a DtoShape {
    design_by_dto(compilation, dto)
        .dto_shape
        .as_ref()
        .expect("a dto shape")
}

fn domain(type_ref: TypeRef) -> DddTypeRef {
    DddTypeRef::Domain(type_ref)
}

fn string() -> DddTypeRef {
    domain(TypeRef::Primitive(PrimitiveType::String))
}

fn int() -> DddTypeRef {
    domain(TypeRef::Primitive(PrimitiveType::Int))
}

fn class(name: &str) -> TypeRef {
    TypeRef::Class {
        package: "nz.example.catalog".to_string(),
        name: name.to_string(),
    }
}

fn assert_member(member: &DtoMember, name: &str, kind: DtoMemberKind, type_: DddTypeRef) {
    assert_eq!(member.name, name, "member order");
    assert_eq!(member.kind, kind, "kind of '{name}'");
    assert_eq!(member.type_, type_, "type of '{name}'");
}

// --- the three forms --------------------------------------------------------

#[test]
fn a_mirror_dto_copies_every_stored_feature() {
    let source = "application Catalog { module m { dto User nonPersistent } }";
    let compilation = compile_dto(source);
    assert_clean(&compilation);
    let design = design_by_dto(&compilation, "User");
    assert_eq!(design.stereotype, Stereotype::Dto);
    assert_eq!(design.class.as_deref(), Some("User"));
    assert_eq!(design.type_ref, Some(class("User")));
    let shape = shape(&compilation, "User");
    assert_eq!(shape.source, Some(class("User")));
    assert_eq!(shape.direction, DtoDirection::Input);
    assert_eq!(
        shape
            .members
            .iter()
            .map(|member| member.name.as_str())
            .collect::<Vec<_>>(),
        ["name", "active"],
        "stored features in declaration order"
    );
    assert_member(&shape.members[0], "name", DtoMemberKind::Select, string());
    assert_eq!(shape.members[0].path, ["name"]);
    assert_eq!(shape.members[0].multiplicity, Some(Multiplicity::REQUIRED));
    assert_member(
        &shape.members[1],
        "active",
        DtoMemberKind::Select,
        domain(TypeRef::Primitive(PrimitiveType::Boolean)),
    );
}

#[test]
fn a_mirror_dto_excludes_derived_features() {
    let source = "application Catalog { module m { dto Stamp } }";
    let compilation = compile_dto(source);
    assert_clean(&compilation);
    let shape = shape(&compilation, "Stamp");
    assert_eq!(
        shape
            .members
            .iter()
            .map(|member| member.name.as_str())
            .collect::<Vec<_>>(),
        ["code"],
        "the derived feature is not a stored member"
    );
}

#[test]
fn selecting_a_derived_source_feature_lowers_as_a_derivation() {
    let source =
        "application Catalog { module m { dto StampSummary from Stamp { code; label; } } }";
    let compilation = compile_dto(source);
    assert_clean(&compilation);
    let shape = shape(&compilation, "StampSummary");
    assert_eq!(shape.direction, DtoDirection::Output);
    assert_eq!(shape.members.len(), 2);
    assert_eq!(shape.members[0].kind, DtoMemberKind::Select);
    assert_eq!(shape.members[0].path, ["code"]);
    assert_eq!(shape.members[1].kind, DtoMemberKind::Derive);
    assert_eq!(shape.members[1].expression.as_deref(), Some("label"));
    assert!(
        shape.members[1].path.is_empty(),
        "a derivation carries no selected path"
    );
}

#[test]
fn a_projection_selects_renames_and_derives_in_declaration_order() {
    let source = concat!(
        "application Catalog { module m {\n",
        "    dto BookSummary from Book {\n",
        "        title;\n",
        "        isbn;\n",
        "        rating = playLength / 60;\n",
        "        authorName = author.fullName;\n",
        "    }\n",
        "} }\n",
    );
    let compilation = compile_dto(source);
    assert_clean(&compilation);
    let design = design_by_dto(&compilation, "BookSummary");
    // A projection's class is its source; the boundary name rides in the
    // shape.
    assert_eq!(design.class.as_deref(), Some("Book"));
    assert_eq!(design.type_ref, Some(class("Book")));
    let shape = shape(&compilation, "BookSummary");
    assert_eq!(shape.source, Some(class("Book")));
    assert_eq!(shape.direction, DtoDirection::Output);
    assert_eq!(shape.members.len(), 4);
    assert_member(&shape.members[0], "title", DtoMemberKind::Select, string());
    assert_eq!(shape.members[0].path, ["title"]);
    assert_member(&shape.members[1], "isbn", DtoMemberKind::Select, string());
    assert_member(&shape.members[2], "rating", DtoMemberKind::Derive, int());
    assert_eq!(
        shape.members[2].expression.as_deref(),
        Some("playLength / 60")
    );
    assert!(shape.members[2].path.is_empty());
    assert_member(
        &shape.members[3],
        "authorName",
        DtoMemberKind::Select,
        string(),
    );
    assert_eq!(shape.members[3].path, ["author", "fullName"]);
    assert!(
        shape.members[3].expression.is_none(),
        "a plain path is a selection, not a derivation"
    );
}

#[test]
fn a_projection_without_derivations_is_input_capable() {
    let source = "application Catalog { module m { dto BookTitle from Book { title; isbn; } } }";
    let compilation = compile_dto(source);
    assert_clean(&compilation);
    let design = design_by_dto(&compilation, "BookTitle");
    assert!(design.dto_shape.as_ref().expect("shape").direction == DtoDirection::Input);
}

#[test]
fn a_standalone_dto_has_no_source_class() {
    let source = concat!(
        "application Catalog { module m {\n",
        "    dto CreateBookRequest {\n",
        "        String title;\n",
        "        Category category;\n",
        "        int[] tags;\n",
        "    }\n",
        "} }\n",
    );
    let compilation = compile_dto(source);
    assert_clean(&compilation);
    let design = design_by_dto(&compilation, "CreateBookRequest");
    assert_eq!(design.class, None);
    assert_eq!(design.type_ref, None);
    let shape = shape(&compilation, "CreateBookRequest");
    assert_eq!(shape.source, None);
    assert_eq!(shape.direction, DtoDirection::Input);
    assert_eq!(shape.members.len(), 3);
    assert_member(
        &shape.members[0],
        "title",
        DtoMemberKind::Declared,
        string(),
    );
    assert_member(
        &shape.members[1],
        "category",
        DtoMemberKind::Declared,
        domain(TypeRef::Enum {
            package: "nz.example.catalog".to_string(),
            name: "Category".to_string(),
        }),
    );
    assert_member(&shape.members[2], "tags", DtoMemberKind::Declared, int());
    assert_eq!(shape.members[2].multiplicity, Some(Multiplicity::MANY));
    assert!(shape.members.iter().all(|member| member.path.is_empty()));
}

#[test]
fn standalone_members_may_embed_later_declared_dtos() {
    let source = concat!(
        "application Catalog { module m {\n",
        "    dto Envelope {\n",
        "        CreateBookRequest request;\n",
        "        Category category;\n",
        "    }\n",
        "    dto CreateBookRequest {\n",
        "        String title;\n",
        "    }\n",
        "} }\n",
    );
    let compilation = compile_dto(source);
    assert_clean(&compilation);
    let shape = shape(&compilation, "Envelope");
    assert_eq!(shape.members.len(), 2);
    assert_member(
        &shape.members[0],
        "request",
        DtoMemberKind::Declared,
        DddTypeRef::dto("CreateBookRequest"),
    );
    assert_eq!(
        shape.members[1].type_,
        domain(TypeRef::Enum {
            package: "nz.example.catalog".to_string(),
            name: "Category".to_string(),
        })
    );
}

#[test]
fn dtos_fill_service_return_and_parameter_slots() {
    let source = concat!(
        "application Catalog { module m {\n",
        "    service CatalogService {\n",
        "        BookTitle title(String isbn);\n",
        "        boolean create(CreateBookRequest request);\n",
        "    }\n",
        "    dto BookTitle from Book { title; }\n",
        "    dto CreateBookRequest { String title; }\n",
        "} }\n",
    );
    let compilation = compile_dto(source);
    assert_clean(&compilation);
    let service = &compilation.model.as_ref().expect("artifact").modules[0].services[0];
    let title = &service.operations[0];
    assert_eq!(title.return_type, Some(DddTypeRef::dto("BookTitle")));
    let create = &service.operations[1];
    assert_eq!(create.params[0].type_, DddTypeRef::dto("CreateBookRequest"));
}

// --- errors -----------------------------------------------------------------

fn error<'a>(compilation: &'a DddCompilation, needle: &str) -> &'a Diagnostic {
    let matches: Vec<&Diagnostic> = compilation
        .diagnostics
        .iter()
        .filter(|(_, diagnostic)| diagnostic.message.contains(needle))
        .map(|(_, diagnostic)| diagnostic)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected one diagnostic containing '{needle}', got: {:?}",
        compilation.diagnostics
    );
    matches[0]
}

#[test]
fn duplicate_output_member_names_are_rejected() {
    let source = "application Catalog { module m { dto Summary from Book { title; title; } } }";
    let compilation = compile_dto(source);
    let diagnostic = error(
        &compilation,
        "E0366 dto 'Summary': duplicate member 'title'",
    );
    assert!(compilation.model.is_none());
    assert_eq!(diagnostic.severity, phero_driver::Severity::Error);
}

#[test]
fn an_unknown_selected_feature_is_rejected() {
    let source = "application Catalog { module m { dto Summary from Book { nope; } } }";
    let compilation = compile_dto(source);
    let diagnostic = error(&compilation, "E0367 unknown dto member 'nope' on 'Summary'");
    assert!(
        diagnostic
            .help
            .as_deref()
            .unwrap_or_default()
            .contains("title"),
        "the help lists the source features: {:?}",
        diagnostic.help
    );
}

#[test]
fn an_unknown_path_segment_is_rejected() {
    let source = "application Catalog { module m { dto Summary from Book { x = author.nope; } } }";
    let compilation = compile_dto(source);
    error(&compilation, "E0367 unknown dto path 'author.nope'");
}

#[test]
fn a_mistyped_derivation_is_rejected() {
    let source =
        "application Catalog { module m { dto Summary from Book { rating = title + true; } } }";
    let compilation = compile_dto(source);
    error(
        &compilation,
        "E0368 invalid dto member 'rating' of 'Summary'",
    );
}

#[test]
fn an_unresolved_standalone_member_type_is_rejected() {
    let source = "application Catalog { module m { dto Request { NoSuchType value; } } }";
    let compilation = compile_dto(source);
    let diagnostic = error(
        &compilation,
        "E0368 invalid dto member 'value' of 'Request'",
    );
    assert!(
        diagnostic.message.contains("unknown type 'NoSuchType'"),
        "the member context wraps the resolution error: {}",
        diagnostic.message
    );
}

#[test]
fn a_standalone_member_may_not_embed_a_designed_entity() {
    let source = concat!(
        "application Catalog { module m {\n",
        "    entity Book\n",
        "    dto Snapshot { Book book; }\n",
        "} }\n",
    );
    let compilation = compile_dto(source);
    error(
        &compilation,
        "E0369 dto member 'book' of 'Snapshot' carries entity 'Book'",
    );
}

#[test]
fn a_selected_entity_path_is_rejected() {
    let source = concat!(
        "application Catalog { module m {\n",
        "    entity Book\n",
        "    dto LoanSummary from Loan { code; book; }\n",
        "} }\n",
    );
    let compilation = compile_dto(source);
    error(
        &compilation,
        "E0369 dto member 'book' of 'LoanSummary' carries entity 'Book'",
    );
}

#[test]
fn an_output_only_dto_is_refused_as_a_parameter() {
    let source = concat!(
        "application Catalog { module m {\n",
        "    service CatalogService { boolean create(BookSummary summary); }\n",
        "    dto BookSummary from Book { title; rating = playLength / 60; }\n",
        "} }\n",
    );
    let compilation = compile_dto(source);
    let diagnostic = error(
        &compilation,
        "E0370 dto 'BookSummary' is output-only: parameter 'summary' of operation \
         'create' of service 'CatalogService' is not input-capable",
    );
    assert!(
        diagnostic.message.contains("rating"),
        "the diagnostic names the derived member: {}",
        diagnostic.message
    );
}

#[test]
fn a_projection_dto_and_its_source_entity_design_coexist() {
    let source = concat!(
        "application Catalog { module m {\n",
        "    entity Book\n",
        "    dto BookSummary from Book { title; }\n",
        "} }\n",
    );
    let compilation = compile_dto(source);
    assert_clean(&compilation);
    assert_eq!(
        compilation.model.as_ref().expect("artifact").modules[0]
            .designs
            .len(),
        2
    );
}

#[test]
fn a_dto_name_may_not_shadow_a_domain_type() {
    let source = "application Catalog { module m { dto Book { String title; } } }";
    let compilation = compile_dto(source);
    let diagnostic = error(&compilation, "E0371 dto 'Book' collides with class 'Book'");
    assert_eq!(diagnostic.severity, phero_driver::Severity::Error);
}

#[test]
fn duplicate_dto_boundary_names_are_rejected() {
    let source = concat!(
        "application Catalog {\n",
        "    module a { dto Request { String title; } }\n",
        "    module b { dto Request { String isbn; } }\n",
        "}\n",
    );
    let compilation = compile_dto(source);
    error(&compilation, "E0371 duplicate dto 'Request'");
}

#[test]
fn a_mirror_dto_name_is_exempt_from_the_collision_rule() {
    let source = "application Catalog { module m { dto User } }";
    let compilation = compile_dto(source);
    assert_clean(&compilation);
}

#[test]
fn a_dto_carrying_a_repository_machine_or_entity_flag_is_rejected() {
    for (design, needle) in [
        (
            "dto User repository UserRepository { findById; }",
            "E0372 repository 'UserRepository' requires an entity design",
        ),
        (
            "dto Order machine status: OrderState { initial New; }",
            "E0372 dto design 'Order' carries a machine block",
        ),
        (
            "dto User scaffold",
            "E0372 flag 'scaffold' requires an entity design",
        ),
        (
            "dto User auditable",
            "E0372 flag 'auditable' requires an entity design",
        ),
        (
            "dto User optimisticLocking",
            "E0372 flag 'optimisticLocking' requires an entity design",
        ),
    ] {
        let source = format!("application Catalog {{ module m {{ {design} }} }}");
        let compilation = compile_dto(&source);
        error(&compilation, needle);
    }
}

#[test]
fn a_dto_value_is_opaque_to_statement_bodies() {
    let source = concat!(
        "application Catalog { module m {\n",
        "    service CatalogService {\n",
        "        inject BookRepository;\n",
        "        String titleOf(CreateBookRequest request) => BookRepository {\n",
        "            return request.title;\n",
        "        }\n",
        "    }\n",
        "    entity Book repository BookRepository { save; }\n",
        "    dto CreateBookRequest { String title; }\n",
        "} }\n",
    );
    let compilation = compile_dto(source);
    let diagnostic = error(&compilation, "E0360");
    assert!(
        diagnostic
            .message
            .contains("unknown feature `title` on class `CreateBookRequest`"),
        "a DTO is opaque: its field access is a clear unknown-feature error, not an \
         unknown-class one — {}",
        diagnostic.message
    );
}

#[test]
fn non_persistent_and_cache_stay_legal_on_a_dto() {
    let source = "application Catalog { module m { dto User nonPersistent cache } }";
    let compilation = compile_dto(source);
    assert_clean(&compilation);
    let design = design_by_dto(&compilation, "User");
    assert!(design.flags.non_persistent && design.flags.cache);
}
