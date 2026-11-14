//! Disk-facing import collection, shared with the LSP: the
//! implementation lives in [`phero_driver::workspace_imports`] (the driver
//! core stays filesystem-free; this module is its host-side bridge).

pub(crate) use phero_driver::workspace_imports::{
    collect_schema_imports, collect_sigil_imports, SigilSources,
};
