//! DD8 (issue pherolang#305): multi-file design applications.

use phero_driver::{compile_ddd_app_str, DddFragment, DomainImports};

const DOMAIN: &str = "package demo\n\nclass Order { String orderNo }\n";

fn compile(root: &str, fragments: &[(&str, &str)]) -> phero_driver::DddCompilation {
    let fragments: Vec<DddFragment> = fragments
        .iter()
        .map(|(path, source)| DddFragment {
            path: (*path).to_string(),
            source: (*source).to_string(),
        })
        .collect();
    compile_ddd_app_str(
        "app.ddd",
        root,
        &fragments,
        &[("demo.mox".to_string(), DOMAIN.to_string())],
        &DomainImports::default(),
    )
}

#[test]
fn fragments_merge_into_one_application() {
    let root = concat!(
        "import \"demo.mox\"\n",
        "import \"sales.ddd\"\n",
        "import \"billing.ddd\"\n",
        "application Shop {\n",
        "    base demo\n",
        "    module core {\n",
        "        service CoreService { inject OrderRepository; save => OrderRepository.save; }\n",
        "        entity Order repository OrderRepository { findById; save; }\n",
        "    }\n",
        "}\n",
    );
    let sales = concat!(
        "module sales {\n",
        "    service SalesService {\n",
        "        boolean sell(String orderNo);\n",
        "    }\n",
        "}\n",
    );
    let billing = concat!(
        "module billing {\n",
        "    service BillingService {\n",
        "        boolean bill(String orderNo);\n",
        "    }\n",
        "}\n",
    );
    let compilation = compile(root, &[("sales.ddd", sales), ("billing.ddd", billing)]);
    assert!(
        compilation.diagnostics.is_empty(),
        "{:?}",
        compilation.diagnostics
    );
    let model = compilation.model.expect("model");
    assert_eq!(
        model
            .modules
            .iter()
            .map(|module| module.name.as_str())
            .collect::<Vec<_>>(),
        ["core", "sales", "billing"]
    );
}

#[test]
fn a_module_reopens_across_fragments() {
    let root = concat!(
        "import \"demo.mox\"\n",
        "import \"extra.ddd\"\n",
        "application Shop {\n",
        "    base demo\n",
        "    module core {\n",
        "        service CoreService { inject OrderRepository; save => OrderRepository.save; }\n",
        "        entity Order repository OrderRepository { findById; save; }\n",
        "    }\n",
        "}\n",
    );
    let extra = concat!(
        "module core {\n",
        "    service ExtraService { inject OrderRepository; touch => OrderRepository.save; }\n",
        "}\n",
    );
    let compilation = compile(root, &[("extra.ddd", extra)]);
    assert!(
        compilation.diagnostics.is_empty(),
        "{:?}",
        compilation.diagnostics
    );
    let model = compilation.model.expect("model");
    assert_eq!(model.modules.len(), 1);
    assert_eq!(model.modules[0].services.len(), 2);
}

#[test]
fn fragment_diagnostics_are_tagged_with_the_fragment_path() {
    let root = concat!(
        "import \"demo.mox\"\n",
        "import \"broken.ddd\"\n",
        "application Shop {\n",
        "    base demo\n",
        "    module core {\n",
        "        entity Order repository OrderRepository { findById; }\n",
        "    }\n",
        "}\n",
    );
    let broken = "module extra {\n    entity Missing\n}\n";
    let compilation = compile(root, &[("broken.ddd", broken)]);
    assert!(compilation.model.is_none());
    assert!(
        compilation
            .diagnostics
            .iter()
            .any(|(path, diagnostic)| path == "broken.ddd"
                && diagnostic.message.contains("unknown type 'Missing'")),
        "{:?}",
        compilation.diagnostics
    );
}

#[test]
fn a_root_cannot_be_imported_as_a_fragment() {
    let root = concat!(
        "import \"demo.mox\"\n",
        "import \"other.ddd\"\n",
        "application Shop {\n",
        "    base demo\n",
        "    module core { entity Order }\n",
        "}\n",
    );
    let other = "application Other { module m { entity Order } }\n";
    let compilation = compile(root, &[("other.ddd", other)]);
    assert!(compilation.model.is_none(), "{:?}", compilation.diagnostics);
    assert!(
        compilation
            .diagnostics
            .iter()
            .any(|(path, diagnostic)| path == "other.ddd"
                && diagnostic
                    .message
                    .contains("cannot be imported as a fragment")),
        "{:?}",
        compilation.diagnostics
    );
}

