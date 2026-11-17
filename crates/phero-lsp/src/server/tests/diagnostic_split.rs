//! LSP server tests: the diagnostic code/severity/message split.
//!
//! Characterization (SK2, epic pherolang#328, issue pherolang#330): both
//! the `.ifml` publish path and the driver-backed `.mox` path publish the
//! stable code as the wire `code` field and the human message **without**
//! a code prefix. The shared-diagnostic-record refactor is
//! behavior-preserving, so this split — and the publish order — must not
//! move.

use super::*;

use std::str::FromStr;

/// One resolver error (`E0006`) and one lint warning (`W0150`) in the same
/// flow: the publish order is the resolver's findings first, then the
/// checker's passes.
const SPLIT_IFML: &str = r#"view "Home" {
    use "Missing" as gone { };
}

action "Orphan" {
    target: Item;
}
"#;

fn ifml_uri(name: &str) -> Uri {
    Uri::from_str(&format!("file:///workspace/{name}")).unwrap()
}

#[tokio::test]
async fn ifml_publish_keeps_the_code_severity_and_message_split() {
    let (mut service, mut socket) = initialized_service().await;
    open_at(&mut service, &ifml_uri("split.ifml"), SPLIT_IFML).await;

    let publish = next_publish(&mut socket, &ifml_uri("split.ifml"))
        .await
        .expect("publishDiagnostics for the split flow");
    let shapes: Vec<(Option<NumberOrString>, Option<DiagnosticSeverity>, &str)> = publish
        .diagnostics
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.code.clone(),
                diagnostic.severity,
                diagnostic.message.as_str(),
            )
        })
        .collect();
    assert_eq!(
        shapes,
        vec![
            (
                Some(NumberOrString::String("E0006".to_string())),
                Some(DiagnosticSeverity::ERROR),
                "unknown module 'Missing'",
            ),
            (
                Some(NumberOrString::String("W0150".to_string())),
                Some(DiagnosticSeverity::WARNING),
                "action 'Orphan' is declared but never invoked",
            ),
        ],
        "{publish:#?}"
    );
    for diagnostic in &publish.diagnostics {
        assert_eq!(diagnostic.source, Some("pherolang".to_string()));
        assert_eq!(
            diagnostic.code_description, None,
            "the stable code string rides in `code` alone"
        );
        assert_eq!(
            diagnostic.data, None,
            "a registry code carries no structured payload"
        );
    }
}

#[tokio::test]
async fn driver_publish_keeps_the_code_severity_and_message_split() {
    let (mut service, mut socket) = initialized_service().await;
    open(&mut service, BROKEN).await;

    let publish = next_publish(&mut socket, &uri())
        .await
        .expect("publishDiagnostics for the domain document");
    assert_eq!(publish.diagnostics.len(), 1, "one error on {BROKEN:?}");
    let diagnostic = &publish.diagnostics[0];
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
    assert_eq!(diagnostic.source, Some("pherolang".to_string()));
    assert_eq!(
        diagnostic.code,
        Some(NumberOrString::String("attributeWithClassType".to_string())),
        "the payload-carrying code travels as its camelCase name"
    );
    assert_eq!(
        diagnostic.message,
        "feature 'oops' has class type 'Book'\nhint: did you mean `contains Book[..] oops` \
         or `refers Book[..] oops`?",
        "the message and help text stay one string; no code prefix"
    );
    let data = diagnostic.data.as_ref().expect("structured code data");
    assert_eq!(data["attributeWithClassType"]["feature"], "oops");
    assert_eq!(data["attributeWithClassType"]["class"], "Book");
}
