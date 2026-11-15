//! Collection-kind schema emission (DD6, issue #303): a `set` keeps the
//! `uniqueItems: true` keyword a unique list already carries. JSON Schema has
//! no standard "unordered" keyword, so the schema records duplicate-freedom
//! only and the semantic kind lives in the artifact.

use phero_backend_jsonschema::{generate, Profile};
use phero_driver::compile_str;

const MODEL: &str = r#"package demo

class Person { String name }

class Line { String sku }

class Play {
    String[] tags { set }
    String[] labels { unique }
    String[] notes
    refers Person[] playedBy { set }
    contains Line[] lines { set }
}
"#;

fn definitions(profile: Profile) -> serde_json::Value {
    let compilation = compile_str(
        "collections.mox",
        MODEL,
        &phero_driver::DomainImports::default(),
    );
    assert!(
        compilation.diagnostics.is_empty(),
        "model must compile cleanly: {:?}",
        compilation.diagnostics
    );
    let model = compilation.model.expect("model lowered");
    let files = generate(&model, profile).expect("generate");
    let json = files.get("schema.json").expect("schema.json");
    serde_json::from_str::<serde_json::Value>(json).expect("valid JSON")["$defs"].clone()
}

#[test]
fn a_set_emits_unique_items_on_attributes_references_and_containments() {
    for profile in [Profile::Wire, Profile::Api] {
        let defs = definitions(profile);
        let play = &defs["Play"]["properties"];
        for property in ["tags", "playedBy", "lines"] {
            let schema = &play[property];
            assert_eq!(
                schema["type"].as_str(),
                Some("array"),
                "{property} stays an array ({profile:?})"
            );
            assert_eq!(
                schema["uniqueItems"],
                serde_json::json!(true),
                "{property} is duplicate-free ({profile:?})"
            );
        }
    }
}

#[test]
fn a_unique_list_and_a_plain_list_keep_their_shapes() {
    for profile in [Profile::Wire, Profile::Api] {
        let defs = definitions(profile);
        let play = &defs["Play"]["properties"];
        assert_eq!(
            play["labels"]["uniqueItems"],
            serde_json::json!(true),
            "a unique list emits uniqueItems ({profile:?})"
        );
        assert!(
            play["notes"].get("uniqueItems").is_none(),
            "a plain list emits no duplicate rule ({profile:?})"
        );
    }
}

/// The schema never invents a keyword for unorderedness: the set differs from
/// the unique list in the artifact alone.
#[test]
fn no_unordered_keyword_is_invented() {
    for profile in [Profile::Wire, Profile::Api] {
        let serialized = definitions(profile).to_string();
        for invented in ["unordered", "\"set\"", "collectionKind", "ordered"] {
            assert!(
                !serialized.contains(invented),
                "schema must not carry {invented:?} ({profile:?}): {serialized}"
            );
        }
    }
}
