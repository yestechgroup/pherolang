//! The DD1 resolved-identity companions (pherolang#298): every loose
//! reference the `.ddd` artifact carries keeps its authored spelling and
//! gains the identity the compiler resolved beside it. These tests pin the
//! companions against real compiler output — the fully resolved library
//! fixture (`tests/conformance/ddd/library.ddd` over `library.mox`, with
//! `library.actor` supplying the capability vocabulary) for the
//! cross-module identities, and inline designs for the `emits` and `invokes`
//! dimensions, whose buckets differ per entry point.
//!
//! The wire contract lives in `docs/DDD.md` (resolved companions): parallel
//! lists align index-for-index, `targetRef.qualifiedName` is
//! `<Application>.<module>.<Name>`, and an optional provenance member
//! (`declaredIn`, `contract`, `version`) stays absent exactly when its
//! counterpart artifact was not supplied.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use phero_driver::{
    compile_actors_str, compile_ddd_str, compile_ddd_str_with_actors,
    compile_ddd_str_with_decisions, DddCompilation, DomainImports,
};
use phero_ir::ddd::{
    CapabilityRef, DddModel, DecisionRef, EventRef, SearchDef, Service, ServiceOperation,
    TargetKind, TargetRef,
};
use phero_ir::dmn::{DecisionService, DmnModel, MetadataKind, ModelMetadata};
use phero_ir::{PrimitiveType, TypeRef};

/// The library fixture's application name: every qualified target identity
/// starts with it.
const APPLICATION: &str = "Library";

fn workspace_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../")
            .canonicalize()
            .expect("workspace root")
    })
}

fn fixture(relative: &str) -> PathBuf {
    workspace_root().join(relative)
}

fn read_fixture(relative: &str) -> String {
    std::fs::read_to_string(fixture(relative))
        .unwrap_or_else(|error| panic!("read fixture {relative}: {error}"))
}

/// The library domain, keyed by its canonical absolute path — the way a host
/// provides it. The design's relative `import "library.mox"` resolves to
/// this path lexically.
fn domain() -> (String, String) {
    let path = fixture("tests/conformance/ddd/library.mox");
    (
        path.to_str().expect("utf-8 path").to_string(),
        read_fixture("tests/conformance/ddd/library.mox"),
    )
}

/// A design path inside the fixture directory, so an inline design's relative
/// imports resolve against the same absolute domain path.
fn inline_design_path() -> String {
    fixture("tests/conformance/ddd/test.ddd")
        .to_str()
        .expect("utf-8 path")
        .to_string()
}

fn compile_design(source: &str, domains: &[(String, String)]) -> DddCompilation {
    compile_ddd_str(
        &inline_design_path(),
        source,
        domains,
        &DomainImports::default(),
    )
}

/// Compiles a design expected to be clean and returns its artifact.
fn artifact(compilation: DddCompilation) -> DddModel {
    assert!(
        compilation.diagnostics.is_empty(),
        "expected a clean compile: {:?}",
        compilation.diagnostics
    );
    compilation.model.expect("clean compile yields an artifact")
}

/// The canonical library design, compiled plain.
fn library() -> DddModel {
    artifact(compile_ddd_str(
        fixture("tests/conformance/ddd/library.ddd")
            .to_str()
            .expect("utf-8 path"),
        &read_fixture("tests/conformance/ddd/library.ddd"),
        &[domain()],
        &DomainImports::default(),
    ))
}

/// The library design, compiled with its `.actor` policy (`library.actor`,
/// whose block declares the fixture's three capabilities). The design is an
/// inline one over the same domain: `library.ddd` itself names capabilities
/// (`ManageCustomer`, `ApproveCustomer`) that `library.actor` does not
/// declare, so compiling that pair is deliberately an E-level unknown-
/// capability compile — the focused design keeps every name resolvable.
fn compile_with_library_actors(source: &str) -> DddModel {
    let domains = [domain()];
    let actors = compile_actors_str(
        fixture("tests/conformance/ddd/library.actor")
            .to_str()
            .expect("utf-8 path"),
        &read_fixture("tests/conformance/ddd/library.actor"),
        &domains,
        &DomainImports::default(),
    );
    assert!(
        actors.diagnostics.is_empty(),
        "the actor fixture must compile clean: {:?}",
        actors.diagnostics
    );
    let actor_model = actors.model.expect("actor artifact");
    artifact(compile_ddd_str_with_actors(
        &inline_design_path(),
        source,
        &domains,
        &DomainImports::default(),
        &[actor_model],
    ))
}

