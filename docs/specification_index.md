# GDScript formatter: Specification for the `index` sub-command

This document defines a sub-command that emits a machine-readable index of
GDScript source: declarations, references, member chains, string literals,
comparisons and comments, each with a source range.

Status: implemented. `gdscript-formatter index` ships the records below.
Where the implementation departs from this document as first drafted, the
section "What implementing it changed" at the end says so and why.

This work lives on the `nojaf` fork branch. See `BRANCH_NOTES.md` for what this
fork changes and why.

## Why this exists

The consumer is a separate static analyser that runs inside Godot as an editor
addon. That analyser has the opposite strengths of this formatter:

- Godot gives it **meaning without positions**. Through `ClassDB` and the script
  reflection API it knows every member of a class including inherited ones, the
  declared type of each property, signal arity, method arity with default and
  vararg counts, which exports can hold null, which methods are engine virtuals,
  and whether a script compiles at all. It knows none of this by line number.
- Tree-sitter gives this formatter **positions without meaning**. It knows exactly
  where each declaration, call, annotation and string sits, and nothing about
  `Label` having a `text` property.

Checks such as "this member does not exist on that type" need both halves. Today
the analyser fakes the syntactic half with regular expressions over lines, and
every bug found in it so far came from that half:

- Declarations carrying an annotation on the same line were skipped entirely.
  `@abstract func may_target(candidate: Node) -> bool` never registered, and its
  text still counted as a reference, so implementations looked alive. Adding
  `@abstract` to a project silently exempted those methods.
- String literals counted as references wherever they appeared, so
  `print("all done")` kept a function named `all` alive.
- Local variables were collected per file rather than per scope, so one
  `var target` in an unrelated function disabled a check for `target` everywhere.
- Inner classes are invisible. A guard inside `class Inner:` `_ready()` satisfies
  the outer script's check.

None of these are fixable by better regular expressions. They need a parser, and
this project already has one.

## Non-goals

- **Not a tree dump.** A consumer written in GDScript would flatten a nested tree
  into these same records immediately, and walking a large nested structure in
  GDScript is slow. Records are the useful form.
- **No type inference.** Tree-sitter cannot resolve `var x := get_thing()`. The
  consumer has the engine for that.
- **No cross-file resolution.** Each file is indexed independently. Joining files
  is the consumer's job.
- **No formatting concerns.** This sub-command must not reformat, rewrite or
  validate style.

## Command

```
gdscript-formatter index [OPTIONS] [FILES]...

Arguments:
  <FILES>...  GDScript files or directories to index. If empty, uses the current
              directory. Reads from stdin when piped.

Options:
  -x, --exclude <PATH>  Exclude one file or directory (repeatable)
  -h, --help            Print help
```

One invocation handles the whole project. The consumer must not spawn a process
per file.

Output goes to stdout. Diagnostics go to stderr, so stdout stays parseable.

The `path` in each `file` header is a `res://` path when the file sits inside a
Godot project, found by looking for `project.godot` in the directories above it.
The consumer matches records against scripts it loaded from the engine, and the
engine knows nothing but `res://` paths. Outside a project the path is reported
as given. Input piped on stdin is reported as `<stdin>`.

## Output format

JSON Lines. One JSON object per line, no trailing commas, no enclosing array.
This lets the consumer parse incrementally and keeps any single line small enough
for `JSON.parse_string` in GDScript.

Each file produces a header record followed by its content records:

```jsonl
{"record": "file", "schema": 1, "path": "res://hud/hud.gd"}
{"record": "declaration", ...}
{"record": "member_chain", ...}
```

Fields that would carry nothing are left out rather than written as `null`, `[]`
or `false`. A missing `annotations` means there were none, a missing `is_call`
means it is not a call. This is the first of the two size mitigations under
"Decisions taken", and it is why the elided example further down still shows
them: that example predates the decision.

### Ranges

Every record carries a `range`, and records that name something also carry a
`name_range` covering the identifier alone. A range is an explicit struct rather
than a tuple, per `docs/rust_coding_guidelines.md`:

```json
"range": {"start_row": 7, "start_column": 2, "end_row": 7, "end_column": 19,
          "start_byte": 142, "end_byte": 159}
```

