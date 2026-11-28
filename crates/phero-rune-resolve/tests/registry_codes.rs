//! Every diagnostic code the Rune toolchain emits is a `phero_diagnostics`
//! registry constant (ADR 0012 D5, RN8): no `sigil-*` source may spell a
//! code as a string literal, so a code can never drift from the registry or
//! collide with another band again.

use std::path::{Path, PathBuf};

/// Literals the guard tolerates. Empty: the unreachable `W0001` skip went
/// with the scannerless parser (RN9), so no `sigil-*` source spells a code.
const ALLOWED: &[(&str, &str)] = &[];

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates directory")
        .to_path_buf()
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("readable source dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Every `"E0123"`/`"W0123"` string literal in `text`.
fn code_literals(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    for start in 0..bytes.len().saturating_sub(6) {
        let window = &bytes[start..start + 7];
        if window[0] == b'"'
            && (window[1] == b'E' || window[1] == b'W')
            && window[2..6].iter().all(u8::is_ascii_digit)
            && window[6] == b'"'
        {
            found.push(String::from_utf8_lossy(&window[1..6]).into_owned());
        }
    }
    found
}

#[test]
fn no_sigil_source_spells_a_diagnostic_code_as_a_literal() {
    let crates = crates_dir();
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&crates).expect("readable crates dir") {
        let krate = entry.expect("dir entry").path();
        let name = krate.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("sigil-") || !krate.join("src").is_dir() {
            continue;
        }
        let mut files = Vec::new();
        rust_files(&krate.join("src"), &mut files);
        for file in files {
            let text = std::fs::read_to_string(&file).expect("readable source");
            let relative = file
                .strip_prefix(&crates)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            for code in code_literals(&text) {
                let allowed = ALLOWED
                    .iter()
                    .any(|(path, allowed)| relative == *path && code == *allowed);
                if !allowed {
                    offenders.push(format!("{relative}: \"{code}\""));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "use the phero_diagnostics rune.* constants, never a code literal:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn every_rune_constant_is_a_registered_rune_entry() {
    for code in [
        phero_diagnostics::E_RUNE_SYNTAX,
        phero_diagnostics::E_RUNE_UNKNOWN_SYMBOL,
        phero_diagnostics::E_RUNE_UNKNOWN_ANNOTATION,
        phero_diagnostics::E_RUNE_NO_SUCH_ANNOTATION_ATTRIBUTE,
        phero_diagnostics::E_RUNE_DUPLICATE_DEFINITION,
        phero_diagnostics::E_RUNE_INHERITANCE_CYCLE,
        phero_diagnostics::E_RUNE_WRONG_KIND,
        phero_diagnostics::E_RUNE_UNKNOWN_MEMBER,
        phero_diagnostics::W_RUNE_DEPRECATED_REFERENCE,
    ] {
        let entry =
            phero_diagnostics::lookup(code).unwrap_or_else(|| panic!("{code} is not registered"));
        assert!(
            entry.band.starts_with("rune."),
            "{code} belongs to {}, not a rune band",
            entry.band
        );
    }
}