fn service<'a>(model: &'a DddModel, name: &str) -> &'a Service {
    model
        .modules
        .iter()
        .flat_map(|module| &module.services)
        .find(|service| service.name == name)
        .unwrap_or_else(|| panic!("service '{name}'"))
}

fn operation<'a>(service: &'a Service, name: &str) -> &'a ServiceOperation {
    service
        .operations
        .iter()
        .find(|operation| operation.name == name)
        .unwrap_or_else(|| panic!("operation '{name}' of service '{}'", service.name))
}

fn search<'a>(model: &'a DddModel, name: &str) -> &'a SearchDef {
    model
        .modules
        .iter()
        .flat_map(|module| &module.searches)
        .find(|search| search.name == name)
        .unwrap_or_else(|| panic!("search '{name}'"))
}

/// The resolved target of a delegating operation.
fn target_ref<'a>(service: &Service, operation: &'a ServiceOperation) -> &'a TargetRef {
    operation
        .delegation
        .as_ref()
        .unwrap_or_else(|| panic!("operation '{operation:?}' delegates"))
        .target_ref
        .as_ref()
        .unwrap_or_else(|| {
            panic!(
                "delegation '{}' of '{}' carries a resolved target",
                operation.name, service.name
            )
        })
}

// -- Classes and features ---------------------------------------------------

/// Every design carries the class its authored name resolved to.
#[test]
fn every_design_carries_its_resolved_class() {
    let model = library();
    let mut designed = 0;
    for design in model.modules.iter().flat_map(|module| &module.designs) {
        designed += 1;
        match design.type_ref.as_ref() {
            Some(TypeRef::Class { package, name }) => {
                let authored = design.class.as_deref().expect("a class-backed design");
                assert_eq!(package, "nz.example.library", "design '{authored}'");
                // The authored name is the class name, unqualified in this
                // fixture.
                assert_eq!(name, authored);
            }
            other => panic!(
                "design '{}' has {:?}",
                design.class.as_deref().unwrap_or_default(),
                other
            ),
        }
    }
    assert_eq!(designed, 7, "every library design was checked");
}

/// Search definitions carry the searched class and, per member, the
/// referenced feature's declared type.
#[test]
fn search_members_carry_their_resolved_feature_types() {
    let model = library();

    let media = search(&model, "MediaSearch");
    assert_eq!(
        media.entity_ref,
        Some(TypeRef::Class {
            package: "nz.example.library".to_string(),
            name: "Movie".to_string(),
        })
    );
    assert_eq!(
        media
            .text
            .iter()
            .map(|field| (field.property.clone(), field.type_ref.clone()))
            .collect::<Vec<_>>(),
        [
            (
                "title".to_string(),
                Some(TypeRef::Primitive(PrimitiveType::String))
            ),
            (
                "synopsis".to_string(),
                Some(TypeRef::Primitive(PrimitiveType::String))
            ),
        ]
    );
    assert_eq!(
        media.filters[0].type_ref,
        Some(TypeRef::Enum {
            package: "nz.example.library".to_string(),
            name: "MovieGenre".to_string(),
        })
    );
    assert_eq!(
        media.sort[0].type_ref,
        Some(TypeRef::Primitive(PrimitiveType::String))
    );

    let directory = search(&model, "MemberDirectory");
    assert_eq!(
        directory.entity_ref,
        Some(TypeRef::Class {
            package: "nz.example.library".to_string(),
            name: "Person".to_string(),
        })
    );
    assert_eq!(
        directory.filters[0].type_ref,
        Some(TypeRef::Primitive(PrimitiveType::Date))
    );
    assert_eq!(
        directory
            .sort
            .iter()
            .map(|sort| sort.type_ref.clone())
            .collect::<Vec<_>>(),
        [
            Some(TypeRef::Primitive(PrimitiveType::String)),
            Some(TypeRef::Primitive(PrimitiveType::Date)),
        ]
    );
}

