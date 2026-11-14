//! The crate's exported diagnostic constants stay in lockstep with the
//! central registry (`phero-diagnostics`): each band's exports must equal
//! the registry's band codes in both directions, so a code can never
//! exist on one side only.

use std::collections::BTreeSet;

use phero_diagnostics::{all, ifml_check, ifml_resolver};

fn band_codes(band: &str) -> BTreeSet<&'static str> {
    all()
        .iter()
        .filter(|entry| entry.band == band)
        .map(|entry| entry.code)
        .collect()
}

#[test]
fn exported_resolver_codes_match_the_registry() {
    let exported = BTreeSet::from([
        phero_ifml::E_IMPORT_NOT_PROVIDED,
        phero_ifml::E_PARSE,
        phero_ifml::E_CIRCULAR,
        phero_ifml::E_AMBIGUOUS_MODULE,
        phero_ifml::E_DUPLICATE_MODULE,
        phero_ifml::E_UNKNOWN_MODULE,
        phero_ifml::E_UNKNOWN_INPUT,
        phero_ifml::E_MISSING_INPUT,
        phero_ifml::E_INPUT_TYPE,
    ]);
    assert_eq!(exported, band_codes(ifml_resolver::RESOLVER_BAND));
}

#[test]
fn exported_checker_codes_match_the_registry() {
    let exported = BTreeSet::from([
        phero_ifml::ifml_check::E_UNKNOWN_ENTITY,
        phero_ifml::ifml_check::E_TYPE_MISMATCH,
        phero_ifml::ifml_check::E_UNKNOWN_FEATURE,
        phero_ifml::ifml_check::E_UNKNOWN_VARIABLE,
        phero_ifml::ifml_check::E_UNKNOWN_ACTION,
        phero_ifml::ifml_check::E_UNKNOWN_VIEW,
        phero_ifml::ifml_check::E_UNKNOWN_COMPONENT,
        phero_ifml::ifml_check::E_UNKNOWN_CAPABILITY,
        phero_ifml::ifml_check::E_UNKNOWN_ROLE,
        phero_ifml::ifml_check::E_UNKNOWN_ACTION_TARGET,
        phero_ifml::ifml_check::E_EMPTY_DECISION_SERVICE,
        phero_ifml::ifml_check::E_AMBIGUOUS_DECISION_SERVICE,
    ]);
    assert_eq!(exported, band_codes(ifml_check::CHECK_BAND));
}