#[test]
fn a_fragment_root_is_refused_standalone() {
    let compilation = compile("module core { entity Order }\n", &[]);
    assert!(compilation.model.is_none());
    assert!(
        compilation
            .diagnostics
            .iter()
            .any(|(_, diagnostic)| diagnostic.message.contains("design fragment")),
        "{:?}",
        compilation.diagnostics
    );
}

// --- fragment import origins and downstream importers -------------------------

const ORDER_DOMAIN: &str = "package demo\n\nclass Order { String orderNo }\n";
const PRODUCT_DOMAIN: &str = "package shop\n\nclass Product { String sku }\n";

const FRAGMENTED_ROOT: &str = concat!(
    "import \"demo.mox\"\n",
    "import \"sub/catalogue.ddd\"\n",
    "application Shop {\n",
    "    base demo\n",
    "    module core { entity Order }\n",
    "}\n",
);

const FRAGMENT_WITH_DOMAIN: &str = concat!(
    "import \"product.mox\"\n",
    "\n",
    "module catalogue {\n",
    "    entity Product repository ProductRepository { findById; }\n",
    "}\n",
);

fn fragment_fetch(key: &str) -> Option<String> {
    // A relative root path (`app.ddd`) resolves imports under `./`.
    let key = key.strip_prefix("./").unwrap_or(key);
    match key {
        "sub/catalogue.ddd" => Some(FRAGMENT_WITH_DOMAIN.to_string()),
        "sub/product.mox" => Some(PRODUCT_DOMAIN.to_string()),
        "demo.mox" => Some(ORDER_DOMAIN.to_string()),
        "app.ddd" => Some(FRAGMENTED_ROOT.to_string()),
        _ => None,
    }
}

#[test]
fn a_fragment_resolves_its_own_domain_import_relative_to_itself() {
    // `product.mox` lives next to the fragment in `sub/`, not next to the
    // root: an import resolves against the file that declares it (DD8).
    let fragments = [DddFragment {
        path: "sub/catalogue.ddd".to_string(),
        source: FRAGMENT_WITH_DOMAIN.to_string(),
    }];
    let domains = [
        ("demo.mox".to_string(), ORDER_DOMAIN.to_string()),
        ("sub/product.mox".to_string(), PRODUCT_DOMAIN.to_string()),
    ];
    let compilation = compile_ddd_app_str(
        "app.ddd",
        FRAGMENTED_ROOT,
        &fragments,
        &domains,
        &DomainImports::default(),
    );
    assert!(
        compilation.diagnostics.is_empty(),
        "a fragment's own import must resolve from its own directory: {:?}",
        compilation.diagnostics
    );
    let model = compilation.model.expect("model");
    let catalogue = model
        .modules
        .iter()
        .find(|module| module.name == "catalogue")
        .expect("the fragment module merges");
    assert_eq!(catalogue.designs.len(), 1);
    assert_eq!(catalogue.designs[0].class.as_deref(), Some("Product"));
}

#[test]
fn the_app_gatherer_collects_fragment_domains() {
    let mut fetch = fragment_fetch;
    let gathered =
        phero_driver::project::gather_ddd_app_imports("app.ddd", FRAGMENTED_ROOT, &mut fetch);
    assert!(gathered.missing.is_empty(), "{:?}", gathered.missing);
    assert!(
        gathered.missing_domains.is_empty(),
        "{:?}",
        gathered.missing_domains
    );
    assert!(
        gathered
            .domains
            .iter()
            .any(|(key, _)| key.ends_with("sub/product.mox")),
        "the fragment's own domain is gathered: {:?}",
        gathered
            .domains
            .iter()
            .map(|(key, _)| key)
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_use_case_importing_a_fragmented_design_sees_every_module() {
    let mut fetch = fragment_fetch;
    let buckets = phero_driver::project::gather_usecase_imports(
        "use.uc",
        "",
        |_| vec!["app.ddd".to_string()],
        &mut fetch,
    )
    .expect("the design app gathers");
    assert_eq!(buckets.designs.len(), 1);
    let modules: Vec<&str> = buckets.designs[0]
        .1
        .modules
        .iter()
        .map(|module| module.name.as_str())
        .collect();
    assert!(
        modules.contains(&"catalogue"),
        "a downstream importer compiles the whole application: {modules:?}"
    );
}