// -- Services, repositories, searches --------------------------------------

/// Every delegation carries the resolved identity of its target: the kind,
/// the declaring module, and the `<Application>.<module>.<Name>` qualified
/// name the semantic graph uses.
#[test]
fn every_delegation_carries_its_resolved_target() {
    let model = library();
    let media = service(&model, "MediaService");

    // A repository of the service's own module.
    assert_eq!(
        target_ref(media, operation(media, "save")),
        &TargetRef::new(
            TargetKind::Repository,
            format!("{APPLICATION}.media.PhysicalMediaRepository"),
            "media",
        )
    );
    // A service in another module.
    assert_eq!(
        target_ref(media, operation(media, "registerBorrower")),
        &TargetRef::new(
            TargetKind::Service,
            format!("{APPLICATION}.person.PersonService"),
            "person",
        )
    );
    // A search of the service's own module.
    assert_eq!(
        target_ref(media, operation(media, "findMedia")),
        &TargetRef::new(
            TargetKind::Search,
            format!("{APPLICATION}.media.MediaSearch"),
            "media",
        )
    );

    let person = service(&model, "PersonService");
    for delegated in ["register", "findByName"] {
        assert_eq!(
            target_ref(person, operation(person, delegated)),
            &TargetRef::new(
                TargetKind::Repository,
                format!("{APPLICATION}.person.PersonRepository"),
                "person",
            )
        );
    }

    // The authored target stays exactly as written beside the companion.
    assert_eq!(
        operation(media, "findMedia")
            .delegation
            .as_ref()
            .map(|delegation| delegation.target.as_str()),
        Some("MediaSearch")
    );
}

/// `dependencyRefs` aligns index-for-index with `dependencies`, each entry the
/// identical target identity a delegation to the same name would record.
#[test]
fn dependency_refs_align_with_the_injected_names() {
    let model = library();
    let media = service(&model, "MediaService");
    assert_eq!(
        media.dependencies,
        [
            "PhysicalMediaRepository",
            "MovieRepository",
            "PersonService",
            "MediaSearch"
        ]
    );
    assert_eq!(
        media.dependency_refs,
        [
            TargetRef::new(
                TargetKind::Repository,
                format!("{APPLICATION}.media.PhysicalMediaRepository"),
                "media",
            ),
            TargetRef::new(
                TargetKind::Repository,
                format!("{APPLICATION}.media.MovieRepository"),
                "media",
            ),
            TargetRef::new(
                TargetKind::Service,
                format!("{APPLICATION}.person.PersonService"),
                "person",
            ),
            TargetRef::new(
                TargetKind::Search,
                format!("{APPLICATION}.media.MediaSearch"),
                "media",
            ),
        ]
    );

    let person = service(&model, "PersonService");
    assert_eq!(person.dependencies, ["PersonRepository"]);
    assert_eq!(
        person.dependency_refs,
        [TargetRef::new(
            TargetKind::Repository,
            format!("{APPLICATION}.person.PersonRepository"),
            "person",
        )]
    );

    // A plain compile supplies no decision models, so the decision companion
    // is name-only.
    assert!(media.decision_dependencies.is_empty());
    assert!(media.decision_dependency_refs.is_empty());
}

// -- Capabilities -----------------------------------------------------------

/// Without an `.actor` bucket the capability companions record the names
/// only: `declaredIn` is absent (unresolved by design, not an error).
#[test]
fn capability_refs_stay_name_only_on_a_plain_compile() {
    let model = library();
    let media = service(&model, "MediaService");
    let borrow = operation(media, "borrow");
    assert_eq!(borrow.capabilities, ["BorrowMedia"]);
    assert_eq!(borrow.capability_refs, [CapabilityRef::new("BorrowMedia")]);
    assert!(borrow.capability_refs[0].declared_in.is_none());

    let projections = search(&model, "MediaSearch");
    assert_eq!(
        projections.capabilities,
        ["SearchMedia", "SearchMediaAdmin"]
    );
    assert_eq!(
        projections.capability_refs,
        [
            CapabilityRef::new("SearchMedia"),
            CapabilityRef::new("SearchMediaAdmin")
        ]
    );
    assert!(projections
        .capability_refs
        .iter()
        .all(|reference| reference.declared_in.is_none()));
}

