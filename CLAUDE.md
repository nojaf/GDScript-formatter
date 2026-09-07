# GDScript Formatter - Claude Context

A fork of `GDQuest/GDScript-formatter` on branch `nojaf`. `main` mirrors
upstream and is never worked on. `BRANCH_NOTES.md` says what the fork adds and
the working agreement. Follow `docs/rust_coding_guidelines.md`: plain loops,
early returns, no external dependencies, code indistinguishable in style from
upstream's.

## The index sub-command

`gdscript-formatter index` writes a JSON Lines index of GDScript source for one
consumer: the linter at `../godot-gdscript-linter` (branch `nojaf`), which reads
it through `addons/gdscript-linter/analyzer/source-index.gd`. That linter gets
types and scenes from the Godot engine and gets positions and structure from
here; it must never fall back to scanning source text itself.

`docs/specification_index.md` is the contract: record shapes, the `scope` and
`context` values, the schema policy, and every requirement the consumer has
filed together with what was done about it.

| Path | What |
|------|------|
| `src/index.rs` | the walk, project-root and `res://` path resolution, JSON writing |
| `src/index/collectors.rs` | the registry and the helpers collectors share, such as `find_argument_position` and `find_expression_context` |
| `src/index/collectors/<kind>.rs` | one collector per record kind |
| `src/index/tests.rs` | exact-output tests, and a corpus test holding grammar node counts equal to record counts |
| `src/node_kind.rs` | maps tree-sitter node names to `GDScriptNodeKind` |
| `Cargo.toml` | pins the `tree-sitter-gdscript` grammar; its `grammar.js` in the cargo checkout says what children a node has |

## Adding a record or a field

1. Write the requirement in the spec under "Requirements from the first
   consumer", and answer it with a `> **Done.**` note once it ships.
2. A collector under `src/index/collectors/`, with `string_literal.rs` as the
   template; register it in `collectors.rs`.
3. An exact-output test in `tests.rs`, and an assertion in the corpus test tying
   the grammar node to the record so a missed form fails the build.
4. A row in the spec's "Shape changes made under schema 1" table. Adding a
   record kind or an optional field is not breaking. Renaming, moving, removing
   or redefining a field is, and bumps `INDEX_SCHEMA_VERSION` in `src/index.rs`.
5. `cargo test --lib index`, `cargo clippy --bin gdscript-formatter`, then
   `cargo build --release --bin gdscript-formatter`. The linter's test suite
   rebuilds this binary itself and runs its fixtures against it.

Then consume it on the linter side: a bucket in `source-index.gd`, and the
check that wanted it.
