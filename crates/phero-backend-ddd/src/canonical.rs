//! Canonical serialization for the `.ddd` artifact.
//!
//! The wire format is owned by [`phero_ir::ddd`]; this module only pins a
//! key order so consumers can hash and diff artifacts deterministically
//! without depending on which `serde_json` map feature is active in their
//! dependency graph. It goes through
//! [`phero_backend_common::canonical_key_order`], the shared implementation
//! every backend uses.

use phero_backend_common::canonical_key_order;
use phero_ir::ddd::DddModel;
use phero_ir::IrError;

/// Serializes the artifact and re-inserts every object key in alphabetical
/// order (recursively), returning the canonical JSON value.
///
/// Deserializing the value still yields the same [`DddModel`]: key order is
/// not part of the contract, only the bytes a consumer hashes are.
pub fn canonical_json(model: &DddModel) -> Result<serde_json::Value, IrError> {
    let value = serde_json::to_value(model)?;
    Ok(canonical_key_order(value))
}

/// [`canonical_json`] rendered as a compact JSON string.
pub fn canonical_json_string(model: &DddModel) -> Result<String, IrError> {
    Ok(serde_json::to_string(&canonical_json(model)?)?)
}