Rows and columns are **one-based**, matching `get_line_column` in
`src/linter/lib.rs`, so the index agrees with what `gdscript-formatter lint`
already prints. Columns are byte offsets within the line plus one, which is what
tree-sitter's `Point::column` gives and what the existing linter already reports.

Byte offsets are included as well, because they are the only unambiguous way to
slice the original source, and because a consumer that needs character columns
(SARIF counts characters, not bytes) can compute them from the source and the
offsets. GDScript sources regularly contain non-ASCII text in comments and
strings, so byte and character columns do diverge in real files. Emitting the
repo's existing convention and letting the consumer convert keeps one convention
in this codebase rather than two.

End positions are new. The existing linter only needs a start, but a consumer that
underlines a member chain or measures an argument list needs both ends.

### `scope`

Every record carries a `scope` string naming the enclosing declarations, dot
separated, empty at file level:

```
""                  file level
"_process"          inside func _process
"Inner"             inside class Inner, at class level
"Inner._ready"      inside Inner's _ready
```

This is what makes per-scope shadowing and inner classes work.

## Records

### `declaration`

| Field | Notes |
|-------|-------|
| `kind` | `function`, `variable`, `constant`, `signal`, `enum`, `enum_member`, `class`, `parameter` |
| `name` | identifier |
| `range` | whole declaration including annotations |
| `name_range` | the identifier only |
| `scope` | as above |
| `annotations` | array of `{"name": "export", "arguments": ["0", "10"], "range": {...}}`, empty when none |
| `modifiers` | subset of `static`, `abstract` |
| `parameters` | functions only: array of `{"name", "type", "default", "range"}` |
| `type` | declared type as written, or `null` |
| `default` | default value source text, or `null` |
| `body_range` | functions only, or `null` for an abstract declaration with no body |
| `body_is_pass_only` | functions only |

`annotations` and `modifiers` are the fields that fix the `@abstract` class of
bug. The consumer never has to recognise annotation syntax again.

### `reference`

A bare identifier used as a value or called directly. Emitted for **every**
identifier, including locals, parameters and loop variables.

This is deliberate. Combined with `scope` and the `declaration` records, it lets a
consumer resolve whether an occurrence of `target` refers to a local or to a
member, instead of counting occurrences and hoping. The analyser currently cannot
do that, and it is why one `var target` in an unrelated function switched a check
off for `target` across a whole file.

| Field | Notes |
|-------|-------|
| `name`, `range`, `scope` | |
| `is_call` | true when followed by an argument list |
| `arguments` | array of `{"text", "range"}`, source text trimmed |
| `context` | see below |

### `member_chain`

Attribute access, with or without a leading `self`.

| Field | Notes |
|-------|-------|
| `segments` | array of `{"name", "range"}`, in order |
| `range` | whole chain |
| `scope` | |
| `is_call` | true when the last segment is called |
| `arguments` | as above |
| `context` | see below |
| `base` | `{"text", "range"}` when the chain starts from something that is not a name, else absent |

`base` covers `$Clock` in `$Clock.text` and `get_tree()` in
`get_tree().paused`. Without it, a chain whose root cannot be named would arrive
as a bare list of segments and look like it started at a name. A consumer
resolving the chain has to know it cannot start.

Per-segment ranges matter. `self.clock.ziggy` should be reported at `ziggy`, not
at the start of the line.

### `string_literal`

| Field | Notes |
|-------|-------|
| `value` | decoded contents, escapes resolved |
| `range`, `scope` | |
| `argument_of` | `{"callee": "call", "index": 0}` when the literal is an argument, else absent |
| `literal_kind` | `string_name` or `node_path`, absent for a plain string |

`&"late_bound"` is a method name as much as `"late_bound"` is, so `StringName`
and `NodePath` literals are emitted here too. `literal_kind` is what tells them
apart, and it is absent for the common case.

`argument_of` is how a consumer tells `self.call("late_bound")` apart from
`print("all done")`. One is a reference to a method, the other is prose.

### `comparison`

| Field | Notes |
|-------|-------|
| `operator` | `==`, `!=`, `<`, `<=`, `>`, `>=`, `in`, `is`, `not in`, `is not` |
| `left`, `right` | `{"text", "range"}` |
| `range`, `scope` | |

Emitted for every comparison, not only those against `null`. The existing
`comparison-with-itself` lint rule wants the same data, so the record earns its
place beyond one consumer.

### `comment`

