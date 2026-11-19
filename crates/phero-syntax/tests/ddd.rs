//! Integration tests for `.ddd` files: standalone design sources made of
//! `import` declarations followed by exactly one `application` declaration
//! of modules, application services, and class designs. The keyword policy
//! is contextual throughout: the design words lex as ordinary identifiers
//! and are special only in their grammar positions.

use phero_syntax::ast::*;
use phero_syntax::{format_ddd, parse_ddd, DddBuiltinOp, DddStereotype, MultiplicityKind, Span};

fn span_text(source: &str, span: Span) -> &str {
    &source[span.start..span.end]
}

// --- parsing -----------------------------------------------------------------

/// A full-featured model exercising every grammar production: all three
/// stereotypes, an abstract entity, all five flags, builtin and declared
/// repository operations, delegated and declared service operations, a
/// multi-name capability clause, injects, base, qualified names, every
/// multiplicity form, and contextual keywords used as names.
const FULL: &str = r#"
import "shared.mox"
import "nz.example.library.mox"

application Library {

    base nz.example.library

    module catalogue {
        /// Manages the lending of books.
        /// Also handles renewals.
        service LoanService {
            inject LoanRepository;
            inject NotificationService;
            boolean borrow(Book[] books, int days) capability BorrowBooks, RenewLoans;
            save => loanRepository.persist capability AuditWrites;
            Book[] overdue(date asOf);
            String save(Book dto) capability AuditWrites;
        }
        abstract entity AbstractThing cache scaffold nonPersistent optimisticLocking auditable
        entity Book scaffold repository BookRepository {
            delete;
            findById;
            findAll;
            save;
            String[] findByTitle(String title);
        }
        value Money
        value cache
        dto LoanSummary optimisticLocking nonPersistent auditable
    }

    module notification {
        service repository {
            inject EmailGateway;
            void notify(Book dto, date asOf) capability delete;
        }
        dto DeliveryReport nonPersistent
    }
}
"#;

fn expect_module<'a>(file: &'a DddFile, name: &str) -> &'a DddModule {
    file.application
        .as_ref()
        .expect("application")
        .modules
        .iter()
        .find(|module| module.name.text == name)
        .unwrap_or_else(|| panic!("expected module `{name}`, found {file:?}"))
}