/// With the actor policy supplied, each capability companion names the block
/// that declares it — operations and searches alike.
#[test]
fn capability_refs_name_the_declaring_block_with_actors() {
    let design = r#"
import "library.mox";

application Library {
    base nz.example.library

    module media {
        service MediaService {
            boolean borrow() capability BorrowMedia;
        }
        entity Movie repository MovieRepository {
            findById;
            save;
        }
        search MediaSearch {
            entity Movie
            text {
                title
            }
            filters {
                genre
            }
            sort {
                title
            }
            capability SearchMedia, SearchMediaAdmin
        }
    }
}
"#;
    let model = compile_with_library_actors(design);
    let media = service(&model, "MediaService");
    assert_eq!(
        operation(media, "borrow").capability_refs,
        [CapabilityRef::new("BorrowMedia").with_declared_in(Some("Library".to_string()))]
    );

    let projections = search(&model, "MediaSearch");
    assert_eq!(
        projections.capability_refs,
        [
            CapabilityRef::new("SearchMedia").with_declared_in(Some("Library".to_string())),
            CapabilityRef::new("SearchMediaAdmin").with_declared_in(Some("Library".to_string())),
        ]
    );
}

/// A capability declared by several `.actor` blocks resolves to the first
/// block in artifact order (the deterministic spelling the contract fixes).
#[test]
fn capability_refs_use_the_first_declaring_block() {
    let design = r#"
import "library.mox";

application Library {
    base nz.example.library

    module media {
        service MediaService {
            boolean borrow() capability BorrowMedia;
        }
    }
}
"#;
    let actors = [
        phero_ir::ActorModel::new().block(phero_ir::ActorsDef::new("First").capability(
            phero_ir::CapabilityDef::new(
                "BorrowMedia",
                TypeRef::Class {
                    package: "nz.example.library".to_string(),
                    name: "Media".to_string(),
                },
            ),
        )),
        phero_ir::ActorModel::new().block(phero_ir::ActorsDef::new("Second").capability(
            phero_ir::CapabilityDef::new(
                "BorrowMedia",
                TypeRef::Class {
                    package: "nz.example.library".to_string(),
                    name: "Media".to_string(),
                },
            ),
        )),
    ];
    let model = artifact(compile_ddd_str_with_actors(
        &inline_design_path(),
        design,
        &[domain()],
        &DomainImports::default(),
        &actors,
    ));
    assert_eq!(
        operation(service(&model, "MediaService"), "borrow").capability_refs,
        [CapabilityRef::new("BorrowMedia").with_declared_in(Some("First".to_string()))]
    );
}

// -- Events -----------------------------------------------------------------

/// `emits` mirrors into `emittedRefs` with no contract: the `.ddd` compile
/// has no `.evt` bucket, so event references are unresolved by design.
#[test]
fn emitted_refs_mirror_emits_without_a_contract() {
    let design = r#"
import "library.mox";

application Library {
    base nz.example.library

    module media {
        service LendingService {
            boolean approve() emits BookLent, BookReturned;
            boolean renew() emits BookLent;
        }
    }
}
"#;
    let model = artifact(compile_design(design, &[domain()]));
    let lending = service(&model, "LendingService");
    let approve = operation(lending, "approve");
    assert_eq!(approve.emits, ["BookLent", "BookReturned"]);
    assert_eq!(
        approve.emitted_refs,
        [EventRef::new("BookLent"), EventRef::new("BookReturned")]
    );
    assert!(approve
        .emitted_refs
        .iter()
        .all(|reference| reference.contract.is_none()));
    assert_eq!(
        operation(lending, "renew").emitted_refs,
        [EventRef::new("BookLent")]
    );
}

// -- Decisions --------------------------------------------------------------

