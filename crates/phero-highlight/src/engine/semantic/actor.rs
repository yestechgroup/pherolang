//! The `.actor` semantic walker: standalone policy files reuse the inline
//! `actors` block walker from the `.mox` surface.

use phero_syntax::ast::ActorFile;

use crate::engine::semantic::{mox, Ctx};

/// Walks a parsed `.actor` file.
pub(crate) fn walk(ctx: &mut Ctx, file: &ActorFile) {
    for block in &file.blocks {
        mox::walk_actors(ctx, block);
    }
}
