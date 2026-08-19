# Branch notes — `nojaf`

This branch collects local work on top of `GDQuest/GDScript-formatter`. It is a
fork branch, kept as one piece so the work can be read in context. It is not a
pull request, and nothing here is meant to land as-is.

I wrote this file for the maintainers. It states what each change does, why the
formatter is the right place for it, and what it costs. The table below marks
which changes are worth offering upstream.

Branch point: `71f44db`, the tip of `upstream/main` at the time.

## Disclosure: this was written by an AI agent

Claude (Anthropic's coding agent) wrote the design work and documentation on this
branch. Every commit carries a `Co-Authored-By: Claude` trailer.

My role is product owner, not implementer. I say what I want, supply the problems
from my own projects that motivate each change, set constraints, reject designs I
do not like, and approve what ships. I do not write the implementation.

I am stating this plainly because it changes how you should review it. Treat this
as code from a contributor whose reasoning you cannot interrogate directly. Each
design decision is documented so you can judge it on the merits rather than on
trust.

You can also decide that this project does not accept AI-written code. That is a
reasonable position. I would rather hear it now than after a pull request.

## What is here

| Change | Status | Offer upstream |
|--------|--------|----------------|
| `docs/specification_index.md`: design for an `index` sub-command | written | Ask first |
| `gdscript-formatter index`: the sub-command itself | implemented | Ask first |

The design document came first and the code follows it. Where implementing it
moved a decision, the document says so at the end rather than quietly matching
the code.

## Why an `index` sub-command

I maintain a fork of a separate project, a GDScript static analyser that runs
inside Godot as an editor addon:
<https://github.com/nojaf/godot-gdscript-linter/tree/nojaf>

That analyser and this formatter have opposite strengths. Godot gives the analyser
type information with no source positions. Tree-sitter gives this formatter source
positions with no type information. Several useful checks need both halves, and
the analyser currently fakes the syntactic half with regular expressions over
lines. Every bug found in it so far came from that half.

The full argument, the record shapes and the implementation plan are in
`docs/specification_index.md`.

What it costs this project: one sub-command, `src/index.rs` with a collector per
record kind under `src/index/collectors/`, two node kinds added to
`src/node_kind.rs` (`attribute_subscript` and `variadic_parameter`, both of
which the formatter previously left as `Other`), and a `SourceRange` struct with
a `get_range` helper next to the linter's existing `get_line_column`. It reuses
the parser and the linter's node kind lookup, and it touches neither
`formatter.rs` nor `renderer.rs`. No new dependencies.

Whether this belongs upstream is a real question rather than a rhetorical one. It
is driven by one consumer, and a sub-command that serves one external tool may not
be something GDQuest wants to carry. It is also plausibly useful to anyone building
tooling on GDScript, which is why it is written as a general index rather than as
the exact set of facts my analyser happens to need today. I would rather ask than
assume.

## Working agreement

- Work on the `nojaf` branch, never on `main`. `main` tracks `upstream/main` so
  it stays a clean mirror of GDQuest.
- `origin` is the fork, `upstream` is GDQuest.
- Follow `docs/rust_coding_guidelines.md` as written, including avoiding external
  dependencies. Local additions should be indistinguishable in style from the
  surrounding code, so that anything offered upstream is easy to review.
- Keep this file current. It is the entry point for anyone asking what the fork
  changes.