#[test]
fn full_featured_model_parses() {
    let result = parse_ddd(FULL);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");

    // Imports keep source order; the whole-decl span covers the keyword.
    assert_eq!(
        file.imports
            .iter()
            .map(|import| import.path.as_str())
            .collect::<Vec<_>>(),
        vec!["shared.mox", "nz.example.library.mox"]
    );
    assert_eq!(
        span_text(FULL, file.imports[0].span),
        r#"import "shared.mox""#
    );

    let application = file.application.as_ref().expect("application");
    assert_eq!(application.name.text, "Library");
    assert!(span_text(FULL, application.span).starts_with("application Library"));
    assert_eq!(
        application.base.as_ref().expect("base").package.full_name(),
        "nz.example.library"
    );
    assert_eq!(
        span_text(FULL, application.base.as_ref().unwrap().span),
        "base nz.example.library"
    );
    assert_eq!(
        application
            .modules
            .iter()
            .map(|module| module.name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["catalogue", "notification"]
    );
}

#[test]
fn service_ops_capabilities_and_dependencies_parse() {
    let result = parse_ddd(FULL);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let catalogue = expect_module(&file, "catalogue");

    let service = &catalogue.services[0];
    assert_eq!(service.name.text, "LoanService");
    assert_eq!(
        service.doc.as_deref(),
        Some("Manages the lending of books.\nAlso handles renewals.")
    );
    assert_eq!(
        service
            .dependencies
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["LoanRepository", "NotificationService"]
    );

    // Declared op with parameters, a multi-name capability clause, and a
    // whole-decl span including the `;`.
    let borrow = &service.operations[0];
    assert_eq!(borrow.name.text, "borrow");
    assert_eq!(
        borrow.return_type.as_ref().unwrap().name.full_name(),
        "boolean"
    );
    assert!(borrow.multiplicity.is_none());
    assert_eq!(borrow.params.len(), 2);
    assert_eq!(borrow.params[0].type_ref.name.full_name(), "Book");
    assert!(matches!(
        borrow.params[0].multiplicity.as_ref().unwrap().kind,
        MultiplicityKind::Unbounded
    ));
    assert_eq!(borrow.params[1].type_ref.name.full_name(), "int");
    assert_eq!(borrow.params[1].name.text, "days");
    assert_eq!(
        borrow
            .capabilities
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["BorrowBooks", "RenewLoans"]
    );
    assert_eq!(
        span_text(FULL, borrow.span),
        "boolean borrow(Book[] books, int days) capability BorrowBooks, RenewLoans;"
    );

    // A delegated op named `save` (a builtin word is a plain identifier in
    // service-member position), with its delegation split at the last
    // dotted segment.
    let delegated = &service.operations[1];
    assert_eq!(delegated.name.text, "save");
    assert!(delegated.return_type.is_none());
    assert!(delegated.params.is_empty());
    let delegation = delegated.delegation.as_ref().expect("delegation");
    assert_eq!(delegation.target.full_name(), "loanRepository");
    assert_eq!(delegation.operation.text, "persist");
    assert_eq!(span_text(FULL, delegation.span), "loanRepository.persist");
    assert_eq!(
        delegated
            .capabilities
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["AuditWrites"]
    );

    // Array return type multiplicity.
    let overdue = &service.operations[2];
    assert_eq!(
        overdue.return_type.as_ref().unwrap().name.full_name(),
        "Book"
    );
    assert!(matches!(
        overdue.multiplicity.as_ref().unwrap().kind,
        MultiplicityKind::Unbounded
    ));

    // A declared op named `save` with a parameter named `dto`.
    let declared_save = &service.operations[3];
    assert_eq!(declared_save.name.text, "save");
    assert_eq!(
        declared_save.return_type.as_ref().unwrap().name.full_name(),
        "String"
    );
    assert_eq!(declared_save.params[0].name.text, "dto");
}

#[test]
fn designs_parse_with_flags_and_repositories() {
    let result = parse_ddd(FULL);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let catalogue = expect_module(&file, "catalogue");

    assert_eq!(catalogue.designs.len(), 5);

    // Abstract entity, all five flags collected from a scrambled source
    // order into the canonical field order.
    let abstract_thing = &catalogue.designs[0];
    assert_eq!(abstract_thing.class.text, "AbstractThing");
    assert_eq!(abstract_thing.stereotype, DddStereotype::Entity);
    assert!(abstract_thing.is_abstract);
    assert!(abstract_thing.flags.scaffold.is_some());
    assert!(abstract_thing.flags.auditable.is_some());
    assert!(abstract_thing.flags.optimistic_locking.is_some());
    assert!(abstract_thing.flags.non_persistent.is_some());
    assert!(abstract_thing.flags.cache.is_some());
    assert!(abstract_thing.repository.is_none());

    // Entity with a repository of builtins (recognized by their keywords)
    // and one declared operation.
    let book = &catalogue.designs[1];
    assert_eq!(book.class.text, "Book");
    assert!(!book.is_abstract);
    let repository = book.repository.as_ref().expect("repository");
    assert_eq!(repository.name.text, "BookRepository");
    assert_eq!(repository.operations.len(), 5);
    assert_eq!(repository.operations[0].name.text, "delete");
    assert_eq!(repository.operations[0].builtin, Some(DddBuiltinOp::Delete));
    assert!(repository.operations[0].return_type.is_none());
    assert_eq!(
        repository.operations[1].builtin,
        Some(DddBuiltinOp::FindById)
    );
    assert_eq!(
        repository.operations[2].builtin,
        Some(DddBuiltinOp::FindAll)
    );
    assert_eq!(repository.operations[3].builtin, Some(DddBuiltinOp::Save));
    assert_eq!(span_text(FULL, repository.operations[1].span), "findById;");
    let declared = &repository.operations[4];
    assert!(declared.builtin.is_none());
    assert_eq!(declared.name.text, "findByTitle");
    assert_eq!(
        declared.return_type.as_ref().unwrap().name.full_name(),
        "String"
    );
    assert!(matches!(
        declared.multiplicity.as_ref().unwrap().kind,
        MultiplicityKind::Unbounded
    ));
    assert_eq!(declared.params[0].type_ref.name.full_name(), "String");

    // A plain value and a design named `cache` (flag words are ordinary
    // identifiers in name position).
    assert_eq!(catalogue.designs[2].class.text, "Money");
    assert_eq!(catalogue.designs[2].stereotype, DddStereotype::Value);
    assert!(catalogue.designs[2].flags.is_empty());
    assert_eq!(catalogue.designs[3].class.text, "cache");

    // Dto with a flag subset.
    let summary = &catalogue.designs[4];
    assert_eq!(summary.class.text, "LoanSummary");
    assert_eq!(summary.stereotype, DddStereotype::Dto);
    assert!(!summary.is_abstract);
    assert!(summary.flags.scaffold.is_none());
    assert!(summary.flags.auditable.is_some());
    assert!(summary.flags.optimistic_locking.is_some());
    assert!(summary.flags.non_persistent.is_some());
    assert!(summary.flags.cache.is_none());
}

#[test]
fn negated_flags_parse_with_the_bang_in_their_span() {
    let source = concat!(
        "application A { module m {\n",
        "    entity Book !auditable cache\n",
        "    entity Movie !optimisticLocking\n",
        "} }\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let designs = &result.ast.unwrap().application.unwrap().modules[0].designs;
    let book = &designs[0];
    let auditable = book.flags.auditable.expect("!auditable parsed");
    assert!(auditable.negated);
    assert_eq!(span_text(source, auditable.span), "!auditable");
    assert!(!book.flags.cache.expect("cache parsed").negated);

    let movie = &designs[1];
    let locking = movie
        .flags
        .optimistic_locking
        .expect("!optimisticLocking parsed");
    assert!(locking.negated);
    assert_eq!(span_text(source, locking.span), "!optimisticLocking");
}

#[test]
fn negated_flags_survive_formatting_in_canonical_position() {
    // Source order is scrambled on purpose: the negated flag re-emits with
    // its `!` attached, in the canonical flag order.
    let source = "application A { module m { entity Book cache !auditable } }\n";
    let formatted = format_ddd(source).unwrap();
    assert!(
        formatted.contains("entity Book !auditable cache"),
        "canonical order with the negation kept: {formatted:?}"
    );
    // Idempotent: formatting the formatted output is a fixpoint.
    assert_eq!(format_ddd(&formatted).unwrap(), formatted);
}

#[test]
fn multiplicity_forms_parse() {
    let source = concat!(
        "application A { module M { service S {\n",
        "    Book[] a();\n",
        "    Book[3] b();\n",
        "    Book[1..*] c();\n",
        "    Book[2..5] d(Book[0..1] e, Book[] f);\n",
        "} } }\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let ops = &result.ast.unwrap().application.unwrap().modules[0].services[0].operations;
    assert!(matches!(
        ops[0].multiplicity.as_ref().unwrap().kind,
        MultiplicityKind::Unbounded
    ));
    assert!(matches!(
        ops[1].multiplicity.as_ref().unwrap().kind,
        MultiplicityKind::Exact(3)
    ));
    assert!(matches!(
        ops[2].multiplicity.as_ref().unwrap().kind,
        MultiplicityKind::Range(1, MultBound::Star)
    ));
    assert!(matches!(
        ops[3].multiplicity.as_ref().unwrap().kind,
        MultiplicityKind::Range(2, MultBound::Int(5))
    ));
    assert!(matches!(
        ops[3].params[0].multiplicity.as_ref().unwrap().kind,
        MultiplicityKind::Range(0, MultBound::Int(1))
    ));
    assert!(matches!(
        ops[3].params[1].multiplicity.as_ref().unwrap().kind,
        MultiplicityKind::Unbounded
    ));
}

#[test]
fn doc_runs_attach_to_services_and_searches() {
    let source = concat!(
        "application A {\n",
        "    module M {\n",
        "        /// First line.\n",
        "        /// Second line.\n",
        "        service Documented { }\n",
        "        // not a doc comment\n",
        "        service Plain { }\n",
        "        /// Separated by a blank line.\n",
        "\n",
        "        service Detached { }\n",
        "        /// Search doc.\n",
        "        search Lookup { }\n",
        "        entity Thing\n",
        "    }\n",
        "}\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let module = &result.ast.unwrap().application.unwrap().modules[0];
    let services = &module.services;
    // Contiguous `///` lines join with newlines — the `.mox` semantic.
    assert_eq!(
        services[0].doc.as_deref(),
        Some("First line.\nSecond line.")
    );
    assert_eq!(services[1].doc, None, "a non-doc comment breaks the run");
    assert_eq!(services[2].doc, None, "a blank line breaks the run");
    assert_eq!(module.searches[0].doc.as_deref(), Some("Search doc."));
}

#[test]
fn contextual_keywords_remain_usable_everywhere() {
    let source = concat!(
        "application application {\n",
        "    module application {\n",
        "        service repository {\n",
        "            inject inject;\n",
        "            service save(application dto) capability cache;\n",
        "            value => entity.inject;\n",
        "        }\n",
        "        entity entity\n",
        "    }\n",
        "}\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let application = file.application.as_ref().unwrap();
    assert_eq!(application.name.text, "application");
    let module = &application.modules[0];
    assert_eq!(module.name.text, "application");
    let service = &module.services[0];
    assert_eq!(service.name.text, "repository");
    assert_eq!(service.dependencies[0].text, "inject");
    // A declared op named `save`, with an `application`-typed parameter
    // named `dto` and a `cache`-named capability: all contextual words are
    // plain identifiers in name positions.
    let declared = &service.operations[0];
    assert_eq!(declared.name.text, "save");
    assert_eq!(
        declared.return_type.as_ref().unwrap().name.full_name(),
        "service"
    );
    assert_eq!(declared.params[0].type_ref.name.full_name(), "application");
    assert_eq!(declared.params[0].name.text, "dto");
    assert_eq!(declared.capabilities[0].text, "cache");
    // A delegated op named `value` targeting `entity.inject`.
    let delegated = &service.operations[1];
    assert_eq!(delegated.name.text, "value");
    let delegation = delegated.delegation.as_ref().unwrap();
    assert_eq!(delegation.target.full_name(), "entity");
    assert_eq!(delegation.operation.text, "inject");
    assert_eq!(module.designs[0].class.text, "entity");
}

#[test]
fn import_semicolon_is_optional_and_unescaped() {
    let result = parse_ddd(r#"import "a.mox"; import "b.mox" application A { }"#);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    assert_eq!(file.imports[0].path, "a.mox");
    assert_eq!(file.imports[1].path, "b.mox");
}

// --- parse failures ------------------------------------------------------------

#[test]
fn missing_application_is_an_error() {
    for source in ["", "   \n\t\n  ", "import \"a.mox\""] {
        let result = parse_ddd(source);
        assert!(
            result.errors.iter().any(|error| error
                .message
                .contains("expected an `application` declaration")),
            "{source:?}: expected the missing-application error, got {:?}",
            result.errors
        );
    }
}

#[test]
fn missing_service_body_brace_is_an_error() {
    let result = parse_ddd("application A { module M { service S { boolean borrow(); ");
    assert!(!result.errors.is_empty(), "expected a clean error");
    let _ = result.ast.expect("recovered AST must still exist");
}

#[test]
fn delegated_op_without_target_operation_is_an_error() {
    let result = parse_ddd("application A { module M { service S { renew =>; } } }");
    assert!(!result.errors.is_empty(), "expected a clean error");
    let result = parse_ddd("application A { module M { service S { renew => persist; } } }");
    assert!(
        result
            .errors
            .iter()
            .any(|error| error.message.contains("expected `dependency.operation`")),
        "expected the delegation-split error, got {:?}",
        result.errors
    );
}

#[test]
fn capability_clause_without_names_is_an_error() {
    let result =
        parse_ddd("application A { module M { service S { boolean borrow() capability; } } }");
    assert!(
        result.errors.iter().any(|error| error
            .message
            .contains("`capability` requires at least one capability name")),
        "expected the capability error, got {:?}",
        result.errors
    );
}

#[test]
fn flags_after_repository_are_an_error() {
    let source =
        "application A { module M { entity Book repository BookRepo { findById; } scaffold } }";
    let result = parse_ddd(source);
    assert!(!result.errors.is_empty(), "expected a clean error");
}

#[test]
fn two_applications_are_an_error() {
    let source = "application A { } application B { }";
    let result = parse_ddd(source);
    assert!(
        result.errors.iter().any(|error| error
            .message
            .contains("duplicate `application` declaration")),
        "expected the duplicate-application error, got {:?}",
        result.errors
    );
    let file = result
        .ast
        .expect("recovered AST keeps the first application");
    assert_eq!(file.application.unwrap().name.text, "A");
}

#[test]
fn base_after_a_module_is_an_error() {
    let source = "application A { module M { } base nz.example.library }";
    let result = parse_ddd(source);
    assert!(
        result
            .errors
            .iter()
            .any(|error| error.message.contains("`base` must be the first member")),
        "expected the base-ordering error, got {:?}",
        result.errors
    );
}

#[test]
fn duplicate_base_is_an_error() {
    let source = "application A { base one base two }";
    let result = parse_ddd(source);
    assert!(
        result
            .errors
            .iter()
            .any(|error| error.message.contains("duplicate `base` declaration")),
        "expected the duplicate-base error, got {:?}",
        result.errors
    );
}

#[test]
fn import_after_application_is_an_error() {
    let source = "application A { } import \"late.mox\";";
    let result = parse_ddd(source);
    assert!(
        result.errors.iter().any(|error| error
            .message
            .contains("`import` after the `application`/`module` declarations")),
        "expected the late-import error, got {:?}",
        result.errors
    );
    let file = result.ast.expect("recovered AST");
    assert!(file.imports.is_empty(), "the late import is dropped");
}

#[test]
fn junk_regions_recover_to_the_next_import_or_application() {
    let source = "bogus garbage 42 import \"x.mox\" application A { module M { entity Book } }";
    let result = parse_ddd(source);
    assert!(!result.errors.is_empty(), "expected a junk-region error");
    let file = result.ast.expect("expected a recovered AST");
    assert_eq!(file.imports.len(), 1);
    assert_eq!(
        file.application.unwrap().modules[0].designs[0].class.text,
        "Book"
    );

    // A `.mox`-style declaration is junk in a `.ddd` file.
    let result = parse_ddd("class Bogus { int pages } application A { }");
    assert!(!result.errors.is_empty(), "expected a junk-region error");
    assert!(result
        .ast
        .expect("recovered")
        .application
        .unwrap()
        .modules
        .is_empty());

    // An unterminated application consumes the rest as junk; the recovered
    // file has no application at all.
    let result = parse_ddd("application A { module M { service");
    assert!(!result.errors.is_empty(), "expected a junk-region error");
    let file = result.ast.expect("recovered AST");
    assert!(file.imports.is_empty());
    assert!(file.application.is_none());
}

#[test]
fn adversarial_inputs_do_not_panic() {
    let nasty = [
        "",
        "   \n\t",
        "import",
        "import \"",
        "import \"unterminated",
        "import import import",
        "application",
        "application A",
        "application A {",
        "application A { base",
        "application A { base base two }",
        "application A { module",
        "application A { module M {",
        "application A { module M { service",
        "application A { module M { service S {",
        "application A { module M { service S { inject",
        "application A { module M { service S { => x.y; } } }",
        "application A { module M { service S { save => ; } } }",
        "application A { module M { service S { a.b.c => d.e.f; } } }",
        "application A { module M { service S { => x.y; } } }",
        "application A { module M { entity } }",
        "application A { module M { entity abstract } }",
        "application A { module M { abstract abstract entity T } }",
        "application A { module M { entity T repository R { findById findById } } }",
        "application A { module M { entity T repository R { String f(String } } }",
        "application A { module M { entity T scaffold scaffold scaffold } }",
        "application A { module M { service S { a => b } } }",
        "application A { application B { } }",
        "@@@ ### :",
        "^application A { module ^module { entity ^entity } }",
        "application A { module M { service S { boolean borrow(String[] a, Book[3] b); } }",
    ];
    for input in nasty {
        let result = parse_ddd(input);
        let _ = format!("{result:?}"); // must be debuggable and must not panic
        let _ = format_ddd(input); // must not panic either
    }
}

// --- formatting ----------------------------------------------------------------

fn fmt_ddd(source: &str) -> String {
    format_ddd(source).expect("format succeeds")
}

#[test]
fn full_model_formats_canonically_and_is_idempotent() {
    let canonical = concat!(
        "import \"shared.mox\";\n",
        "import \"nz.example.library.mox\";\n",
        "\n",
        "application Library {\n",
        "    base nz.example.library\n",
        "    module catalogue {\n",
        "        /// Manages the lending of books.\n",
        "        /// Also handles renewals.\n",
        "        service LoanService {\n",
        "            inject LoanRepository;\n",
        "            inject NotificationService;\n",
        "            boolean borrow(Book[] books, int days) capability BorrowBooks, RenewLoans;\n",
        "            save => loanRepository.persist capability AuditWrites;\n",
        "            Book[] overdue(date asOf);\n",
        "            String save(Book dto) capability AuditWrites;\n",
        "        }\n",
        "        abstract entity AbstractThing scaffold auditable optimisticLocking nonPersistent cache\n",
        "        entity Book scaffold repository BookRepository {\n",
        "            delete;\n",
        "            findById;\n",
        "            findAll;\n",
        "            save;\n",
        "            String[] findByTitle(String title);\n",
        "        }\n",
        "        value Money\n",
        "        value cache\n",
        "        dto LoanSummary auditable optimisticLocking nonPersistent\n",
        "    }\n",
        "    module notification {\n",
        "        service repository {\n",
        "            inject EmailGateway;\n",
        "            void notify(Book dto, date asOf) capability delete;\n",
        "        }\n",
        "        dto DeliveryReport nonPersistent\n",
        "    }\n",
        "}\n",
    );
    assert_eq!(fmt_ddd(FULL), canonical);
    // Idempotence: the canonical output is a fixpoint.
    assert_eq!(fmt_ddd(canonical), canonical);
}

#[test]
fn empty_bodies_stay_inline() {
    assert_eq!(fmt_ddd("application A { }"), "application A {}\n");
    assert_eq!(
        fmt_ddd("application A { module M { service S { } } }"),
        "application A {\n    module M {\n        service S {}\n    }\n}\n"
    );
}

#[test]
fn design_flags_canonicalize_to_a_fixed_order() {
    assert_eq!(
        fmt_ddd("application A { module M { entity T cache nonPersistent optimisticLocking auditable scaffold } }"),
        concat!(
            "application A {\n",
            "    module M {\n",
            "        entity T scaffold auditable optimisticLocking nonPersistent cache\n",
            "    }\n",
            "}\n",
        )
    );
    // Repeats collapse.
    assert_eq!(
        fmt_ddd("application A { module M { entity T cache cache } }"),
        "application A {\n    module M {\n        entity T cache\n    }\n}\n"
    );
}

#[test]
fn comments_are_preserved() {
    let source = concat!(
        "// header\n",
        "import \"a.mox\" // trailing\n",
        "/* block */\n",
        "application A { // inner\n",
        "    module M {\n",
        "        /// doc line\n",
        "        service S { // member\n",
        "            inject R; // dependency\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    let expected = concat!(
        "// header\n",
        "import \"a.mox\"; // trailing\n",
        "\n",
        "/* block */\n",
        "application A { // inner\n",
        "    module M {\n",
        "        /// doc line\n",
        "        service S { // member\n",
        "            inject R; // dependency\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    assert_eq!(fmt_ddd(source), expected);
    assert_eq!(fmt_ddd(expected), expected);
}

#[test]
fn imports_are_hoisted_ahead_of_the_application() {
    let source = concat!(
        "application A { module M { } }\n",
        "\n",
        "\n",
        "import \"b.mox\"\n",
        "import \"a.mox\";\n",
    );
    let expected = concat!(
        "import \"b.mox\";\n",
        "import \"a.mox\";\n",
        "\n",
        "application A {\n",
        "    module M {}\n",
        "}\n",
    );
    assert_eq!(fmt_ddd(source), expected);
    assert_eq!(fmt_ddd(expected), expected);
}

#[test]
fn interior_blank_lines_are_dropped() {
    assert_eq!(
        fmt_ddd("application A {\n    module M { }\n\n\n    module N { }\n}"),
        concat!(
            "application A {\n",
            "    module M {}\n",
            "    module N {}\n",
            "}\n",
        )
    );
}

#[test]
fn parse_errors_still_format() {
    assert_eq!(fmt_ddd("application"), "application\n");
    assert_eq!(
        fmt_ddd("@@@ application A { }"),
        "@ @ @\n\napplication A {}\n"
    );
}

#[test]
fn formatted_output_round_trips_structurally() {
    let formatted = fmt_ddd(FULL);
    let once = parse_ddd(&formatted);
    assert!(
        once.errors.is_empty(),
        "formatted output must parse cleanly, got {:?}",
        once.errors
    );
    let twice_formatted = fmt_ddd(&formatted);
    let twice = parse_ddd(&twice_formatted);
    assert!(
        twice.errors.is_empty(),
        "re-formatted output must parse cleanly, got {:?}",
        twice.errors
    );
    assert_eq!(once.ast, twice.ast);
}

// --- search projections ------------------------------------------------------

/// A full-featured search projection: every member kind, out of canonical
/// order (the formatter reorders), a concat document entry, a brace-bearing
/// conditional document entry, a `;` inside a string literal, per-field
/// boost/analyzer, the camelCase `tfIdf` keyword, and a multi-name
/// capability clause — the same clause grammar a service operation takes.
const SEARCH: &str = r#"
import "library.mox"

application Library {
    module media {
        // a leading line comment
        search MediaSearch {
            capability SearchMedia, SearchMediaAdmin
            pagination {
                cursor
                max 100
                limit 20
            }
            analyzer "english"
            ranking bm25
            document {
                headline = title;
                blurb = title + " - " + synopsis;
                teaser = if (runtimeMinutes > 100) { title + " (long)" } else { "a; b" };
            }
            sort {
                title
            }
            filters {
                genre
            }
            text {
                title boost 3
                synopsis analyzer "english"
            }
            entity Movie
        }
    }
}
"#;

fn expect_search<'a>(file: &'a DddFile, name: &str) -> &'a DddSearch {
    expect_module(file, "media")
        .searches
        .iter()
        .find(|search| search.name.text == name)
        .unwrap_or_else(|| panic!("expected search `{name}`"))
}

#[test]
fn search_decl_parses_with_all_members() {
    let result = parse_ddd(SEARCH);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let search = expect_search(&file, "MediaSearch");

    assert_eq!(search.entity.as_ref().expect("entity").full_name(), "Movie");
    assert_eq!(search.text.len(), 2);
    assert_eq!(search.text[0].property.full_name(), "title");
    assert_eq!(search.text[0].boost, Some(3));
    assert_eq!(search.text[0].analyzer, None);
    assert_eq!(search.text[1].property.full_name(), "synopsis");
    assert_eq!(search.text[1].boost, None);
    assert_eq!(search.text[1].analyzer.as_deref(), Some("english"));
    assert_eq!(
        search
            .filters
            .iter()
            .map(|name| name.full_name())
            .collect::<Vec<_>>(),
        vec!["genre"]
    );
    assert_eq!(
        search
            .sort
            .iter()
            .map(|name| name.full_name())
            .collect::<Vec<_>>(),
        vec!["title"]
    );
    assert_eq!(search.document.len(), 3);
    let DddProjectionMember::Assign { name, expr, .. } = &search.document[0] else {
        panic!(
            "expected an assign document entry, got {:?}",
            search.document[0]
        );
    };
    assert_eq!(name.text, "headline");
    assert_eq!(span_text(SEARCH, *expr), "title");
    let DddProjectionMember::Assign { name, expr, .. } = &search.document[1] else {
        panic!(
            "expected an assign document entry, got {:?}",
            search.document[1]
        );
    };
    assert_eq!(name.text, "blurb");
    assert_eq!(span_text(SEARCH, *expr), r#"title + " - " + synopsis"#);
    let DddProjectionMember::Assign { name, expr, .. } = &search.document[2] else {
        panic!(
            "expected an assign document entry, got {:?}",
            search.document[2]
        );
    };
    assert_eq!(name.text, "teaser");
    assert_eq!(
        span_text(SEARCH, *expr),
        r#"if (runtimeMinutes > 100) { title + " (long)" } else { "a; b" }"#
    );
    assert_eq!(search.ranking, Some(DddRanking::Bm25));
    assert_eq!(search.analyzer.as_deref(), Some("english"));
    let pagination = search.pagination.as_ref().expect("pagination");
    assert_eq!(pagination.limit, Some(20));
    assert_eq!(pagination.max, Some(100));
    assert!(pagination.cursor);
    assert_eq!(
        search
            .capabilities
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["SearchMedia", "SearchMediaAdmin"]
    );
    assert!(search.doc.is_none(), "`//` comments are not doc runs");
}

#[test]
fn search_named_document_reference_parses() {
    // DF3 (issue pherolang#326): `document DtoName;` is the reusable form;
    // the inline block remains the one-off form.
    let source = "application A { module m { search S { entity Movie document BookSummary; } } }";
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let search = &file.application.as_ref().expect("application").modules[0].searches[0];
    assert_eq!(
        search
            .document_ref
            .as_ref()
            .expect("the named reference parses")
            .full_name(),
        "BookSummary"
    );
    assert!(
        search.document.is_empty(),
        "the named form carries no inline entries"
    );
}

#[test]
fn search_capability_clause_accepts_a_list_and_clauses_merge() {
    // The unified clause grammar: a search's `capability` clause takes the
    // same `capability <name> (, <name>)*` list a service operation takes,
    // and repeated clauses merge into one list (the `text`/`filters`
    // precedent) instead of shadowing each other.
    let source = concat!(
        "application A {\n",
        "    module m {\n",
        "        search S {\n",
        "            entity Book\n",
        "            capability ReadBooks, LendBooks\n",
        "            capability RenewLoans\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "a capability list in a search must parse: {:?}",
        result.errors
    );
    let search = &result
        .ast
        .expect("expected an AST")
        .application
        .unwrap()
        .modules[0]
        .searches[0];
    assert_eq!(
        search
            .capabilities
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["ReadBooks", "LendBooks", "RenewLoans"]
    );
}

#[test]
fn search_repeated_clauses_merge_and_singles_are_idempotent() {
    let source = concat!(
        "application A {\n",
        "    module m {\n",
        "        search S {\n",
        "            entity Book\n",
        "            text { title }\n",
        "            filters { genre }\n",
        "            entity Movie\n", // idempotent: the first wins
        "            ranking bm25\n",
        "            ranking exact\n", // idempotent: the first wins
        "            text { synopsis boost 2 }\n",
        "            sort { title }\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let search = &result
        .ast
        .expect("expected an AST")
        .application
        .unwrap()
        .modules[0]
        .searches[0];
    assert_eq!(search.entity.as_ref().expect("entity").full_name(), "Book");
    assert_eq!(search.ranking, Some(DddRanking::Bm25));
    assert_eq!(
        search
            .text
            .iter()
            .map(|field| field.property.full_name())
            .collect::<Vec<_>>(),
        vec!["title", "synopsis"]
    );
    assert_eq!(search.text[1].boost, Some(2));
    assert_eq!(search.filters.len(), 1);
    assert_eq!(search.sort.len(), 1);
}

#[test]
fn search_parse_failures_are_clean_errors() {
    for (source, label) in [
        (
            "application A { module m { search { entity B } } }",
            "missing name",
        ),
        (
            "application A { module m { search S { document { h = title } } } }",
            "unterminated document entry",
        ),
        (
            "application A { module m { search S { document { h = ; } } } }",
            "empty document entry",
        ),
        (
            "application A { module m { search S { ranking } } }",
            "ranking without a strategy",
        ),
        (
            r#"application A { module m { search S { ranking custom } } }"#,
            "custom without a string",
        ),
    ] {
        let result = parse_ddd(source);
        assert!(!result.errors.is_empty(), "{label} must be an error");
    }
}

#[test]
fn search_formats_canonically_and_is_idempotent() {
    let formatted = fmt_ddd(SEARCH);
    assert_eq!(
        formatted,
        concat!(
            "import \"library.mox\";\n",
            "\n",
            "application Library {\n",
            "    module media {\n",
            "        // a leading line comment\n",
            "        search MediaSearch {\n",
            "            entity Movie\n",
            "            text {\n",
            "                title boost 3\n",
            "                synopsis analyzer \"english\"\n",
            "            }\n",
            "            filters {\n",
            "                genre\n",
            "            }\n",
            "            sort {\n",
            "                title\n",
            "            }\n",
            "            document {\n",
            "                headline = title;\n",
            "                blurb = title + \" - \" + synopsis;\n",
            "                teaser = if(runtimeMinutes > 100) { title + \" (long)\" } else { \"a; b\" };\n",
            "            }\n",
            "            ranking bm25\n",
            "            analyzer \"english\"\n",
            "            pagination {\n",
            "                cursor\n",
            "                max 100\n",
            "                limit 20\n",
            "            }\n",
            "            capability SearchMedia, SearchMediaAdmin\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
        "members must reorder into the canonical order, got:\n{formatted}"
    );
    assert_eq!(
        fmt_ddd(&formatted),
        formatted,
        "formatting must be idempotent"
    );
}

#[test]
fn a_named_search_document_formats_on_one_line_and_is_idempotent() {
    let source = "application A { module m { search S { entity Movie document BookSummary; } } }";
    let formatted = fmt_ddd(source);
    assert_eq!(
        formatted,
        concat!(
            "application A {\n",
            "    module m {\n",
            "        search S {\n",
            "            entity Movie\n",
            "            document BookSummary;\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
        "the named document stays one canonical line, got:\n{formatted}"
    );
    assert_eq!(fmt_ddd(&formatted), formatted, "formatting is idempotent");
}

#[test]
fn search_output_round_trips_structurally() {
    let formatted = fmt_ddd(SEARCH);
    let once = parse_ddd(&formatted);
    assert!(
        once.errors.is_empty(),
        "formatted output must parse cleanly, got {:?}",
        once.errors
    );
    let twice = parse_ddd(&fmt_ddd(&formatted));
    assert!(
        twice.errors.is_empty(),
        "re-formatted output must parse cleanly, got {:?}",
        twice.errors
    );
    assert_eq!(once.ast, twice.ast);
}

#[test]
fn adversarial_search_inputs_do_not_panic() {
    let sources = [
        "search",
        "search S",
        "search S {",
        "search S { entity",
        "search S { text { } filters { } document { } }",
        "search S { document { = title; } }",
        "search S { document { h = title } document { h2 = synopsis; } }",
        "search S { document { h = \"unterminated } } }",
        "search S { document { h = (((title) ; } }",
        "search S { pagination { limit } }",
        "search S { pagination { max x } }",
        "search S { text { title boost -3 } }",
        "application A { module m { search S { entity E } search T { entity F } } }",
    ];
    for source in sources {
        let _ = parse_ddd(source);
        // Formatting may fail on lex errors (e.g. unterminated strings);
        // the contract is only that neither path panics.
        let _ = format_ddd(source);
    }
}

// --- state machines -------------------------------------------------------------

/// A machine-bearing design: repository + machine on one design, a machine on
/// a body-less design, triggered and bare transitions, an edge guard, and
/// service/repository ops guarded by machine states.
const MACHINE: &str = r#"
import "payrun.mox"

application Payroll {
    base nz.example.payroll
    module payroll {
        service PayRunService {
            boolean approve() capability ApprovePayRun when (status == AwaitingApproval);
        }
        entity PayRun repository PayRunRepository {
            findById;
            save;
            PhysicalMedia[] findByState(String state) when (status == Posted);
        }
        machine status: PayRunStatus {
            initial Draft;
            Draft -> Calculating via Calculate;
            AwaitingApproval -> Approved via Approve when (total > 0);
            Draft -> Reversed;
            terminal Reversed;
            terminal Posted;
        }
        entity AuditLog
        machine state: AuditState {
            initial Open;
            Open -> Closed via Close;
            terminal Closed;
        }
    }
}
"#;

#[test]
fn machine_blocks_parse() {
    let result = parse_ddd(MACHINE);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let module = expect_module(&file, "payroll");

    let payrun = module
        .designs
        .iter()
        .find(|design| design.class.text == "PayRun")
        .expect("the PayRun design");
    let machine = payrun.machine.as_ref().expect("the PayRun machine");
    assert_eq!(machine.field.text, "status");
    assert_eq!(machine.type_.name.full_name(), "PayRunStatus");
    assert_eq!(machine.initial.as_ref().expect("initial").text, "Draft");
    assert_eq!(
        machine
            .terminals
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Reversed", "Posted"]
    );
    assert_eq!(machine.transitions.len(), 3);
    let approve = &machine.transitions[1];
    assert_eq!(approve.from.text, "AwaitingApproval");
    assert_eq!(approve.to.text, "Approved");
    assert_eq!(approve.trigger.as_ref().expect("trigger").text, "Approve");
    assert_eq!(
        span_text(MACHINE, approve.guard.expect("guard")),
        "(total > 0)"
    );
    let bare = &machine.transitions[2];
    assert_eq!(bare.from.text, "Draft");
    assert_eq!(bare.to.text, "Reversed");
    assert!(bare.trigger.is_none());
    assert!(bare.guard.is_none());

    // A machine on a body-less design parses the same way.
    let audit = module
        .designs
        .iter()
        .find(|design| design.class.text == "AuditLog")
        .expect("the AuditLog design");
    assert!(audit.repository.is_none());
    let audit_machine = audit.machine.as_ref().expect("the AuditLog machine");
    assert_eq!(audit_machine.field.text, "state");
    assert_eq!(audit_machine.transitions.len(), 1);
}

#[test]
fn service_op_when_clause_parses() {
    let result = parse_ddd(MACHINE);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let module = expect_module(&file, "payroll");
    let service = module
        .services
        .iter()
        .find(|service| service.name.text == "PayRunService")
        .expect("the PayRunService");
    let approve = service
        .operations
        .iter()
        .find(|op| op.name.text == "approve")
        .expect("the approve operation");
    assert_eq!(
        span_text(MACHINE, approve.condition.expect("condition")),
        "(status == AwaitingApproval)"
    );

    // A declared repository operation takes a `when` clause but no
    // `capability` clause (the documented slice-1).
    let payrun = module
        .designs
        .iter()
        .find(|design| design.class.text == "PayRun")
        .expect("the PayRun design");
    let repository = payrun.repository.as_ref().expect("repository");
    let by_state = repository
        .operations
        .iter()
        .find(|op| op.name.text == "findByState")
        .expect("the findByState operation");
    assert_eq!(
        span_text(MACHINE, by_state.condition.expect("condition")),
        "(status == Posted)"
    );
}

/// A service operation carrying the `emits` clause: a comma list of event
/// names, written between the `capability` clause and the `when` guard
/// (the canonical clause order).
const EMITS: &str = r#"
import "payrun.mox"

application Payroll {
    base nz.example.payroll
    module payroll {
        service PayRunService {
            boolean approve(String approver) capability ApprovePayRun emits PayRunStateChanged, PayRunSubmitted when (status == AwaitingApproval);
            save => payRunRepository.save emits PayRunStateChanged;
            void purge() emits PayRunPurged;
            void replicate();
        }
    }
}
"#;

#[test]
fn service_op_emits_clause_parses() {
    let result = parse_ddd(EMITS);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let service = &expect_module(&file, "payroll").services[0];
    assert_eq!(service.operations.len(), 4);

    // A declared op: emits names collected in source order, the whole-decl
    // span covering the clause and the `;`.
    let approve = &service.operations[0];
    assert_eq!(approve.name.text, "approve");
    assert_eq!(
        approve
            .capabilities
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["ApprovePayRun"]
    );
    assert_eq!(
        approve
            .emits
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["PayRunStateChanged", "PayRunSubmitted"]
    );
    assert_eq!(
        span_text(EMITS, approve.condition.expect("condition")),
        "(status == AwaitingApproval)"
    );
    assert_eq!(
        span_text(EMITS, approve.span),
        "boolean approve(String approver) capability ApprovePayRun emits PayRunStateChanged, \
         PayRunSubmitted when (status == AwaitingApproval);"
    );

    // A delegated op carries the clause too (it is a service operation).
    let delegated = &service.operations[1];
    assert!(delegated.delegation.is_some());
    assert_eq!(
        delegated
            .emits
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["PayRunStateChanged"]
    );

    // An op with no clause emits nothing (the AST default).
    let replicate = &service.operations[3];
    assert_eq!(replicate.name.text, "replicate");
    assert!(replicate.emits.is_empty());
}

#[test]
fn emits_clause_requires_at_least_one_event_name() {
    let result = parse_ddd("application A { module M { service S { boolean approve() emits; } } }");
    assert!(
        result.errors.iter().any(|error| error
            .message
            .contains("`emits` requires at least one event name")),
        "expected the emits error, got {:?}",
        result.errors
    );
}

#[test]
fn emits_formats_in_canonical_clause_order() {
    // The source is already canonical (capability → emits → when); the
    // formatter re-emits it byte-identically and is idempotent.
    let formatted = fmt_ddd(EMITS);
    assert_eq!(
        formatted,
        concat!(
            "import \"payrun.mox\";\n",
            "\n",
            "application Payroll {\n",
            "    base nz.example.payroll\n",
            "    module payroll {\n",
            "        service PayRunService {\n",
            "            boolean approve(String approver) capability ApprovePayRun emits PayRunStateChanged, PayRunSubmitted when (status == AwaitingApproval);\n",
            "            save => payRunRepository.save emits PayRunStateChanged;\n",
            "            void purge() emits PayRunPurged;\n",
            "            void replicate();\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
        "canonical clause order capability → emits → when, got:\n{formatted}"
    );
    assert_eq!(
        fmt_ddd(&formatted),
        formatted,
        "formatting must be idempotent"
    );

    // The formatted output parses cleanly.
    let once = parse_ddd(&formatted);
    assert!(
        once.errors.is_empty(),
        "formatted output must parse cleanly, got {:?}",
        once.errors
    );
}

/// A delegated service operation carrying the `transition` clause (issue
/// pherolang#176): the explicit transition binding, unqualified or qualified
/// by the machine's enum type name.
const TRANSITION: &str = r#"
import "payrun.mox"

application Payroll {
    base nz.example.payroll
    module payroll {
        service PayRunService {
            inject PayRunRepository;
            submit => PayRunRepository.save transition Submit when (status == Draft);
            reverse => PayRunRepository.save transition PayRunStatus.Reverse;
            protected post => PayRunRepository.save capability PostPayRun emits PayRunPosted transition Post;
        }
    }
}
"#;

#[test]
fn transition_clause_parses() {
    let result = parse_ddd(TRANSITION);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let service = &expect_module(&file, "payroll").services[0];
    assert_eq!(service.operations.len(), 3);

    // Unqualified: the trigger only, `machine` none; the guard rides after
    // the binding in the canonical clause order.
    let submit = &service.operations[0];
    let transition = submit.transition.as_ref().expect("the transition binding");
    assert_eq!(transition.trigger.text, "Submit");
    assert!(transition.machine.is_none());
    assert_eq!(span_text(TRANSITION, transition.span), "transition Submit");
    assert_eq!(
        span_text(TRANSITION, submit.condition.expect("condition")),
        "(status == Draft)"
    );

    // Qualified: the machine qualifier is the enum type name, split at the
    // dot.
    let reverse = &service.operations[1];
    let qualified = reverse.transition.as_ref().expect("the transition binding");
    assert_eq!(qualified.trigger.text, "Reverse");
    assert_eq!(
        qualified.machine.as_ref().expect("the qualifier").text,
        "PayRunStatus"
    );
    assert_eq!(
        span_text(TRANSITION, qualified.span),
        "transition PayRunStatus.Reverse"
    );

    // The binding composes with the other clauses, in the canonical order:
    // capability, emits, transition.
    let post = &service.operations[2];
    assert!(post.is_protected);
    assert_eq!(post.capabilities[0].text, "PostPayRun");
    assert_eq!(post.emits[0].text, "PayRunPosted");
    let bound = post.transition.as_ref().expect("the transition binding");
    assert_eq!(bound.trigger.text, "Post");
    assert!(bound.machine.is_none());
    assert!(post.condition.is_none());

    // An op without the binding carries none (the AST default).
    let plain = r#"
application A { module m { service S {
    void go();
    finish => T.save;
} } }
"#;
    let result = parse_ddd(plain);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let ops = &expect_module(&file, "m").services[0].operations;
    assert!(ops[0].transition.is_none());
    assert!(ops[1].transition.is_none());
}

#[test]
fn transition_clause_requires_a_trigger_name() {
    let result =
        parse_ddd("application A { module m { service S { finish => T.save transition; } } }");
    assert!(
        result.errors.iter().any(|error| error
            .message
            .contains("`transition` requires a trigger name")),
        "expected the transition error, got {:?}",
        result.errors
    );
}

#[test]
fn transition_clause_order_is_pinned() {
    // The clause order is fixed: `capability? invokes? emits? transition?
    // when?`. A violation is a syntax error, not a formatting opportunity.
    for source in [
        // `transition` before `emits`.
        "application A { module m { service S { finish => T.save transition Submit emits PayRunPosted; } } }",
        // `transition` after the guard.
        "application A { module m { service S { finish => T.save when (total > 0) transition Submit; } } }",
        // A third dotted segment is not part of the spelling.
        "application A { module m { service S { finish => T.save transition pkg.PayRunStatus.Reverse; } } }",
    ] {
        let result = parse_ddd(source);
        assert!(
            !result.errors.is_empty(),
            "expected a syntax error for {source}"
        );
    }
}

#[test]
fn transition_formats_in_canonical_clause_order() {
    // The source is already canonical (emits → transition → when); the
    // formatter re-emits it byte-identically and is idempotent.
    let formatted = fmt_ddd(TRANSITION);
    assert_eq!(
        formatted,
        concat!(
            "import \"payrun.mox\";\n",
            "\n",
            "application Payroll {\n",
            "    base nz.example.payroll\n",
            "    module payroll {\n",
            "        service PayRunService {\n",
            "            inject PayRunRepository;\n",
            "            submit => PayRunRepository.save transition Submit when (status == Draft);\n",
            "            reverse => PayRunRepository.save transition PayRunStatus.Reverse;\n",
            "            protected post => PayRunRepository.save capability PostPayRun emits PayRunPosted transition Post;\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
        "canonical clause order emits → transition → when, got:\n{formatted}"
    );
    assert_eq!(
        fmt_ddd(&formatted),
        formatted,
        "formatting must be idempotent"
    );

    // The formatted output parses cleanly.
    let once = parse_ddd(&formatted);
    assert!(
        once.errors.is_empty(),
        "formatted output must parse cleanly, got {:?}",
        once.errors
    );
}

#[test]
fn transition_word_stays_contextual() {
    // `transition` is special only in the clause position between a service
    // operation's `emits` clause and its `when` guard; everywhere else it
    // is an ordinary identifier: service name, op name, class name — and
    // `^transition` escapes as any contextual word. A machine transition
    // line may also be named `transition` (the from-state position).
    let source = concat!(
        "application transition { module m {\n",
        "    service transition {\n",
        "        void transition();\n",
        "    }\n",
        "    entity transition\n",
        "    entity ^transition\n",
        "    machine transition: Status {\n",
        "        initial Draft;\n",
        "        transition -> Posted;\n",
        "        terminal Posted;\n",
        "    }\n",
        "} }\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let module = expect_module(&file, "m");
    assert_eq!(module.designs[0].class.text, "transition");
    assert_eq!(module.designs[1].class.text, "transition");
    assert_eq!(module.services[0].name.text, "transition");
    // The machine block attaches to the preceding design (the `^transition`
    // one), and its transition line may itself be named `transition`.
    let machine = module.designs[1].machine.as_ref().expect("the machine");
    assert_eq!(machine.field.text, "transition");
    assert_eq!(machine.transitions[0].from.text, "transition");
}

#[test]
fn emits_word_stays_contextual() {
    // `emits` is special only between a service operation's capability and
    // guard clauses; everywhere else it is an ordinary identifier: service
    // name, inject target, op name, parameter type, parameter name,
    // capability name, emit name, class name — and `^emits` escapes as any
    // contextual word.
    let source = concat!(
        "application A { module m {\n",
        "    service emits {\n",
        "        inject emits;\n",
        "        boolean emits(emits emits) capability emits;\n",
        "        value => entity.emits emits emits;\n",
        "    }\n",
        "    entity emits\n",
        "    entity ^emits\n",
        "} }\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let module = expect_module(&file, "m");
    assert_eq!(module.designs[0].class.text, "emits");
    assert_eq!(module.designs[1].class.text, "emits");
    let service = &module.services[0];
    assert_eq!(service.name.text, "emits");
    assert_eq!(service.dependencies[0].text, "emits");
    let declared = &service.operations[0];
    assert_eq!(declared.name.text, "emits");
    assert_eq!(
        declared.return_type.as_ref().unwrap().name.full_name(),
        "boolean"
    );
    assert_eq!(declared.params[0].type_ref.name.full_name(), "emits");
    assert_eq!(declared.params[0].name.text, "emits");
    assert_eq!(declared.capabilities[0].text, "emits");
    // A delegated op named `value` targeting `entity.emits`, emitting the
    // event `emits`: the keyword position is behind the delegation target.
    let delegated = &service.operations[1];
    let delegation = delegated.delegation.as_ref().unwrap();
    assert_eq!(delegation.target.full_name(), "entity");
    assert_eq!(delegation.operation.text, "emits");
    assert_eq!(delegated.emits[0].text, "emits");
}

#[test]
fn machine_words_stay_contextual() {
    // `machine`, `initial`, `terminal`, and `via` are ordinary identifiers in
    // every other position: class names, field names, enum names, state
    // names, and escaped forms.
    let source = r#"
application A {
    module m {
        entity machine
        machine initial: terminal {
            initial terminal;
            terminal -> initial via via;
            terminal initial;
        }
        entity ^machine
    }
}
"#;
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let module = expect_module(&file, "m");
    assert_eq!(module.designs[0].class.text, "machine");
    let machine = module.designs[0].machine.as_ref().expect("machine");
    assert_eq!(machine.field.text, "initial");
    assert_eq!(machine.type_.name.full_name(), "terminal");
    assert_eq!(machine.initial.as_ref().expect("initial").text, "terminal");
    assert_eq!(machine.terminals[0].text, "initial");
    assert_eq!(machine.transitions[0].from.text, "terminal");
    assert_eq!(machine.transitions[0].to.text, "initial");
    assert_eq!(machine.transitions[0].trigger.as_ref().unwrap().text, "via");
    assert_eq!(module.designs[1].class.text, "machine");
}

#[test]
fn machine_parse_failures_are_clean_errors() {
    for (source, label) in [
        (
            "application A { module m { machine { initial Open; } } }",
            "missing field and type",
        ),
        (
            "application A { module m { entity E machine status: S { Draft; } } }",
            "transition without an arrow",
        ),
        (
            "application A { module m { entity E machine status: S { initial; } } }",
            "initial without a state",
        ),
        (
            "application A { module m { entity E machine status: S { Draft -> Done via; } } }",
            "via without a trigger",
        ),
        (
            "application A { module m { entity E machine status: S { Open -> Done when total; } } }",
            "when without parens",
        ),
        (
            "application A { module m { entity E machine status: S { Open -> Done, Cancelled; } } }",
            "multi-target arrow",
        ),
    ] {
        let result = parse_ddd(source);
        assert!(!result.errors.is_empty(), "{label} must be an error");
    }
}

#[test]
fn machine_recovered_errors_keep_parsing_the_design() {
    let source = r#"
application A {
    module m {
        entity E
        machine status: S {
            initial Open;
            Open -> Done via;
            terminal Done;
        }
        entity F repository FRepository {
            findById;
        }
    }
}
"#;
    let result = parse_ddd(source);
    assert!(
        !result.errors.is_empty(),
        "the `via` without a trigger must be an error"
    );
    let file = result.ast.expect("a partial AST");
    let module = expect_module(&file, "m");
    assert!(
        module.designs.iter().any(|design| design.class.text == "F"),
        "the design after the broken machine must still parse"
    );
}

#[test]
fn machines_format_canonically_and_are_idempotent() {
    let formatted = fmt_ddd(MACHINE);
    assert_eq!(
        formatted,
        concat!(
            "import \"payrun.mox\";\n",
            "\n",
            "application Payroll {\n",
            "    base nz.example.payroll\n",
            "    module payroll {\n",
            "        service PayRunService {\n",
            "            boolean approve() capability ApprovePayRun when (status == AwaitingApproval);\n",
            "        }\n",
            "        entity PayRun repository PayRunRepository {\n",
            "            findById;\n",
            "            save;\n",
            "            PhysicalMedia[] findByState(String state) when (status == Posted);\n",
            "        }\n",
            "        machine status: PayRunStatus {\n",
            "            initial Draft;\n",
            "            Draft -> Calculating via Calculate;\n",
            "            AwaitingApproval -> Approved via Approve when (total > 0);\n",
            "            Draft -> Reversed;\n",
            "            terminal Reversed;\n",
            "            terminal Posted;\n",
            "        }\n",
            "        entity AuditLog\n",
            "        machine state: AuditState {\n",
            "            initial Open;\n",
            "            Open -> Closed via Close;\n",
            "            terminal Closed;\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
        "the machine source is already canonical, got:\n{formatted}"
    );
    assert_eq!(
        fmt_ddd(&formatted),
        formatted,
        "formatting must be idempotent"
    );
}

#[test]
fn machine_output_round_trips_structurally() {
    let formatted = fmt_ddd(MACHINE);
    let once = parse_ddd(&formatted);
    assert!(
        once.errors.is_empty(),
        "formatted output must parse cleanly, got {:?}",
        once.errors
    );
    let twice = parse_ddd(&fmt_ddd(&formatted));
    assert!(
        twice.errors.is_empty(),
        "re-formatted output must parse cleanly, got {:?}",
        twice.errors
    );
    assert_eq!(once.ast, twice.ast);
}

#[test]
fn adversarial_machine_inputs_do_not_panic() {
    let sources = [
        "machine",
        "machine status",
        "machine status:",
        "machine status: S",
        "machine status: S {",
        "machine status: S { initial",
        "machine status: S { Open ->",
        "machine status: S { Open -> Done when }",
        "machine status: S { Open -> Done when ( }",
        "application A { module m { entity E machine status: S { initial Open; } machine other: T { initial X; } } }",
    ];
    for source in sources {
        let _ = parse_ddd(source);
        // Formatting may fail on lex errors; the contract is only that
        // neither path panics.
        let _ = format_ddd(source);
    }
}

// --- decision links (`invokes` clause, `inject decision`) ----------------------

/// A service exercising the decision-link surface: plain and decision
/// injects, and the `invokes` clause on declared and delegated operations.
const DECISION_LINKS: &str = r#"
application A {
    module m {
        service LoanService {
            inject LoanRepository;
            inject decision underwriting.RiskScore;
            inject decision FraudScreen;
            boolean approve(int amount) capability ApproveLoans invokes RiskScore, underwriting.FraudScreen emits LoanApproved when (true);
            route => riskEngine.evaluate invokes CreditBureau;
        }
    }
}
"#;

#[test]
fn invokes_clauses_parse_on_declared_and_delegated_ops() {
    let result = parse_ddd(DECISION_LINKS);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let service = &expect_module(&file, "m").services[0];

    let approve = &service.operations[0];
    assert_eq!(approve.name.text, "approve");
    let invokes: Vec<String> = approve
        .invokes
        .iter()
        .map(|entry| entry.name.full_name())
        .collect();
    assert_eq!(invokes, vec!["RiskScore", "underwriting.FraudScreen"]);
    // The entries carry no pin here; the spans cover the references
    // themselves, source order kept.
    assert!(approve.invokes.iter().all(|entry| entry.pin.is_none()));
    assert_eq!(
        span_text(DECISION_LINKS, approve.invokes[0].name.span),
        "RiskScore"
    );
    assert_eq!(
        span_text(DECISION_LINKS, approve.invokes[1].name.span),
        "underwriting.FraudScreen"
    );

    // A delegated op carries the clause too (it is a service operation).
    let route = &service.operations[1];
    assert!(route.delegation.is_some());
    assert_eq!(
        route
            .invokes
            .iter()
            .map(|entry| entry.name.full_name())
            .collect::<Vec<_>>(),
        vec!["CreditBureau"]
    );
    assert_eq!(
        span_text(DECISION_LINKS, route.invokes[0].name.span),
        "CreditBureau"
    );
}

#[test]
fn inject_decision_parses_beside_the_plain_form() {
    let result = parse_ddd(DECISION_LINKS);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let service = &expect_module(&file, "m").services[0];

    // Plain `inject` is unchanged; `inject decision` collects its own list,
    // both in source order.
    assert_eq!(
        service
            .dependencies
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["LoanRepository"]
    );
    assert_eq!(
        service
            .decision_dependencies
            .iter()
            .map(|entry| entry.name.full_name())
            .collect::<Vec<_>>(),
        vec!["underwriting.RiskScore", "FraudScreen"]
    );
    assert!(service
        .decision_dependencies
        .iter()
        .all(|entry| entry.pin.is_none()));
}

/// The `version` pins (pherolang#154): `compatible` and the exact string,
/// per `invokes` entry and on the `inject decision` line, and the
/// committed-missing-value error.
#[test]
fn version_pins_parse_on_invokes_and_inject_decision() {
    let source = r#"
application A {
    module m {
        service LoanService {
            inject decision underwriting.RiskScore version compatible;
            inject decision FraudScreen version "1.4.2";
            boolean approve() invokes RiskScore version compatible, underwriting.FraudScreen version "1.4.2", CreditBureau;
        }
    }
}
"#;
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let service = &expect_module(&file, "m").services[0];

    use phero_syntax::ast::DddVersionPin;
    let injects = &service.decision_dependencies;
    assert_eq!(
        injects
            .iter()
            .map(|entry| entry.name.full_name())
            .collect::<Vec<_>>(),
        vec!["underwriting.RiskScore", "FraudScreen"]
    );
    assert_eq!(injects[0].pin, Some(DddVersionPin::Compatible));
    assert_eq!(injects[1].pin, Some(DddVersionPin::Exact("1.4.2".into())));

    let invokes = &service.operations[0].invokes;
    assert_eq!(invokes.len(), 3);
    assert_eq!(invokes[0].pin, Some(DddVersionPin::Compatible));
    assert_eq!(invokes[1].pin, Some(DddVersionPin::Exact("1.4.2".into())));
    assert_eq!(
        invokes[2].pin, None,
        "the pin-less entry keeps today's shape"
    );
}

#[test]
fn a_bare_version_pin_recovers_with_an_error() {
    // The pin is committed once `version` matched: a missing value is a
    // syntax error, and the clause still recovers.
    let source = r#"
application A {
    module m {
        service S {
            boolean broken() invokes RiskScore version;
            boolean fine();
        }
    }
}
"#;
    let result = parse_ddd(source);
    assert!(
        !result.errors.is_empty(),
        "a bare `version` pin is a reported error"
    );
    assert!(result.ast.is_some(), "the service still recovers");
}

#[test]
fn a_missing_invokes_name_recovers_with_an_error() {
    // The clause is committed once the keyword matched: a missing name list
    // is a syntax error, and the rest of the service still parses.
    let source = r#"
application A {
    module m {
        service S {
            boolean broken() invokes;
            boolean fine();
        }
    }
}
"#;
    let result = parse_ddd(source);
    assert!(
        !result.errors.is_empty(),
        "the empty `invokes` clause must be an error"
    );
    let file = result.ast.expect("a partial AST");
    let service = &expect_module(&file, "m").services[0];
    assert!(
        service
            .operations
            .iter()
            .any(|op| op.name.text == "fine" && op.invokes.is_empty()),
        "the operation after the broken clause must still parse"
    );
}

#[test]
fn decision_links_format_canonically_and_round_trip() {
    // The canonical clause order is `capability ... invokes ... emits ...
    // when ...`; the decision inject re-emits with its discriminator.
    let canonical = concat!(
        "application A {\n",
        "    module m {\n",
        "        service LoanService {\n",
        "            inject LoanRepository;\n",
        "            inject decision underwriting.RiskScore;\n",
        "            inject decision FraudScreen;\n",
        "            boolean approve(int amount) capability ApproveLoans invokes RiskScore, underwriting.FraudScreen emits LoanApproved when (true);\n",
        "            route => riskEngine.evaluate invokes CreditBureau;\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    assert_eq!(fmt_ddd(DECISION_LINKS), canonical);
    // Idempotence: the canonical output is a fixpoint.
    assert_eq!(fmt_ddd(canonical), canonical);
    // The formatted output parses back to the same AST.
    let once = parse_ddd(&fmt_ddd(DECISION_LINKS));
    let twice = parse_ddd(&fmt_ddd(&fmt_ddd(DECISION_LINKS)));
    assert!(once.errors.is_empty(), "{:?}", once.errors);
    assert!(twice.errors.is_empty(), "{:?}", twice.errors);
    assert_eq!(once.ast, twice.ast);
}

#[test]
fn invokes_and_decision_stay_contextual_identifiers() {
    // `^invokes` is an ordinary (escaped) operation name; `^decision` is a
    // plain dependency named `decision`; neither parses as its keyword.
    let source = concat!(
        "application A {\n",
        "    module m {\n",
        "        service S {\n",
        "            inject ^decision;\n",
        "            boolean ^invokes();\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let service = &expect_module(&file, "m").services[0];
    assert_eq!(
        service
            .dependencies
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["decision"]
    );
    assert!(service.decision_dependencies.is_empty());
    assert_eq!(service.operations[0].name.text, "invokes");
    assert!(service.operations[0].invokes.is_empty());
    // The escaped spellings survive formatting (the escape is load-bearing).
    let formatted = fmt_ddd(source);
    assert!(formatted.contains("inject ^decision;"), "{formatted}");
    assert!(formatted.contains("boolean ^invokes();"), "{formatted}");
}

// --- child-scoped `children` blocks (issue pherolang#179) -------------------

/// A module with two `children` blocks over one aggregate root: a collection
/// child with `add`/`remove`/`save` actions (one `protected`, one guarded),
/// and a scalar child with `save`.
const CHILDREN_SOURCE: &str = r#"
import "payroll.mox";

application Payroll {

    base nz.example.payroll

    module payroll {
        entity PayRun repository PayRunRepository {
            findById;
        }
        children PayRun {
            boolean addLine(PayRunLineDraft line) => lines.add;
            protected boolean removeLine(String lineId) => lines.remove when (lineId != "");
        }
        children PayRun {
            boolean saveLine(PayRunLineDraft line) => lines.save;
        }
    }
}
"#;

#[test]
fn children_blocks_parse_with_child_operations() {
    let result = parse_ddd(CHILDREN_SOURCE);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let module = expect_module(&file, "payroll");
    assert_eq!(module.extends.len(), 2, "both blocks are kept in order");

    let first = &module.extends[0];
    assert_eq!(first.class.text, "PayRun");
    assert_eq!(first.operations.len(), 2);

    let add = &first.operations[0];
    assert_eq!(add.name.text, "addLine");
    assert_eq!(add.return_type.name.full_name(), "boolean");
    assert_eq!(add.params[0].name.text, "line");
    assert_eq!(add.params[0].type_ref.name.full_name(), "PayRunLineDraft");
    assert_eq!(add.feature.text, "lines");
    assert_eq!(add.action.text, "add");
    assert!(!add.is_protected);
    assert!(add.condition.is_none());

    let remove = &first.operations[1];
    assert!(remove.is_protected);
    assert_eq!(remove.action.text, "remove");
    assert_eq!(
        span_text(CHILDREN_SOURCE, remove.condition.expect("the guard")),
        "(lineId != \"\")"
    );

    assert_eq!(module.extends[1].operations[0].action.text, "save");
}

#[test]
fn children_formats_canonically_and_is_idempotent() {
    let canonical = concat!(
        "import \"payroll.mox\";\n",
        "\n",
        "application Payroll {\n",
        "    base nz.example.payroll\n",
        "    module payroll {\n",
        "        entity PayRun repository PayRunRepository {\n",
        "            findById;\n",
        "        }\n",
        "        children PayRun {\n",
        "            boolean addLine(PayRunLineDraft line) => lines.add;\n",
        "            protected boolean removeLine(String lineId) => lines.remove when (lineId != \"\");\n",
        "        }\n",
        "        children PayRun {\n",
        "            boolean saveLine(PayRunLineDraft line) => lines.save;\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    let formatted = fmt_ddd(CHILDREN_SOURCE);
    assert_eq!(formatted, canonical, "children layout drifted");
    assert_eq!(fmt_ddd(&formatted), formatted, "formatting is a fixpoint");
    assert_eq!(
        parse_ddd(&formatted).errors.len(),
        0,
        "the canonical form parses"
    );
}

/// The 0.4 `extend` spelling (DF4, issue pherolang#327): removed from the
/// grammar in 0.5 — a parse error, never a warning.
const EXTEND_ALIAS_SOURCE: &str = r#"
import "payroll.mox";

application Payroll {

    base nz.example.payroll

    module payroll {
        entity PayRun repository PayRunRepository {
            findById;
        }
        extend PayRun {
            boolean addLine(PayRunLineDraft line) => lines.add;
        }
    }
}
"#;

#[test]
fn the_extend_alias_is_a_parse_error() {
    let result = parse_ddd(EXTEND_ALIAS_SOURCE);
    assert!(
        !result.errors.is_empty(),
        "`extend` must fail to parse in 0.5 (DF4): {:?}",
        result.errors
    );
}

#[test]
fn fmt_still_migrates_the_extend_alias() {
    // The token-level rewrite survives the grammar removal: `pherolang fmt`
    // is the mechanical migration path for 0.4 sources.
    let formatted = fmt_ddd(EXTEND_ALIAS_SOURCE);
    assert!(
        !formatted.contains("extend"),
        "the alias must be rewritten: {formatted}"
    );
    assert_eq!(
        parse_ddd(&formatted).errors.len(),
        0,
        "the migrated source parses"
    );
    assert_eq!(fmt_ddd(&formatted), formatted, "formatting is a fixpoint");
}

#[test]
fn child_op_without_an_action_is_an_error() {
    let source = r#"
import "payroll.mox"

application Payroll {
    base nz.example.payroll
    module payroll {
        entity PayRun repository PayRunRepository {
            findById;
        }
        children PayRun {
            boolean addLine(PayRunLineDraft line) => lines;
        }
    }
}
"#;
    let result = parse_ddd(source);
    assert!(
        result
            .errors
            .iter()
            .any(|error| error.message.contains("expected `feature.action`")),
        "a one-segment child target is a syntax error: {:?}",
        result.errors
    );
}

#[test]
fn extend_is_an_ordinary_identifier() {
    // Since 0.5 `extend` is special nowhere (the 0.4 alias is gone), so a
    // design may even be named `extend`.
    let source = r#"
application Payroll {
    module payroll {
        entity extend
    }
}
"#;
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "a design may be named `extend`: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    assert_eq!(
        expect_module(&file, "payroll").designs[0].class.text,
        "extend"
    );
}

#[test]
fn a_junk_child_operation_recovers_within_the_children_block() {
    let source = r#"
import "payroll.mox"

application Payroll {
    base nz.example.payroll
    module payroll {
        children PayRun {
            nope;
            boolean addLine(PayRunLineDraft line) => lines.add;
        }
    }
}
"#;
    let result = parse_ddd(source);
    assert!(
        !result.errors.is_empty(),
        "the junk line is reported, not silently dropped"
    );
    let file = result.ast.expect("the file still parses");
    let module = expect_module(&file, "payroll");
    assert_eq!(
        module.extends[0]
            .operations
            .iter()
            .map(|operation| operation.name.text.as_str())
            .collect::<Vec<_>>(),
        ["addLine"],
        "the valid operation after the junk still parses"
    );
}

// --- executable operation bodies (issue #178) --------------------------------

const BODIES: &str = r#"
import "payroll.mox"

application Payroll {
    base nz.example.payroll
    module payroll {
        service PayRunService {
            inject PayRunRepository;
            submit(PayRunDraft draft) capability SubmitPayRun when (draft.quantity > 0) => PayRunRepository {
                let run=self.findById("x") else error NotFound("PayRun","x");
                guard run.total>=0 else error Conflict("total");
                run.total=draft.quantity;
                if (run.total>0) { let positive=true; } else { let positive=false; }
            }
            boolean approve() => PayRunRepository {
                return true;
            }
        }
        entity PayRun repository PayRunRepository {
            findById;
            save;
            int totalQuantity(String runId) when (runId != "") {
                let run = self.findById(runId) else error NotFound("PayRun", runId);
                return run.lines.sum(line => line.quantity);
            }
        }
    }
}
"#;

#[test]
fn service_and_repository_bodies_parse_into_statements() {
    let result = parse_ddd(BODIES);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let module = expect_module(&file, "payroll");
    let service = &module.services[0];
    let submit = &service.operations[0];
    assert_eq!(submit.name.text, "submit");
    assert_eq!(
        submit.body_receiver.as_ref().map(|name| name.text.as_str()),
        Some("PayRunRepository")
    );
    let body = submit.body.as_ref().expect("the service body parsed");
    assert_eq!(body.stmts.len(), 4, "let, guard, assign, if");

    let DddStmt::Let {
        name,
        expr,
        raise: Some(raise),
    } = &body.stmts[0]
    else {
        panic!("expected a let-else, got {:?}", body.stmts[0]);
    };
    assert_eq!(name.text, "run");
    assert_eq!(span_text(BODIES, *expr), r#"self.findById("x")"#);
    assert_eq!(raise.variant.text, "NotFound");
    assert_eq!(
        raise
            .args
            .iter()
            .map(|arg| span_text(BODIES, *arg))
            .collect::<Vec<_>>(),
        [r#""PayRun""#, r#""x""#],
    );

    assert!(matches!(&body.stmts[1], DddStmt::Guard { .. }));
    let DddStmt::Assign { path, expr } = &body.stmts[2] else {
        panic!("expected an assignment, got {:?}", body.stmts[2]);
    };
    assert_eq!(
        path.iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        ["run", "total"]
    );
    assert_eq!(span_text(BODIES, *expr), "draft.quantity");

    let DddStmt::If { then, else_, .. } = &body.stmts[3] else {
        panic!("expected an if statement, got {:?}", body.stmts[3]);
    };
    assert_eq!(then.len(), 1);
    assert_eq!(else_.len(), 1);

    let approve = &service.operations[1];
    assert!(matches!(
        approve.body.as_ref().unwrap().stmts[0],
        DddStmt::Return { expr: Some(_) }
    ));

    // The repository operation's declared body carries no receiver.
    let repository = module.designs[0].repository.as_ref().expect("repository");
    assert_eq!(
        repository.operations[2].body.as_ref().unwrap().stmts.len(),
        2
    );
}

#[test]
fn service_and_repository_bodies_format_canonically() {
    let formatted = fmt_ddd(BODIES);
    let expected = r#"import "payroll.mox";

application Payroll {
    base nz.example.payroll
    module payroll {
        service PayRunService {
            inject PayRunRepository;
            submit(PayRunDraft draft) capability SubmitPayRun when (draft.quantity > 0) => PayRunRepository {
                let run = self.findById("x") else error NotFound("PayRun", "x");
                guard run.total >= 0 else error Conflict("total");
                run.total = draft.quantity;
                if (run.total > 0) {
                    let positive = true;
                } else {
                    let positive = false;
                }
            }
            boolean approve() => PayRunRepository {
                return true;
            }
        }
        entity PayRun repository PayRunRepository {
            findById;
            save;
            int totalQuantity(String runId) when (runId != "") {
                let run = self.findById(runId) else error NotFound("PayRun", runId);
                return run.lines.sum(line => line.quantity);
            }
        }
    }
}
"#;
    assert_eq!(formatted, expected);
    assert_eq!(fmt_ddd(&formatted), formatted, "formatting is a fixpoint");
}

#[test]
fn unsupported_statement_names_the_construct() {
    let source = r#"
application A {
    module M {
        service S {
            boolean submit() => R {
                while (true) { return true; }
            }
        }
    }
}
"#;
    let result = parse_ddd(source);
    assert!(
        result.errors.iter().any(|error| error
            .message
            .contains("unsupported statement 'while' in a service body: loops")),
        "expected the named refusal, got: {:?}",
        result.errors
    );
}

#[test]
fn builtin_repository_body_is_refused() {
    let source = r#"
application A {
    module M {
        entity E repository R {
            save { let x = 1; }
        }
    }
}
"#;
    let result = parse_ddd(source);
    assert!(
        result.errors.iter().any(|error| error
            .message
            .contains("built-in repository operation 'save' takes no body")),
        "expected the builtin-body refusal, got: {:?}",
        result.errors
    );
}

#[test]
fn multi_segment_body_receiver_is_refused() {
    let source = r#"
application A {
    module M {
        service S {
            boolean submit() => a.b.RunRepository {
                return true;
            }
        }
    }
}
"#;
    let result = parse_ddd(source);
    assert!(
        result.errors.iter().any(|error| error
            .message
            .contains("body receiver must be a single repository name")),
        "expected the receiver refusal, got: {:?}",
        result.errors
    );
}

// --- DTO projections and standalone declarations (DD3, issue #300) -----------

/// The three DTO forms: a projection DTO (`from <source>`), a standalone DTO
/// (declared members), and a mirror DTO (flags only, no members).
const DTOS: &str = r#"
import "library.mox"

application Library {
    module catalogue {
        entity Book
        dto BookSummary from Book {
            title;
            isbn;
            rating = playLength / 60;
        }
        dto CreateBookRequest {
            String title;
            String isbn;
            int[] tags;
            Book[] shelves;
        }
        dto User nonPersistent
    }
}
"#;

fn expect_design<'a>(module: &'a DddModule, name: &str) -> &'a DddDesign {
    module
        .designs
        .iter()
        .find(|design| design.class.text == name)
        .unwrap_or_else(|| panic!("expected design `{name}`"))
}

#[test]
fn dto_projection_parses() {
    let result = parse_ddd(DTOS);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let catalogue = expect_module(&file, "catalogue");

    let summary = expect_design(catalogue, "BookSummary");
    assert_eq!(summary.stereotype, DddStereotype::Dto);
    assert_eq!(
        summary
            .dto_source
            .as_ref()
            .expect("from source")
            .full_name(),
        "Book"
    );
    let members = match summary.dto_members.as_ref().expect("members") {
        DddDtoMembers::Projection(members) => members,
        other => panic!("expected projection members, got {other:?}"),
    };
    assert_eq!(members.len(), 3);
    match &members[0] {
        DddProjectionMember::Select { name, span } => {
            assert_eq!(name.text, "title");
            assert_eq!(span_text(DTOS, *span), "title;");
        }
        other => panic!("expected a select member, got {other:?}"),
    }
    match &members[1] {
        DddProjectionMember::Select { name, .. } => assert_eq!(name.text, "isbn"),
        other => panic!("expected a select member, got {other:?}"),
    }
    match &members[2] {
        DddProjectionMember::Assign { name, expr, span } => {
            assert_eq!(name.text, "rating");
            assert_eq!(span_text(DTOS, *expr), "playLength / 60");
            assert_eq!(span_text(DTOS, *span), "rating = playLength / 60;");
        }
        other => panic!("expected an assign member, got {other:?}"),
    }
}

#[test]
fn dto_standalone_declares_typed_members() {
    let result = parse_ddd(DTOS);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let catalogue = expect_module(&file, "catalogue");

    let request = expect_design(catalogue, "CreateBookRequest");
    assert_eq!(request.stereotype, DddStereotype::Dto);
    assert!(request.dto_source.is_none());
    let members = match request.dto_members.as_ref().expect("members") {
        DddDtoMembers::Declared(members) => members,
        other => panic!("expected declared members, got {other:?}"),
    };
    assert_eq!(members.len(), 4);
    assert_eq!(members[0].type_ref.name.full_name(), "String");
    assert!(members[0].multiplicity.is_none());
    assert_eq!(members[0].name.text, "title");
    assert_eq!(span_text(DTOS, members[0].span), "String title;");
    assert_eq!(members[2].type_ref.name.full_name(), "int");
    assert!(matches!(
        members[2].multiplicity.as_ref().unwrap().kind,
        MultiplicityKind::Unbounded
    ));
    assert_eq!(members[2].name.text, "tags");
    assert_eq!(
        span_text(DTOS, members[3].multiplicity.as_ref().unwrap().span),
        "[]"
    );
}

#[test]
fn dto_mirror_has_no_source_or_members() {
    let result = parse_ddd(DTOS);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let user = expect_design(expect_module(&file, "catalogue"), "User");
    assert_eq!(user.stereotype, DddStereotype::Dto);
    assert!(user.dto_source.is_none());
    assert!(user.dto_members.is_none());
    assert!(user.flags.non_persistent.is_some());
}

#[test]
fn entity_has_no_dto_suffix_fields() {
    let result = parse_ddd(DTOS);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let book = expect_design(expect_module(&file, "catalogue"), "Book");
    assert!(book.dto_source.is_none());
    assert!(book.dto_members.is_none());
}

#[test]
fn dto_duplicate_member_names_are_allowed_at_parse_time() {
    let source = concat!(
        "application A { module m {\n",
        "    dto Summary from Book { title; title; }\n",
        "    dto Request { String title; String title; }\n",
        "} }\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "duplicates are a driver concern, not a syntax error: {:?}",
        result.errors
    );
    let module = &result.ast.unwrap().application.unwrap().modules[0];
    let summary = expect_design(module, "Summary");
    let DddDtoMembers::Projection(members) = summary.dto_members.as_ref().unwrap() else {
        panic!("expected projection members");
    };
    assert_eq!(members.len(), 2);
    let request = expect_design(module, "Request");
    let DddDtoMembers::Declared(members) = request.dto_members.as_ref().unwrap() else {
        panic!("expected declared members");
    };
    assert_eq!(members.len(), 2);
}

#[test]
fn dto_from_on_a_non_dto_design_is_an_error() {
    for source in [
        "application A { module m { entity Book from Other { title; } } }",
        "application A { module m { value Money from Other { amount; } } }",
    ] {
        let result = parse_ddd(source);
        assert!(
            result
                .errors
                .iter()
                .any(|error| error.message.contains("only allowed on a `dto` design")),
            "expected the dto-only refusal for {source:?}, got {:?}",
            result.errors
        );
    }
}

#[test]
fn dto_member_block_on_a_non_dto_design_is_an_error() {
    let result = parse_ddd("application A { module m { entity Book { String title; } } }");
    assert!(
        result
            .errors
            .iter()
            .any(|error| error.message.contains("only allowed on a `dto` design")),
        "expected the dto-only refusal, got {:?}",
        result.errors
    );
}

#[test]
fn dto_projection_rejects_declared_members() {
    let result =
        parse_ddd("application A { module m { dto Request from Book { String title; } } }");
    assert!(
        result
            .errors
            .iter()
            .any(|error| error.message.contains("projection member")),
        "expected the projection-member refusal, got {:?}",
        result.errors
    );
}

#[test]
fn dto_standalone_rejects_projection_shorthand() {
    for source in [
        "application A { module m { dto Request { title; } } }",
        "application A { module m { dto Request { rating = playLength / 60; } } }",
    ] {
        let result = parse_ddd(source);
        assert!(
            result
                .errors
                .iter()
                .any(|error| error.message.contains("declared member")),
            "expected the declared-member refusal for {source:?}, got {:?}",
            result.errors
        );
    }
}

#[test]
fn dto_from_without_a_projection_block_is_an_error() {
    let result = parse_ddd("application A { module m { dto Summary from Book } }");
    assert!(
        result
            .errors
            .iter()
            .any(|error| error.message.contains("projection block")),
        "expected the missing-block error, got {:?}",
        result.errors
    );
}

#[test]
fn search_document_rejects_the_select_shorthand() {
    let result = parse_ddd("application A { module m { search S { document { title; } } } }");
    assert!(
        result
            .errors
            .iter()
            .any(|error| error.message.contains("document entry")),
        "expected the search-document refusal, got {:?}",
        result.errors
    );
}

#[test]
fn dto_forms_format_canonically_and_round_trip() {
    let formatted = fmt_ddd(DTOS);
    assert_eq!(
        formatted,
        concat!(
            "import \"library.mox\";\n",
            "\n",
            "application Library {\n",
            "    module catalogue {\n",
            "        entity Book\n",
            "        dto BookSummary from Book {\n",
            "            title;\n",
            "            isbn;\n",
            "            rating = playLength / 60;\n",
            "        }\n",
            "\n",
            "        dto CreateBookRequest {\n",
            "            String title;\n",
            "            String isbn;\n",
            "            int[] tags;\n",
            "            Book[] shelves;\n",
            "        }\n",
            "\n",
            "        dto User nonPersistent\n",
            "    }\n",
            "}\n",
        ),
        "canonical DTO layout, got:\n{formatted}"
    );
    assert_eq!(
        fmt_ddd(&formatted),
        formatted,
        "formatting must be idempotent"
    );
    let once = parse_ddd(&formatted);
    assert!(
        once.errors.is_empty(),
        "formatted output must parse cleanly, got {:?}",
        once.errors
    );
    let twice = parse_ddd(&fmt_ddd(&formatted));
    assert_eq!(once.ast, twice.ast, "formatted output must round-trip");
}

#[test]
fn dto_member_comments_are_preserved() {
    let source = concat!(
        "application A { module m {\n",
        "    dto Summary from Book {\n",
        "        // selected\n",
        "        title; // the title\n",
        "        rating = playLength / 60; // derived\n",
        "    }\n",
        "    dto Request {\n",
        "        // declared\n",
        "        String title;\n",
        "    }\n",
        "} }\n",
    );
    let expected = concat!(
        "application A {\n",
        "    module m {\n",
        "        dto Summary from Book {\n",
        "            // selected\n",
        "            title; // the title\n",
        "            rating = playLength / 60; // derived\n",
        "        }\n",
        "\n",
        "        dto Request {\n",
        "            // declared\n",
        "            String title;\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    assert_eq!(fmt_ddd(source), expected);
    assert_eq!(fmt_ddd(expected), expected, "formatting must be idempotent");
}

#[test]
fn from_word_stays_usable_as_an_escaped_name() {
    // `from` is a real lexer keyword; the escaped form remains an ordinary
    // name in every design position, including directly ahead of the DTO
    // projection's `from` word.
    let source = concat!(
        "application A { module m {\n",
        "    entity ^from\n",
        "    dto ^from nonPersistent\n",
        "    dto ^from from Book { title; }\n",
        "} }\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let module = &result.ast.unwrap().application.unwrap().modules[0];
    assert_eq!(module.designs[0].class.text, "from");
    assert!(module.designs[0].class.escaped);
    assert_eq!(module.designs[1].class.text, "from");
    assert!(module.designs[1].dto_members.is_none());
    let projection = &module.designs[2];
    assert_eq!(projection.class.text, "from");
    assert_eq!(
        projection
            .dto_source
            .as_ref()
            .expect("from source")
            .full_name(),
        "Book"
    );
}

#[test]
fn adversarial_dto_inputs_do_not_panic() {
    for source in [
        "dto",
        "dto Summary from",
        "dto Summary from Book {",
        "dto Summary from Book { = ; }",
        "dto Summary from Book { title title }",
        "dto Request { String }",
        "dto Request { ; }",
        "dto Request from Book from Other { }",
        "application A { module m { dto Summary from Book { title; } } }",
        "application A { module m { dto Summary from Book { title; } dto Other { int x; } } }",
    ] {
        let _ = parse_ddd(source);
        let _ = format_ddd(source);
    }
}

// --- declared error vocabulary and `throws` (DD7, issue pherolang#304) -------

/// An application declares errors at both scopes with typed payloads, a doc
/// comment, and `throws` clauses on declared, delegated, and body service
/// operations and on declared repository operations.
#[test]
fn error_declarations_and_throws_parse() {
    let source = r#"
application Payments {
    base nz.example.payments

    /// The payment was refused by the provider.
    error PaymentDeclined(DeclineReason reason, String reference);
    error OutOfStock(String sku);
    error Empty();

    module billing {
        error InvoiceAlreadyIssued(String invoiceId);
        service PaymentService {
            inject PaymentRepository;
            Receipt pay(OrderId orderId) capability PayOrder throws PaymentDeclined, OutOfStock when (order.status == AwaitingPayment);
            submit => PaymentRepository.save throws PaymentDeclined;
            boolean settle(String id) capability Settle throws NotFound => PaymentRepository {
                let run = self.findById(id) else error NotFound("Payment", id);
                return true;
            }
        }
        entity Payment repository PaymentRepository {
            findById;
            save;
            Receipt capture(String id) throws PaymentDeclined, Conflict when (status == Pending);
        }
    }
}
"#;
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let application = result.ast.unwrap().application.unwrap();
    assert_eq!(application.errors.len(), 3);
    assert_eq!(application.errors[0].name.text, "PaymentDeclined");
    assert_eq!(
        application.errors[0].doc.as_deref(),
        Some("The payment was refused by the provider.")
    );
    assert_eq!(application.errors[0].fields.len(), 2);
    assert_eq!(application.errors[0].fields[0].name.text, "reason");
    assert_eq!(application.errors[0].fields[1].name.text, "reference");
    assert!(application.errors[2].fields.is_empty());

    let module = &application.modules[0];
    assert_eq!(module.errors.len(), 1);
    assert_eq!(module.errors[0].name.text, "InvoiceAlreadyIssued");

    let pay = &module.services[0].operations[0];
    assert_eq!(
        pay.throws
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        ["PaymentDeclined", "OutOfStock"]
    );
    let submit = &module.services[0].operations[1];
    assert!(submit.delegation.is_some());
    assert_eq!(submit.throws.len(), 1);
    let settle = &module.services[0].operations[2];
    assert_eq!(settle.throws.len(), 1);
    assert!(settle.body.is_some());

    let repository = module.designs[0].repository.as_ref().expect("repository");
    let capture = repository
        .operations
        .iter()
        .find(|operation| operation.name.text == "capture")
        .expect("capture");
    assert_eq!(
        capture
            .throws
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        ["PaymentDeclined", "Conflict"]
    );
    // A builtin takes no `throws`.
    assert!(repository.operations[0].throws.is_empty());
}

/// The declarations and `throws` clauses re-emit in canonical position:
/// `base`, then the application errors, then the modules; each module's
/// errors lead its body; `throws` sits between `transition` and `when`.
#[test]
fn error_declarations_and_throws_format_canonically() {
    let source = concat!(
        "import \"payments.mox\";\n",
        "\n",
        "application Payments {\n",
        "    base nz.example.payments\n",
        "    error PaymentDeclined(DeclineReason reason,String reference) ;\n",
        "    module billing {\n",
        "        error InvoiceAlreadyIssued(String invoiceId);\n",
        "        service PaymentService {\n",
        "            inject PaymentRepository;\n",
        "            Receipt pay(OrderId orderId) capability PayOrder throws PaymentDeclined,OutOfStock when (order.status == AwaitingPayment);\n",
        "            submit => PaymentRepository.save transition Submit throws PaymentDeclined;\n",
        "            boolean settle(String id) throws NotFound => PaymentRepository {\n",
        "                let run = self.findById(id) else error NotFound(\"Payment\", id);\n",
        "                return true;\n",
        "            }\n",
        "        }\n",
        "        entity Payment repository PaymentRepository {\n",
        "            findById;\n",
        "            Receipt capture(String id) throws PaymentDeclined,Conflict when (status == Pending);\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    let expected = concat!(
        "import \"payments.mox\";\n",
        "\n",
        "application Payments {\n",
        "    base nz.example.payments\n",
        "    error PaymentDeclined(DeclineReason reason, String reference);\n",
        "    module billing {\n",
        "        error InvoiceAlreadyIssued(String invoiceId);\n",
        "        service PaymentService {\n",
        "            inject PaymentRepository;\n",
        "            Receipt pay(OrderId orderId) capability PayOrder throws PaymentDeclined, OutOfStock when (order.status == AwaitingPayment);\n",
        "            submit => PaymentRepository.save transition Submit throws PaymentDeclined;\n",
        "            boolean settle(String id) throws NotFound => PaymentRepository {\n",
        "                let run = self.findById(id) else error NotFound(\"Payment\", id);\n",
        "                return true;\n",
        "            }\n",
        "        }\n",
        "        entity Payment repository PaymentRepository {\n",
        "            findById;\n",
        "            Receipt capture(String id) throws PaymentDeclined, Conflict when (status == Pending);\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    assert_eq!(fmt_ddd(source), expected);
    assert_eq!(fmt_ddd(expected), expected, "formatting is a fixpoint");
}

/// `error` declarations lead their enclosing body: a late application error
/// (after a module) and a late module error (after a member) are named syntax
/// errors and are dropped from the canonical AST.
#[test]
fn late_error_declarations_are_named_errors() {
    let application = concat!(
        "application A {\n",
        "    module m { }\n",
        "    error Late(String x);\n",
        "}\n",
    );
    let result = parse_ddd(application);
    assert!(
        result.errors.iter().any(|error| error
            .message
            .contains("`error` declarations must precede the application's modules")),
        "expected the application-scope order refusal, got: {:?}",
        result.errors
    );
    assert!(result
        .ast
        .expect("recovered AST")
        .application
        .expect("application")
        .errors
        .is_empty());

    let module = concat!(
        "application A {\n",
        "    module m {\n",
        "        service S { boolean x(); }\n",
        "        error Late(String x);\n",
        "    }\n",
        "}\n",
    );
    let result = parse_ddd(module);
    assert!(
        result.errors.iter().any(|error| error
            .message
            .contains("`error` declarations must precede the module's members")),
        "expected the module-scope order refusal, got: {:?}",
        result.errors
    );
}

/// A built-in repository operation takes no `throws` clause: the refusal
/// names the builtin, and the clause is consumed so recovery continues.
#[test]
fn builtin_repository_throws_is_refused() {
    let source = concat!(
        "application A {\n",
        "    module m {\n",
        "        entity E repository R { findById throws NotFound; save; }\n",
        "    }\n",
        "}\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.iter().any(|error| error
            .message
            .contains("built-in repository operation 'findById' takes no `throws` clause")),
        "expected the builtin-throws refusal, got: {:?}",
        result.errors
    );
    assert!(
        !result
            .errors
            .iter()
            .any(|error| error.message.contains("expected a repository operation")),
        "the clause is consumed whole: {:?}",
        result.errors
    );
    let module = &result.ast.unwrap().application.unwrap().modules[0];
    let repository = module.designs[0].repository.as_ref().expect("repository");
    assert_eq!(
        repository.operations.len(),
        1,
        "the refused builtin is dropped; save still parses"
    );
    assert_eq!(repository.operations[0].name.text, "save");
}

/// `throws` is contextual: an operation may be named `throws`, and an
/// escaped `^throws` is an ordinary name.
#[test]
fn throws_stays_a_contextual_identifier() {
    let source = concat!(
        "application A {\n",
        "    module m {\n",
        "        service S { boolean throws(); boolean ^throws(); }\n",
        "        entity E repository R { boolean throws(int x); }\n",
        "    }\n",
        "}\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let module = &result.ast.unwrap().application.unwrap().modules[0];
    assert_eq!(module.services[0].operations[0].name.text, "throws");
    assert_eq!(module.services[0].operations[1].name.text, "throws");
    assert!(module.services[0].operations[1].name.escaped);
}

/// An error declaration with a malformed payload recovers; the declaration
/// still parses where it can, and a missing `throws` name list is a named
/// error.
#[test]
fn malformed_error_vocabulary_inputs_do_not_panic() {
    for source in [
        "application A { error ; }",
        "application A { error E( }",
        "application A { error E(String); }",
        "application A { module m { error E { } } }",
        "application A { module m { service S { boolean x() throws; } } }",
        "application A { module m { service S { boolean x() throws A B; } } }",
        "application A { module m { entity E repository R { boolean x() throws ; } } }",
    ] {
        let _ = parse_ddd(source);
        let _ = format_ddd(source);
    }
}

/// The orchestration-only exclusions are named refusals: loops,
/// `try`/`catch`, compound assignment, collection mutation, and bare
/// expression statements each say what was refused and where it belongs.
#[test]
fn orchestration_only_exclusions_are_named_refusals() {
    let body = |statement: &str| {
        format!(
            "application A {{\n    module M {{\n        service S {{\n            \
             boolean submit() => R {{\n                {statement}\n            }}\n        }}\n    }}\n}}\n"
        )
    };
    let cases = [
        (
            "for (x in run.lines) { }",
            "unsupported statement 'for' in a service body: loops (`for`, `while`)",
        ),
        (
            "while (true) { return true; }",
            "unsupported statement 'while' in a service body: loops (`for`, `while`)",
        ),
        (
            "try { return true; } catch { }",
            "unsupported statement 'try' in a service body: `try`/`catch`",
        ),
        (
            "run.total += 1;",
            "unsupported compound assignment '+=' in a service body",
        ),
        (
            "run.lines.push(line);",
            "unsupported collection mutation 'push' in a service body",
        ),
        (
            "run.lines.remove(0);",
            "unsupported collection mutation 'remove' in a service body",
        ),
        (
            "run.lines.clear();",
            "unsupported collection mutation 'clear' in a service body",
        ),
        (
            "self.save(run);",
            "unsupported expression statement in a service body: bind the call's result",
        ),
    ];
    for (statement, expected) in cases {
        let result = parse_ddd(&body(statement));
        assert!(
            result
                .errors
                .iter()
                .any(|error| error.message.contains(expected)),
            "expected `{expected}` for `{statement}`, got: {:?}",
            result.errors
        );
    }
}

/// Feature-path assignment, a domain `op` call bound by `let`, and a
/// feature-path expression statement are legal (the orchestration cap keeps
/// the assignment form).
#[test]
fn orchestration_only_keeps_feature_path_assignment_and_bound_calls() {
    let source = concat!(
        "application A {\n",
        "    module M {\n",
        "        service S {\n",
        "            boolean submit() => R {\n",
        "                let run = self.findById(\"x\") else error NotFound(\"Run\", \"x\");\n",
        "                let submitted = run.submit();\n",
        "                run.total = submitted;\n",
        "                return true;\n",
        "            }\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let module = &result.ast.unwrap().application.unwrap().modules[0];
    let operation = &module.services[0].operations[0];
    let body = operation.body.as_ref().expect("body");
    assert!(matches!(
        &body.stmts[1],
        DddStmt::Let { name, expr, .. }
            if name.text == "submitted" && &source[expr.start..expr.end] == "run.submit()"
    ));
}

/// A qualified `throws` spelling gets the bare-name diagnostic and resolves
/// to the last segment (the application-wide vocabulary's spelling).
#[test]
fn qualified_throws_spelling_gets_a_bare_name_diagnostic() {
    let source = concat!(
        "application A {\n",
        "    module m {\n",
        "        service S { boolean pay() throws billing.PaymentDeclined; }\n",
        "    }\n",
        "}\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.iter().any(|error| error
            .message
            .contains("`throws` entries are bare error names")),
        "expected the bare-name diagnostic, got: {:?}",
        result.errors
    );
    let module = &result.ast.unwrap().application.unwrap().modules[0];
    assert_eq!(
        module.services[0].operations[0].throws[0].text,
        "PaymentDeclined"
    );
}

#[test]
fn invariant_clause_parses_before_the_repository_block() {
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
        "    }\n",
        "}\n",
    );
    let result = parse_ddd(source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let design = &result.ast.expect("ast").application.expect("app").modules[0].designs[0];
    assert_eq!(design.invariants.len(), 2);
    assert_eq!(span_text(source, design.invariants[1]), "total >= 0");
}

#[test]
fn invariants_format_one_per_line_and_keep_the_expression() {
    let source = concat!(
        "import \"orders.mox\"\n",
        "application A {\n",
        "    base nz.example.orders\n",
        "    module m {\n",
        "        entity Order invariant total == lines.map(l => l.quantity * l.unitPrice).sum(); invariant total >= 0; repository OrderRepository { findById; }\n",
        "        value Money invariant cents >= 0;\n",
        "    }\n",
        "}\n",
    );
    let formatted = fmt_ddd(source);
    let expected = concat!(
        "import \"orders.mox\";\n",
        "\n",
        "application A {\n",
        "    base nz.example.orders\n",
        "    module m {\n",
        "        entity Order\n",
        "            invariant total == lines.map(l => l.quantity * l.unitPrice).sum();\n",
        "            invariant total >= 0;\n",
        "            repository OrderRepository {\n",
        "                findById;\n",
        "            }\n",
        "        value Money\n",
        "            invariant cents >= 0;\n",
        "    }\n",
        "}\n",
    );
    assert_eq!(formatted, expected);
    assert_eq!(fmt_ddd(&formatted), formatted, "formatting is a fixpoint");
}

#[test]
fn a_fragment_parses_top_level_modules_without_an_application() {
    let source = concat!(
        "import \"orders.mox\"\n",
        "module sales {\n",
        "    entity Order\n",
        "}\n",
    );
    let result = parse_ddd(source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let file = result.ast.expect("ast");
    assert!(file.application.is_none());
    assert_eq!(file.fragments.len(), 1);
    assert_eq!(file.fragments[0].name.text, "sales");
}

#[test]
fn a_fragment_formats_without_an_application() {
    let source = "import \"orders.mox\"\nmodule sales { entity Order }\n";
    let formatted = fmt_ddd(source);
    assert_eq!(
        formatted,
        "import \"orders.mox\";\n\nmodule sales {\n    entity Order\n}\n"
    );
    assert_eq!(fmt_ddd(&formatted), formatted);
}

#[test]
fn padded_application_parses() {
    let source = "   application Other { module m { entity Order } }\n";
    let result = parse_ddd(source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result.ast.expect("ast").application.is_some());
}

// --- DD9: typed repository queries -------------------------------------------

/// A repository query written with every clause (DD9): the read-only
/// projection `Order[] openFor(CustomerId, OrderStatus)` with its `where`
/// predicate, two `sort` keys (one explicit default `asc`), a `page` window,
/// and the guarding `capability` list.
const QUERY: &str = r#"
import "orders.mox"

application Shop {
    base nz.example.shop
    module orders {
        entity Order repository OrderRepository {
            findById;
            Order[] openFor(CustomerId customer, OrderStatus state) where (customer == self.customer and self.status == state) sort createdDate desc, id asc page limit 20 max 100 capability ReadOrders;
        }
    }
}
"#;

#[test]
fn repository_query_parses_every_clause() {
    let result = parse_ddd(QUERY);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let repository = expect_module(&file, "orders").designs[0]
        .repository
        .as_ref()
        .expect("repository");
    assert_eq!(repository.operations.len(), 2);
    assert_eq!(
        repository.operations[0].builtin,
        Some(DddBuiltinOp::FindById)
    );

    let op = &repository.operations[1];
    assert_eq!(op.name.text, "openFor");
    assert!(op.builtin.is_none());
    assert!(op.body.is_none());
    assert!(op.condition.is_none(), "a query takes no `when` guard");
    assert!(op.throws.is_empty());
    assert_eq!(
        op.return_type
            .as_ref()
            .expect("return type")
            .name
            .full_name(),
        "Order"
    );
    assert!(matches!(
        op.multiplicity.as_ref().expect("multiplicity").kind,
        MultiplicityKind::Unbounded
    ));
    assert_eq!(
        op.params
            .iter()
            .map(|param| param.name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["customer", "state"]
    );

    let query = op.query.as_ref().expect("the query clauses");
    assert_eq!(
        span_text(QUERY, query.predicate),
        "(customer == self.customer and self.status == state)",
        "the predicate keeps its parens and is sliced verbatim"
    );
    assert_eq!(
        span_text(QUERY, query.span),
        "where (customer == self.customer and self.status == state) \
         sort createdDate desc, id asc page limit 20 max 100"
    );

    // Sort keys in written order with their authored directions (`asc` is
    // recorded even though canonical output drops it).
    assert_eq!(query.sort.len(), 2);
    assert_eq!(query.sort[0].field.text, "createdDate");
    assert_eq!(query.sort[0].direction, Some(DddSortDirection::Desc));
    assert_eq!(span_text(QUERY, query.sort[0].span), "createdDate desc");
    assert_eq!(query.sort[1].field.text, "id");
    assert_eq!(query.sort[1].direction, Some(DddSortDirection::Asc));
    assert_eq!(span_text(QUERY, query.sort[1].span), "id asc");

    let page = query.page.as_ref().expect("the page window");
    assert_eq!(page.limit, Some(20));
    assert_eq!(page.max, Some(100));
    assert!(!page.cursor);
    assert_eq!(
        span_text(QUERY, page.span),
        "page limit 20 max 100",
        "the page span covers the clause keyword and its members"
    );

    assert_eq!(
        op.capabilities
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["ReadOrders"]
    );
    assert_eq!(
        span_text(QUERY, op.span),
        "Order[] openFor(CustomerId customer, OrderStatus state) where \
         (customer == self.customer and self.status == state) sort createdDate \
         desc, id asc page limit 20 max 100 capability ReadOrders;"
    );
}

/// `throws` stays grammatically legal before the query clauses (the driver
/// rejects it later); the query parses whole either way.
#[test]
fn repository_query_keeps_throws_before_the_query_clauses() {
    let source = r#"
application Shop {
    module orders {
        entity Order repository OrderRepository {
            Order[] openFor(CustomerId customer) throws NotFound where (customer == self.customer) capability ReadOrders;
        }
    }
}
"#;
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let op = &expect_module(&file, "orders").designs[0]
        .repository
        .as_ref()
        .expect("repository")
        .operations[0];
    assert_eq!(
        op.throws
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["NotFound"]
    );
    assert_eq!(
        span_text(source, op.query.as_ref().expect("query").predicate),
        "(customer == self.customer)"
    );
    assert_eq!(op.query.as_ref().unwrap().sort.len(), 0);
    assert!(op.query.as_ref().unwrap().page.is_none());
}

#[test]
fn repository_query_formats_canonically_and_drops_the_default_asc() {
    // The clauses are deliberately scrambled (the parser rejects that order,
    // the token-based formatter canonicalizes it) and the default `asc` is
    // written out; the canonical line is `where sort page capability` with
    // `asc` omitted.
    let source = concat!(
        "application Shop {\n",
        "module orders {\n",
        "entity Order repository OrderRepository {\n",
        "Order[] openFor(CustomerId customer, OrderStatus state) capability ReadOrders page limit 20 max 100 sort createdDate desc, id asc where (customer  ==  self.customer and self.status == state);\n",
        "}\n",
        "}\n",
        "}\n",
    );
    let expected = concat!(
        "application Shop {\n",
        "    module orders {\n",
        "        entity Order repository OrderRepository {\n",
        "            Order[] openFor(CustomerId customer, OrderStatus state) where (customer  ==  self.customer and self.status == state) sort createdDate desc, id page limit 20 max 100 capability ReadOrders;\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    let formatted = fmt_ddd(source);
    assert_eq!(formatted, expected);
    assert_eq!(fmt_ddd(&formatted), formatted, "formatting is a fixpoint");

    // The canonical text parses, and the predicate's interior spacing came
    // through untouched (it is a verbatim `raw_parens` slice, never
    // token-respaced), while `asc` is gone.
    assert!(!formatted.contains("id asc"));
    let result = parse_ddd(&formatted);
    assert!(
        result.errors.is_empty(),
        "the canonical query must parse: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let op = &expect_module(&file, "orders").designs[0]
        .repository
        .as_ref()
        .expect("repository")
        .operations[0];
    let query = op.query.as_ref().expect("the query clauses");
    assert_eq!(
        span_text(&formatted, query.predicate),
        "(customer  ==  self.customer and self.status == state)"
    );
    assert_eq!(query.sort.len(), 2);
    assert_eq!(query.sort[0].direction, Some(DddSortDirection::Desc));
    assert_eq!(query.sort[1].field.text, "id");
    assert_eq!(
        query.sort[1].direction, None,
        "the omitted `asc` re-parses as the absent default"
    );
    let page = query.page.as_ref().expect("the page window");
    assert_eq!(
        (page.limit, page.max, page.cursor),
        (Some(20), Some(100), false)
    );
    assert_eq!(
        op.capabilities
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["ReadOrders"]
    );
}

#[test]
fn repository_query_round_trips_through_parse_and_format() {
    let formatted = fmt_ddd(QUERY);
    let canonical = concat!(
        "import \"orders.mox\";\n",
        "\n",
        "application Shop {\n",
        "    base nz.example.shop\n",
        "    module orders {\n",
        "        entity Order repository OrderRepository {\n",
        "            findById;\n",
        "            Order[] openFor(CustomerId customer, OrderStatus state) where (customer == self.customer and self.status == state) sort createdDate desc, id page limit 20 max 100 capability ReadOrders;\n",
        "        }\n",
        "    }\n",
        "}\n",
    );
    assert_eq!(formatted, canonical);
    assert_eq!(fmt_ddd(canonical), canonical, "formatting is a fixpoint");
    let before = parse_ddd(canonical).ast.expect("the first AST");
    let after = parse_ddd(&formatted).ast.expect("the reparsed AST");
    assert_eq!(before, after, "parse -> format -> parse is the identity");
}

/// A query takes `where`, never `when`: the mismatch is a named parse error
/// (the operation itself survives with its query clauses).
#[test]
fn repository_query_with_when_is_a_named_error() {
    let source = concat!(
        "application A { module m {\n",
        "    entity E repository R {\n",
        "        E[] open() where (status == Open) when (owner == self.owner) capability Read;\n",
        "    }\n",
        "} }\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.iter().any(|error| error
            .message
            .contains("takes a `where` predicate, not a `when` guard")),
        "expected the named `when`-on-a-query error, got: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let op = &expect_module(&file, "m").designs[0]
        .repository
        .as_ref()
        .expect("repository")
        .operations[0];
    assert!(op.query.is_some(), "the query clauses survive the error");
    assert!(op.condition.is_none(), "the stray `when` is not a guard");
}

/// A signature followed by a `capability` clause and no query clause is not a
/// query: it keeps today's repository-operation rejection.
#[test]
fn repository_capability_without_query_clauses_is_still_rejected() {
    let source = concat!(
        "application A { module m {\n",
        "    entity E repository R {\n",
        "        findById;\n",
        "        E[] all() capability Read;\n",
        "    }\n",
        "} }\n",
    );
    let result = parse_ddd(source);
    assert!(
        result
            .errors
            .iter()
            .any(|error| error.message.contains("expected a repository operation")),
        "expected the repository-operation rejection, got: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let repository = expect_module(&file, "m").designs[0]
        .repository
        .as_ref()
        .expect("repository");
    assert_eq!(
        repository.operations.len(),
        1,
        "the capability-only operation is dropped, not narrowed into a query"
    );
    assert_eq!(
        repository.operations[0].builtin,
        Some(DddBuiltinOp::FindById)
    );
}

/// The query words are contextual: `where`, `sort`, and `page` stay ordinary
/// identifiers outside the clause positions (an operation name, a parameter
/// name, a sort key), and `^page` escapes anywhere.
#[test]
fn query_clause_words_are_contextual_identifiers() {
    let source = concat!(
        "application A { module m {\n",
        "    entity E repository R {\n",
        "        boolean where(String x);\n",
        "        boolean sort(String page);\n",
        "        boolean ^page(String where);\n",
        "        E[] bySort(String sort, int page) where (sort == \"asc\") sort sort desc, page capability Read;\n",
        "    }\n",
        "} }\n",
    );
    let result = parse_ddd(source);
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
    let file = result.ast.expect("expected an AST");
    let repository = expect_module(&file, "m").designs[0]
        .repository
        .as_ref()
        .expect("repository");
    assert_eq!(repository.operations.len(), 4);

    // Operations (and a parameter) merely named after the clause words.
    assert_eq!(repository.operations[0].name.text, "where");
    assert!(repository.operations[0].query.is_none());
    assert_eq!(repository.operations[1].name.text, "sort");
    assert_eq!(repository.operations[1].params[0].name.text, "page");
    assert!(repository.operations[1].query.is_none());
    assert_eq!(repository.operations[2].name.text, "page");
    assert_eq!(repository.operations[2].params[0].name.text, "where");
    assert!(repository.operations[2].query.is_none());

    let query_op = &repository.operations[3];
    let query = query_op.query.as_ref().expect("the query clauses");
    assert_eq!(span_text(source, query.predicate), "(sort == \"asc\")");
    assert_eq!(
        query
            .sort
            .iter()
            .map(|key| (key.field.text.as_str(), key.direction))
            .collect::<Vec<_>>(),
        vec![("sort", Some(DddSortDirection::Desc)), ("page", None)]
    );
    assert_eq!(
        query_op
            .capabilities
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Read"]
    );

    // Formatting preserves those identifiers and stays a fixpoint.
    let formatted = fmt_ddd(source);
    assert!(
        formatted.contains("sort sort desc, page capability Read;"),
        "the sort keys named after clause words survive, got:\n{formatted}"
    );
    assert_eq!(fmt_ddd(&formatted), formatted);
}