| Field | Notes |
|-------|-------|
| `text` | comment text including the leading `#` |
| `range`, `scope` | |
| `is_documentation` | true for `##` |
| `is_trailing` | true when code precedes it on the same line |

Emitted so a consumer never has to read the source file itself. The analyser
suppresses findings with directives written as comments, such as
`# gdlint:ignore-next-line:unknown-member`, and binding a directive to the right
declaration is exactly the kind of line anchoring that has broken before. With
comment ranges and declaration ranges in the same stream, binding becomes an
interval comparison rather than a guess.

### `context` values

`statement`, `condition`, `assignment_target`, `assignment_value`, `argument`,
`return_value`, `type`, `other`.

`statement` is the one that unlocks new checks: an expression whose context is
`statement` and which is not a call does nothing at runtime.

`type` marks a name written as a type rather than used as a value: the `Label`
in `var clock: Label`, the `int` in `Array[int]`, the `CanvasLayer` in
`extends CanvasLayer`. Every identifier is emitted, so these appear whether or
not a consumer wants them; `type` is how it decides.

The context is read from where the expression sits, not from the statement
around it. In `return a + b`, the operands are `other` and the addition is the
`return_value`. Widening it would make `statement` mean less than it does.

## Worked example

Input:

```gdscript
class_name Hud
extends CanvasLayer

@onready var clock: Label = $Clock

func _process(_delta: float) -> void:
	self.clock.ziggy = "x"
	self.call("late_bound")
```

Output, elided for readability:

```jsonl
{"record":"file","schema":1,"path":"res://hud/hud.gd"}
{"record":"declaration","kind":"class","name":"Hud","scope":"","range":{...},"name_range":{...},"annotations":[],"modifiers":[]}
{"record":"declaration","kind":"variable","name":"clock","scope":"","type":"Label","default":"$Clock","annotations":[{"name":"onready","arguments":[],"range":{...}}],"modifiers":[],"range":{...},"name_range":{...}}
{"record":"declaration","kind":"function","name":"_process","scope":"","parameters":[{"name":"_delta","type":"float","default":null,"range":{...}}],"modifiers":[],"annotations":[],"body_range":{...},"body_is_pass_only":false,"range":{...},"name_range":{...}}
{"record":"member_chain","segments":[{"name":"self","range":{...}},{"name":"clock","range":{...}},{"name":"ziggy","range":{...}}],"scope":"_process","is_call":false,"context":"assignment_target","range":{...}}
{"record":"member_chain","segments":[{"name":"self","range":{...}},{"name":"call","range":{...}}],"scope":"_process","is_call":true,"arguments":[{"text":"\"late_bound\"","range":{...}}],"context":"statement","range":{...}}
{"record":"string_literal","value":"late_bound","scope":"_process","argument_of":{"callee":"call","index":0},"range":{...}}
```

## Schema versioning and failure

The `schema` field starts at 1 and increments on any breaking change to record
shapes or field meanings. Adding a new record kind or an optional field is not
breaking.

Consumers must refuse a schema they do not know and exit non-zero.

This matters more than it looks. On the consumer side, three separate mistakes
have already produced an empty report and exit code 0, which is indistinguishable
from a clean project: a stale script class cache, a type inference error, and an
over-generous rule. Adding a second process across a versioned interface creates
two more ways to fail silently, so:

- The sub-command exits non-zero when it cannot parse a file, and names the file
  on stderr.
- A file that fails to parse still emits its `file` header, with
  `"parse_error": true`, so a consumer can tell "no records because the file is
  broken" from "no records because nothing matched".
- The sub-command never emits a partial line.

## Implementation notes

Follow `docs/rust_coding_guidelines.md`. In particular: hand-write the JSON
serialisation rather than adding `serde`, use explicit structs for every record,
plain loops over iterator chains, and early returns over nesting.

Reuse what the linter already has rather than building a second walker:

- `src/linter.rs` walks the tree exactly once with a visitor and dispatches to
  rules by target node kind. The index wants the same shape: a set of collectors,
  each declaring the node kinds it cares about, each turning a node into records.
  Model the collector trait on `Rule` in `src/linter/rules.rs`, which pairs
  `get_target_ast_nodes` with `check_node`.
- `src/linter/lib.rs` already has `get_line_column` and `get_node_text`. Use them,
  and add a `get_range` helper beside them that returns the full struct described
  above. Every record should build its range through that one function so the
  convention lives in a single place.