/// The decision fixture's design: one injected decision service and one
/// operation invoking two (a bare name and a package-qualified one).
const DECISION_DESIGN: &str = r#"
import "library.mox";
import "decisions.dmn";

application Library {
    base nz.example.library

    module lending {
        service LoanService {
            inject decision underwriting.RiskScore;
            boolean approve(Book book) invokes RiskScore, underwriting.FraudScreen;
        }
    }
}
"#;

fn decision_model(services: &[&str], version: Option<&str>) -> DmnModel {
    let mut model = DmnModel::new();
    for name in services {
        model.services.push(DecisionService {
            name: (*name).to_string(),
            description: None,
            inputs: Vec::new(),
            outputs: Vec::new(),
            encapsulates: Vec::new(),
        });
    }
    if let Some(version) = version {
        model.metadata.push(ModelMetadata {
            kind: MetadataKind::Version,
            value: version.to_string(),
        });
    }
    model
}

/// Without a decision bucket every decision reference is name-only — both on
/// operations (`invokes`) and on services (`inject decision`).
#[test]
fn decision_refs_are_name_only_without_decisions() {
    let model = artifact(compile_design(DECISION_DESIGN, &[domain()]));
    let lending = service(&model, "LoanService");

    assert_eq!(lending.decision_dependencies, ["RiskScore"]);
    assert_eq!(
        lending.decision_dependency_refs,
        [DecisionRef::new("RiskScore")]
    );

    let approve = operation(lending, "approve");
    assert_eq!(approve.invokes, ["RiskScore", "FraudScreen"]);
    assert_eq!(
        approve.decision_refs,
        [
            DecisionRef::new("RiskScore"),
            DecisionRef::new("FraudScreen")
        ]
    );
    assert!(approve
        .decision_refs
        .iter()
        .all(|reference| reference.contract.is_none() && reference.version.is_none()));
}

/// With the decisions supplied, each reference names the providing artifact
/// and the version that artifact declares — the resolved identity, not the
/// authored spelling.
#[test]
fn decision_refs_resolve_through_the_decisions_bucket() {
    let decisions = [(
        "decisions.dmn".to_string(),
        decision_model(&["RiskScore", "FraudScreen"], Some("1.4.2")),
    )];
    let model = artifact(compile_ddd_str_with_decisions(
        &inline_design_path(),
        DECISION_DESIGN,
        &[domain()],
        &DomainImports::default(),
        &[],
        &decisions,
    ));
    let lending = service(&model, "LoanService");

    let resolved = |service: &str| {
        DecisionRef::new(service)
            .with_contract(Some("decisions.dmn".to_string()))
            .with_version(Some("1.4.2".to_string()))
    };
    assert_eq!(
        lending.decision_dependency_refs,
        [resolved("RiskScore")],
        "the inject line resolves too"
    );
    assert_eq!(
        operation(lending, "approve").decision_refs,
        [resolved("RiskScore"), resolved("FraudScreen")]
    );
}

/// A pinned reference resolves through its authored pin, and the companion's
/// `version` is the **resolved** model's declared version.
#[test]
fn pinned_decision_refs_record_the_resolved_version() {
    let design = r#"
import "library.mox";

application Library {
    base nz.example.library

    module lending {
        service LoanService {
            boolean approve() invokes RiskScore version compatible;
        }
    }
}
"#;
    let decisions = [
        (
            "risk-v1.dmn".to_string(),
            decision_model(&["RiskScore"], Some("1.0.0")),
        ),
        (
            "risk-v2.dmn".to_string(),
            decision_model(&["RiskScore"], Some("2.1.0")),
        ),
    ];
    let model = artifact(compile_ddd_str_with_decisions(
        &inline_design_path(),
        design,
        &[domain()],
        &DomainImports::default(),
        &[],
        &decisions,
    ));
    let approve = operation(service(&model, "LoanService"), "approve");
    assert_eq!(
        approve.decision_refs,
        [DecisionRef::new("RiskScore")
            .with_contract(Some("risk-v2.dmn".to_string()))
            .with_version(Some("2.1.0".to_string()))]
    );
}
