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