- Match on `GDScriptNodeKind` from `src/node_kind.rs` rather than comparing kind
  strings. Any node kind missing from that enum should be added there.
- Reuse the parse entry point in `src/parser.rs`. The index must not depend on
  `src/formatter.rs` or `src/renderer.rs`, since it neither formats nor renders.

Layout, mirroring the linter:

```
src/index.rs              command entry, walk, JSON writing
src/index/collectors.rs   the collector trait and registry
src/index/collectors/     one file per record kind
src/index/tests.rs        fixtures and assertions
```

Add `Index` to the sub-command enum in `src/cli.rs` beside `Lint`, with a
`HELP_INDEX` constant next to `HELP_LINTER`.

Collectors hold no state, so the registry stores plain function pointers in a
`CollectorDefinition` table rather than the `Box<dyn Rule>` the linter uses.
That keeps the pairing of target node kinds with a collect function and still
follows the guideline against trait objects.

Tests should assert exact ranges, not just record counts. Ranges are the part most
likely to drift when the grammar updates, and a wrong range is invisible in any
test that only checks names.

## Decisions taken

These were open while drafting and are settled, because the analyser is the
consumer and its needs answer them.

**Emit every identifier, including locals and parameters.** The alternative was to
emit only "interesting" references, which sounds smaller and is worse. Resolution
needs the whole picture: to know that `target` on line 40 is the local declared on
line 38 rather than the member declared on line 8, a consumer needs both the
declaration and the occurrence, with scopes. Filtering at the producer would leave
the consumer counting again.

This is the field most likely to make the output large. Two mitigations, in order:
omit empty and null fields rather than writing `"annotations": []`, and if
measurement on a real project shows a problem, add a flag to narrow what is
emitted. Do not add the flag before measuring.

**One output format.** JSON Lines only, and no `--format` flag. A flag with one
legal value is noise, and a compact binary format is speculation until something
is measured as too slow. It can be added when there is a number to point at.

**Emit comments.** The initial draft said no consumer needed them. That was wrong.
The analyser reads comments for suppression directives, and directive binding is
precisely where line anchoring has broken before. Emitting comments means the
consumer never opens the source file at all, which is the point: one parser, one
source of truth about where things are.

## What implementing it changed

Everything above was written before the code. These are the places where writing
it moved something, each with the case that moved it.

**Scopes.** Functions, inner classes, enums and signals open a scope; lambdas do
not. Enum members would otherwise look like declarations one level up, and a
signal's parameters would look like file level variables. Lambdas are
expressions and half of them have no name, so giving only the named ones a scope
segment would be harder to reason about than leaving their parameters in the
enclosing scope. A lambda parameter that shadows an enclosing local is the one
case this gets wrong, and it is the reason to revisit the choice if it bites.

**`for` bindings are declarations.** The list of `kind` values has no entry for a
loop binding and the grammar has no declaration node for one, but per-scope
resolution needs it exactly as much as it needs a `var`. `for item in ...` emits
a `variable` declaration named `item`, ranged over the binding alone.

**Parameters are recorded twice, on purpose.** Once in the `parameters` array of
the function that owns them, because that is how a consumer reads a signature,
and once as their own `parameter` declaration records, because that is how it
resolves a name to a parameter rather than to a member that shares it.

**`_init` has no name in the grammar.** It is a keyword, so the constructor is
emitted as a `function` named `_init` with the keyword's range as its
`name_range`.

**Types are references.** "Emit every identifier" was meant to keep resolution
possible, and type positions are identifiers. They are emitted with
`context: "type"` rather than dropped, which is also how `extends CanvasLayer`
stays visible without a record kind of its own.

**Size, measured.** Indexing this repository's 110 test fixtures produces 2858
records from roughly 60 KB of GDScript, and the JSON Lines output is about
fourteen times the size of the source. Ranges are most of it: six numbers each,
at least two per record. The longest single line is 1.2 KB, comfortably inside
what `JSON.parse_string` handles. That is not a problem worth a flag yet, which
is what "Decisions taken" asked for before adding one.

**Not covered.** `match` patterns that bind names (`var x` inside a pattern) do
not produce declarations, and `get = _get_y` setter and getter references do not
produce `reference` records. Both are small gaps rather than design choices; add
them when a consumer needs them.
