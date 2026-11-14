# phero-ir

The Core IR (resolved metamodel) for [pherolang](https://github.com/yestechgroup/pherolang):
an Ecore-like structural [`Model`] that code-generation backends consume, plus the
standalone [`ActorModel`] authorization-policy artifact. The IR is fully *resolved* —
every type reference is a qualified `TypeRef`, never an unresolved name.

The serialized artifact is a **stable, versioned wire format** (camelCase, adjacent
tagging, a `formatVersion` gate). The wire-format contract is normative in the
crate docs (`cargo doc -p phero-ir --open`).

Backends live out of tree: consume `phero_ir::Model` / `ActorModel` — or read the
serialized artifact — and emit anything. The backend-author guide, including the
semver/`formatVersion` policy and the wire-format gotchas, is
[docs/BACKENDS.md](https://github.com/yestechgroup/pherolang/blob/main/docs/BACKENDS.md).

[`Model`]: https://docs.rs/phero-ir/latest/phero_ir/struct.Model.html
[`ActorModel`]: https://docs.rs/phero-ir/latest/phero_ir/struct.ActorModel.html
