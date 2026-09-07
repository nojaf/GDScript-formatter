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
  -x, --exclude <PATH>      Exclude one file or directory (repeatable)
      --project-root <PATH> Directory the res:// paths are relative to
  -h, --help                Print help
```

One invocation handles the whole project. The consumer must not spawn a process
per file.

Output goes to stdout. Diagnostics go to stderr, so stdout stays parseable.

### Paths

The `path` in each `file` header is a `res://` path relative to the project root.
The consumer matches records against scripts it loaded from the engine, and the
engine knows nothing but `res://` paths.

The root comes from `--project-root` when given. Otherwise it is discovered by
walking up from each input file looking for `project.godot`.

**A run never mixes the two forms.** Either every path is a `res://` path or
every path is an absolute file system path. A run that half-resolved would join
nothing on the consumer side and report no findings, which reads exactly like a
clean project, so every case that would mix them is an error naming its fix
rather than a quiet fallback:

- Two different `project.godot` files above the inputs: error, pass
  `--project-root`.
- Some inputs inside a project and some outside it: error, pass
  `--project-root` or index them separately.
- An input outside an explicit `--project-root`: error.

When no project is found at all, every path is absolute and a warning says so on
stderr. Absolute rather than as-given, so the same file indexes under the same
name whatever directory the command ran from.

Input piped on stdin is reported as `<stdin>`.

## Output format

JSON Lines. One JSON object per line, no trailing commas, no enclosing array.
This lets the consumer parse incrementally and keeps any single line small enough
for `JSON.parse_string` in GDScript.

Each file produces a header record followed by its content records:

```jsonl
{"record": "file", "schema": 1, "path": "res://hud/hud.gd", "extends": "CanvasLayer"}
{"record": "declaration", ...}
{"record": "member_chain", ...}
```

The header carries `extends`, what the file's own script extends, as written: a
class name or a `"res://path.gd"` string with its quotes. It sits here as well as
on the `class` declaration because most scripts have no `class_name`, and a
consumer working out which script broke first needs the base of every script
rather than only of the named ones. Absent when the file extends nothing.

Annotations that no declaration claims get their own record, described below.

### Absent means empty, never unknown

Fields that would carry nothing are left out rather than written as `null`, `[]`
or `false`. This is the first of the two size mitigations under "Decisions
taken", and it is why the elided example further down still shows them: that
example predates the decision.

Absence is a guarantee, not an admission. The sub-command never omits a field
because it could not work something out. Specifically:

- An absent `annotations` or `modifiers` means there were none.
- An absent `parameters` means zero parameters. Signal arity checking depends on
  this, so it is the guarantee that matters most.
- An absent `is_call`, `is_trailing`, `is_documentation` or `body_is_pass_only`
  means false.
- An absent `type` or `default` means none was written.
- An absent `argument_of` means the record is not an argument.

The one exception is `body_range`, which every `function` declaration reports
either as a range or as an explicit `null`. It has to separate "has no body" from
"is not a function", and absence cannot carry both.

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
| `body_range` | functions only, a range or an explicit `null` when there is no body |
| `body_is_pass_only` | functions only |
| `extends` | classes only: the base as written, a class name or a `"res://path.gd"` string |
| `is_file_class` | classes only: `true` on the file's own `class_name`, absent on inner classes |

`annotations` and `modifiers` are the fields that fix the `@abstract` class of
bug. The consumer never has to recognise annotation syntax again. They cover both
spellings: an annotation written on the same line as the declaration and one
written on the line above it. The grammar attaches the first and leaves the
second as a sibling, and both apply in Godot, so both land on the record.
`range` covers them either way.

`extends` is on the class record and on the `file` header, and reads the same in
both. `class_name Foo extends Bar` and `class_name Foo` above `extends Bar` mean
the same thing and report the same thing.

An inner class reports its own base or none at all. It never inherits the file's:
`class Helper:` inside a file that says `extends Node` reports no `extends`,
because nothing was written and the implicit base is `RefCounted` rather than
`Node`.

`is_file_class` separates the file's own class from an inner class, which
otherwise look alike: both are `kind: "class"` at file scope. Exactly one record
per file carries it, and a file with no `class_name` has none, so absent means
"an inner class, or a file that declares no class" rather than "unknown".
Position cannot stand in for it, because the first class in a file is the file's
own only when the file has a `class_name` at all.

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
| `argument_of` | as on `string_literal` |

### `member_chain`

Attribute access, with or without a leading `self`.

| Field | Notes |
|-------|-------|
| `segments` | array, in order, see below |
| `range` | whole chain |
| `scope` | |
| `is_call` | true when the last segment is called |
| `arguments` | as above |
| `context` | see below |
| `argument_of` | as on `string_literal` |

Every hop is a segment and every segment says what it is:

| Field | Notes |
|-------|-------|
| `kind` | `identifier`, `self`, `call`, `node_path`, `subscript`, `other` |
| `name` | the member name, when the segment has one |
| `text` | the segment as written, for segments with no name |
| `is_call` | true for a called segment |
| `range` | the segment alone |

A flat list of names loses what a consumer cannot recover. `$Clock.text` would
arrive as a lone `text` and look like a member of the enclosing script.
`self.get_thing().field` would make `get_thing` look like a property, so `field`
gets checked against the wrong type. Both produce findings that are wrong rather
than missing, which is the direction that costs a user trust.

A consumer walking types stops at the first segment that is not `self` or
`identifier`. `kind` is not resolution: it is an honest signal that the chain
left the ground where hop-by-hop resolution is valid.

`super` is reported as `other` rather than as an identifier. Resolving through it
means resolving against the base class, which this index does not know, and
calling it an identifier would invite exactly the wrong lookup.

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

`reference` and `member_chain` carry `argument_of` too, so a nested call such as
`assert(is_instance_valid(thing))` reports each level directly instead of making
the consumer re-parse the `arguments` text of the level above it.

### `node_path`

| Field | Notes |
|-------|-------|
| `path` | the path as Godot reads it: `Panel/Button` for `$Panel/Button`, decoded for `$"Panel/With Space"` |
| `unique` | true for the `%Name` form, absent otherwise |
| `range`, `scope`, `context` | |

One record per `$Path` or `%Name` expression, wherever it sits. A `$Clock` at
the head of a member chain was already visible as a `node_path` segment, and a
bare one was visible nowhere: `self.button = $Panel/Button`, `add_child($X)`
and the value of an `@onready` variable produced no record. Those are the lines
a scene check most needs, because they are where the node is fetched, so every
form is a record here and the chain segment stays as it was.

`get_node("Panel/Button")` is not this record. It is a `string_literal` with
`argument_of` naming `get_node`, which already says everything a consumer needs.

### `comparison`

| Field | Notes |
|-------|-------|
| `operator` | `==`, `!=`, `<`, `<=`, `>`, `>=`, `in`, `is`, `not in`, `is not` |
| `left`, `right` | `{"text", "range"}` |
| `range`, `scope` | |

Emitted for every comparison, not only those against `null`. The existing
`comparison-with-itself` lint rule wants the same data, so the record earns its
place beyond one consumer.

### `annotation`

An annotation that belongs to no declaration.

| Field | Notes |
|-------|-------|
| `name` | the annotation name without the `@` |
| `arguments` | array of source text, absent when there are none |
| `range`, `scope` | |

Most annotations belong to a declaration and are reported in its `annotations`
field. The rest have nothing to attach to: `@tool` above a bare `extends`, or
`@warning_ignore(...)` above a statement inside a `match`. Between this record
and the `annotations` field, every annotation in a file is reported exactly once,
and a test over the fixture corpus holds that to be true.

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

`condition` unlocks another: a method named but not called in a truth test is a
`Callable`, which is always true, so the branch never varies. It is reported for
every expression whose value is tested for truth, not only for the whole
condition of a statement:

- the condition of `if`, `elif`, `while` and of a ternary
- both operands of `and` and `or`, including outside a condition, as in
  `var ready := self.a and self.b`, because the value is still tested for truth
- the operand of `not`

The `not` of `is not` and `not in` does not count. Those spell a comparison, not
a boolean operator, and their operands are `other`.

Parentheses do not change a context. `if (self.predicate):` reports the same
thing `if self.predicate:` does, because parentheses change how an expression is
written and not what it does.

`type` marks a name written as a type rather than used as a value: the `Label`
in `var clock: Label`, the `int` in `Array[int]`, the `CanvasLayer` in
`extends CanvasLayer`. Every identifier is emitted, so these appear whether or
not a consumer wants them; `type` is how it decides.

The context is read from where the expression sits, not from the statement
around it. In `return a + b`, the operands are `other` and the addition is the
`return_value`. Widening it would make `statement` mean less than it does.

**This set is open.** Values may be added as the index learns to tell more
positions apart. Treat an unrecognised value as `other` rather than failing, so
that adding one is not a breaking change. `type` was itself added this way.

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
{"record":"node_path","path":"Clock","scope":"","range":{...},"context":"assignment_value"}
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

**Breaking the shape freely is fine. Breaking it quietly is not.** There is one
consumer, written alongside this sub-command, and shapes should change whenever
its experience says they are wrong. Nobody should build compatibility shims for
older shapes. What that freedom does not extend to is leaving the number alone
while the meaning moves, because the consumer's guard is the only thing between
an incompatible producer and a silently wrong answer, and it cannot fire if the
number never moves. A consumer built against an older shape reads a field that
moved as absent, absent means empty rather than unknown, so it reports fewer
findings and exits 0: indistinguishable from a clean project.

So: rename, move, remove or redefine a field, and bump. Add a record kind or an
optional field, and do not.

### While both sides are in development

That reasoning bites once someone is running a build they cannot rebuild at will.
Right now nobody is: this producer and its one consumer are written by two people
in direct contact, neither is in production, and both are updated together. So
shapes have changed under version 1 on purpose, and the number has stayed at 1 on
purpose.

This stops being the arrangement the moment any of these is true, and the number
starts moving then:

- the consumer ships to anyone who cannot rebuild it on demand
- either side gets a tagged release that promises a stable interface
- a second consumer appears

Until then, record every shape change below. Not for version negotiation, but so
that a build which turns out to be older than expected can be diagnosed instead
of guessed at.

### Shape changes made under schema 1

| Change | Breaking |
|--------|----------|
| Initial release. | — |
| `member_chain.segments` gained `kind` and `is_call`, and carry `name` or `text` rather than always `name`. `member_chain.base` removed. | yes |
| `body_range` is an explicit `null` on a function with no body rather than absent. | yes |
| `--project-root`, and a run refuses to mix `res://` paths with file system paths. | behaviour |
| `argument_of` on `reference` and `member_chain`. `extends` on `class` declarations and the `file` header. New `annotation` record. | no |
| `context` reports `condition` for operands of `and`, `or` and `not`, and reads through parentheses. | values |
| Constructors and own-line annotations are reported at all, having been dropped. | no, but output grows |
| `is_file_class` on the file's own `class` declaration. | no |
| An inner class reports its own `extends` or none, having reported the file's. | no, but a wrong value becomes right |
| New `node_path` record for every `$Path` and `%Name` expression. | no |

The exact-output tests in `src/index/tests.rs` fail on any change to any record,
which is the moment to add a row here and ask whether the change needs a bump. A
second test pins the version so that updating those fixtures alone is not enough
to let a shape change through unrecorded.

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
`name_range`. This paragraph described the intent for a while before it described
the behaviour; see "What the corpus audit found" below.

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

## Requirements from the first consumer

Written after trying the first implementation against a real project. Ordered by
importance. The first two are correctness problems, the rest are contracts I need
stated so I can rely on them.

**All five are implemented.** Each one below carries a note saying what shipped
and where the answer differs from the request. Schema stayed at 1: see "Schema
versioning and failure" for why nothing here needed a bump.

### 1. Segments must say what they are

A member chain is currently a flat list of names, which loses information the
consumer cannot recover:

```gdscript
self.get_thing().field   ->  segments: self, get_thing, field
$Clock.text              ->  segments: text
```

The first erases the call, so a consumer resolving hop by hop treats `get_thing`
as a property and checks `field` against the wrong type. The second drops the
receiver, so `text` looks like a member of the enclosing script.

Both produce false findings rather than missing ones, which is the direction that
costs a user trust.

Give each segment a `kind` and an `is_call` flag:

| `kind` | Example |
|--------|---------|
| `identifier` | `clock` in `self.clock.text` |
| `self` | the leading `self` |
| `call` | `get_thing` in `self.get_thing().field` |
| `node_path` | `$Clock`, `%Clock` |
| `subscript` | `items[0]` |
| `other` | anything else |

A consumer walking types stops at the first segment that is not `self` or
`identifier`. That is all I need: not resolution, just an honest signal that the
chain left the territory where hop-by-hop resolution is valid.

> **Done.** Every hop is a segment now, including the receiver, so `$Clock.text`
> reports `node_path` then `identifier` rather than a lone `text`. Segments carry
> `kind`, `is_call`, and either a `name` or the `text` as written for hops that
> have no name. The `base` field is gone: it existed to carry the unnamed
> receiver and a typed segment does that job properly.
>
> One addition beyond the list: `super` is reported as `other`, not
> `identifier`. Resolving through it means resolving against the base class,
> which this index cannot know, and calling it an identifier would produce the
> same class of false finding this request is about.

### 2. Deterministic project-relative paths

`path` is `res://...` when a `project.godot` is found by walking up, and an
absolute path otherwise. The consumer keys every record on `res://` paths, so a
run that silently produces absolute paths joins nothing and reports no findings,
which is indistinguishable from a clean project.

Add `--project-root <PATH>` to set the root explicitly, and document the discovery
rule. When a root is known, every path in the run must be `res://`. Never mix the
two forms in one run.

> **Done.** `--project-root <PATH>` sets the root, discovery is documented under
> "Paths", and three cases that would have mixed the two forms are now errors
> that name their fix: two projects in one run, some inputs inside a project and
> some outside, and an input outside an explicit root. When no project is found
> at all, paths are absolute and a warning says so on stderr.
>
> The fallback is now an absolute path rather than the path as given, so a file
> indexes under the same name whatever directory the command ran from.
>
> Exit codes separate the two failures: 2 means some files did not parse and the
> rest still produced records, 1 means the run produced no usable index at all.

### 3. State that absent means empty, never unknown

Empty and false fields are omitted: `modifiers`, `annotations`, and
`body_is_pass_only` only appear when they have content. That is the right call for
size, but the consumer needs it written down as a guarantee, because "absent" and
"could not determine" would mean different things and it cannot tell them apart.

Specifically: an absent `body_is_pass_only` means false. An absent `parameters` on
a function or signal means zero parameters, which is what signal arity checking
depends on. A function with no body at all is identified by `body_range: null`.

> **Done.** Written down as a guarantee under "Absent means empty, never
> unknown", including the specific readings asked for here. `body_range` is now
> emitted explicitly as `null` for a `function` with no body, since absence there
> would have to mean two things at once.

### 4. Document `context` as open

The implementation emits `type`, which the specification above does not list. That
is a good addition. State that the set may grow, and that consumers should treat
an unrecognised value as `other` rather than failing, so adding a value later is
not a breaking change.

> **Done.** `type` is listed, and the set is documented as open with the
> instruction to treat unknown values as `other`.

### 5. Nice to have: `argument_of` beyond string literals

`string_literal` carries `argument_of`, `reference` and `member_chain` do not.
Calls do carry their `arguments` as source text, so this is derivable by parsing
that text, and I can live without it. Direct `argument_of` on every record would
remove the re-parsing, which matters most for nested calls such as
`assert(is_instance_valid(thing))`.

> **Done.** `reference` and `member_chain` carry `argument_of` on the same shape
> as `string_literal`. It was a shared helper away, and that example now reports
> `is_instance_valid` at index 0 of `assert` and `thing` at index 0 of
> `is_instance_valid` without anyone re-parsing text.

### Behaviour to keep

These already work and the consumer depends on them:

- One invocation indexes a directory tree, with `-x` to exclude.
- A file that fails to parse emits its header with `"parse_error": true`, the run
  continues, other files still emit records, and the exit code is non-zero.
- Records follow their file's header in output order.
- Ranges carry one-based rows and columns plus byte offsets.
- `scope` nests through inner classes, for example `Inner._ready`.
- `default` is present on variable declarations, which is how an `@export` opting
  out with `= null` is recognised.
- `modifiers` reports `abstract` and `static`, and `annotations` reports the
  annotation names, both for same-line and own-line forms.
- `string_literal.argument_of` names the callee and the argument index.
- `comparison` carries `left` and `right` as text with ranges.

### 6. Truth-test context must survive boolean operators

Found while porting the second consumer check. An expression evaluated for its
truthiness reports `context: "condition"` only when it is the whole condition:

```gdscript
if self.predicate:              # context: condition
while self.predicate:           # context: condition
assert(self.predicate)          # context: argument, argument_of names assert
if self.flag and self.predicate:  # context: other   <- both operands
if not self.predicate:            # context: other
```

The consumer reports a method used as a condition without being called, since the
reference is a `Callable` and the branch is always taken. The last two forms are
real occurrences of that bug and are currently indistinguishable from an ordinary
expression.

Report `condition` for any expression whose value is used as a boolean:

- the condition of `if`, `elif`, `while`, and of a ternary
- both operands of `and` and `or`
- the operand of `not`

Operands of `and` and `or` qualify even outside a condition, as in
`var ready := self.a and self.b`, because the value is still tested for truth.

`assert` needs no special case now that `argument_of` is emitted on member chains
and references, which was requirement 5 and is already implemented.

> **Done.** All three forms report `condition`, and they nest: in
> `if self.a or (self.b and self.c):` all three chains are conditions.
>
> One addition beyond the list: parentheses no longer change a context, for any
> context rather than just this one. `if (self.predicate):` was reporting `other`
> for the same reason the `and` case was, and `return (value)` had the matching
> problem. A parenthesis changes how an expression is written, not what it does.
>
> `is not` and `not in` are deliberately excluded. Their `not` belongs to a
> comparison rather than to a boolean operator, and treating those operands as
> truth-tested would report the bug this check looks for where it cannot be.

### 7. Name the base class on the class declaration

A class declaration record carries `name` but not what it extends. The base
appears only as a `reference` with `context: "type"`, which is indistinguishable
from a return type or a parameter type:

```gdscript
@icon("res://i.svg") class_name Foo extends Bar

func x() -> void:      # `void` is also context: type
```

Add `extends` to the `class` declaration record, holding the base as written:
a class name, or a `"res://path.gd"` string.

The consumer needs this for scripts that Godot cannot compile. When a base script
fails, every script extending it fails too, and reporting fifteen consequences
instead of one root cause is noise. Resolving the base through the engine is not
possible in exactly that case, because a script that does not compile has no base
script to ask for. Tree-sitter parses these files fine, since the failures are
semantic rather than syntactic, so the index is the only source that still works
when it matters.

Until this lands the consumer keeps a small text scan for `class_name` and
`extends`, used only on scripts that failed to load.

> **Done.** `extends` is on the `class` declaration, holding the base as written:
> `Bar`, `Qux.Inner`, or `"res://path.gd"` with its quotes, since every other
> source-text field keeps what was written and the `string_literal` record for
> that same string already carries the path with escapes resolved.
>
> One addition, because the literal request would have missed the common case:
> `extends` is also on the `file` header. A `class` declaration only exists when
> the file has a `class_name`, and most scripts do not have one. Reading it only
> off the class record would have left the text scan in place for the majority of
> a project, which is the opposite of the point. Inner classes carry it too.
>
> The text scan can go.

## Found while implementing requirement 7

The list under "Behaviour to keep" says `annotations` works "both for same-line
and own-line forms". It did not. It does now.

The grammar attaches an annotation written on the same line as a declaration to
that declaration, and leaves one written on the line above as a sibling in front
of it. The index only read the attached ones, so this:

```gdscript
@abstract
func may_target(candidate: Node) -> bool
```

produced a function declaration with no `annotations` and no
`modifiers: ["abstract"]`, while the same annotation written on one line
produced both. That is the `@abstract` bug this index was built to end, in a
different spelling, shipped inside the fix for it.

It went unnoticed because the formatter moves `@export` and `@onready` onto the
declaration's line, so a formatted project hides the failure for the two
annotations people look at most, and shows it for `@abstract`, `@rpc`, `@tool`
and `@warning_ignore`.

Annotations above a declaration now bind to it, comments in between included,
and the declaration's `range` covers them. Annotations that sit above something
that is not a declaration, which in practice means `@tool` above a bare
`extends`, go on the `file` header instead, so nothing falls out either way.

The scale of it, measured on the consumer's own project: 21 own-line
annotations, all 21 previously missing from the index. 18 `@tool`, 2 `@abstract`
and 1 `@icon`. The two `@abstract` are the bug this index was built to end, and
they were being dropped by the tool that fixed them. Worth a check on the
consumer side: any project indexed before this under-reported its annotations.

## What the corpus audit found

The `@abstract` annotation bug above was found by accident, while reading the
grammar for something else. That is not a way to find bugs, so after fixing it
the counts were checked properly: for a corpus of 157 files, the tree-sitter node
count of each construct against the number of records the index emits for it.

Two more silent drops fell out immediately.

**Constructors produced no declaration at all.** `_init` is a keyword in the
grammar, keywords are anonymous nodes, and the lookup scanned named children
only. So every `_init` in every project was missing from the index, while the
paragraph above claimed the opposite. A consumer asking "does this method exist"
would find no constructor anywhere. 17 in the corpus, and this is a method that
almost every non-trivial script has.

**Annotations belonging to no declaration were dropped.** `@warning_ignore(...)`
above a statement inside a `match` binds to nothing, and the fix for the
own-line annotation bug only looked at the top level of a file. Annotations with
no declaration to claim them are now records of their own, at any nesting depth,
carrying the scope they sit in. That replaced an earlier attempt that put them on
the `file` header, which handled `@tool` and nothing deeper.

Both were invisible to every test in this repository, because a fixture only ever
proves what someone thought to write down. `test_no_construct_is_silently_dropped_across_the_fixture_corpus`
now runs the audit over `tests/input` on every `cargo test`. It was checked
against both bugs by reintroducing them: each one makes it fail, with a count.

What the audit does not cover: `reference`, `comparison` and `context`, which
have no one-to-one node to count against. Those remain covered by fixtures only,
so a silent drop there would still go unnoticed. Worth extending if a consumer
finds something missing.

### 8. Bump `schema` when the output changes incompatibly

Requirement 7 shipped as a breaking change while `schema` stayed at 1.

The consumer refuses a schema it does not know and exits non-zero. That guard is
the only thing standing between an incompatible producer and a silent wrong
answer, and it cannot fire if the number never moves. A consumer built against the
old shape would read missing fields as absent rather than as changed, and absent
means "empty, not unknown" by requirement 3, so it would report fewer findings and
exit 0. That is indistinguishable from a clean project.

Bump `schema` whenever a field is renamed, moved, removed, or changes meaning.
Adding a new record kind or a new optional field is not breaking and does not need
a bump.

For the record, requirement 7 as delivered was additive for the first consumer.
`extends` appears on both the `file` record and the `class` declaration record,
and every field the consumer reads is unchanged. It was rechecked field by field
and end to end against saved baselines: no differences, no runtime errors. The
process point stands regardless of this instance being harmless.

> **Done as policy. `schema` stays at 1 for now, by decision rather than by
> oversight.**
>
> The rule is written into "Schema versioning and failure" above, replacing the
> paragraph that said shapes could change without a bump and left it at that.
>
> It was bumped to 2 first, on the argument that requirement 1 had already
> changed `member_chain.segments` under version 1 and two incompatible shapes
> were therefore both called 1. That argument was weaker than it sounded: the
> silent-wrong-answer failure needs a consumer that is still running the old
> build, and there is not one. Both sides are rebuilt together and neither is in
> production, so the bump would have cost a real change on the consumer side to
> guard against something that cannot currently happen.
>
> What survives is everything that costs nothing: the rule for when to bump, the
> conditions that end the current arrangement, and a table recording every shape
> change made under version 1 so an unexpected build can be diagnosed.
>
> Enforcement, such as it is: the exact-output tests fail on any change to any
> record, and a separate test pins the version, so updating the fixtures alone no
> longer lets a shape change go unrecorded. Neither test can tell whether a change
> is breaking. That judgment stays with whoever makes it, which is worth saying
> plainly rather than pretending the tests decide.

### 9. Say which `class` record is the file's own `class_name`

Requirement 7 was filed to retire the consumer's last two text scans, for
`class_name` and for `extends`. It settled `extends` and left the other half.

A file's `class_name` and an inner class both arrive as `kind: "class"` with
`scope: ""`, and nothing on the record separates them:

```gdscript
class_name Hud
extends CanvasLayer

class Helper extends RefCounted:
	var n: int = 0
```

```jsonl
{"record": "declaration", "kind": "class", "name": "Hud",    "scope": "", "extends": "CanvasLayer"}
{"record": "declaration", "kind": "class", "name": "Helper", "scope": "", "extends": "RefCounted"}
```

Position does not decide it either. Godot requires `class_name` above everything
but annotations and `extends`, so the file's own class is always the first of
these, but a file with inner classes and no `class_name` then hands back an inner
class in answer to "what does this file declare". Range does not decide it: an
inner class body usually spans rows, but `class OneLine: pass` is a single row,
the same shape as a `class_name` line.

Add one field to the `class` declaration record, or use a distinct `kind`. Either
reads the same to a consumer. A boolean is enough:

```jsonl
{"record": "declaration", "kind": "class", "name": "Hud", "is_file_class": true, ...}
```

Absent means false, per requirement 3, so only the one record per file gains a
field and nothing else changes shape.

**How the consumer works without it, and why this is still worth doing.** The
name now comes from `ProjectSettings.get_global_class_list()`, which the engine
keeps whether or not a script compiles, so no text scan came back. That source
has a gap the index does not: it is built at import, so a `class_name` added
since the last `--import` is missing from it, and the consumer's fold of
cascading load failures degrades to reporting each failure separately. Noisier,
never wrong. The index reads the file as it is on disk and would close that gap.

Not urgent. Nothing is blocked, and the current answer is good enough for a
project that is imported before it is linted.

> **Done.** The file's own class reports `"is_file_class": true`. Absent means
> false, so exactly one record per file gains a field and nothing else changes
> shape.
>
> A boolean rather than a distinct `kind`, as offered. A consumer that only wants
> "what does this file declare" reads one field; one that walks every class still
> matches a single `kind`, and no existing branch on `kind: "class"` has to learn
> a second spelling.
>
> The node kind decides it, not position: `class_name Foo` is a
> `class_name_statement` in the grammar and an inner class is a
> `class_definition`. So a file with inner classes and no `class_name` marks
> none of them, which is the answer the requirement asks for, and the one-row
> `class OneLine: pass` case never comes up.

## Found while implementing requirement 9

**An inner class was reported as extending whatever the file extends.** This:

```gdscript
extends Node

class Helper:
	var n := 0
```

reported `Helper` as `"extends": "Node"`. It extends nothing written, so its base
is `RefCounted`. An inner class that wrote its own base on a later line, as
`class Helper:` above an indented `extends RefCounted`, was also reported as
`Node`.

Requirement 7 shipped `extends` by reading the class declaration's own `extends`
field and, failing that, searching the parent for an `extends` statement. That
second half is right for `class_name Foo` above `extends Bar`, where the file's
`extends` really is a sibling of the `class_name` statement. For an inner class
the parent is the file, so the search walked out of the class and found the
file's.

Each of the two now looks where its own `extends` can be: a sibling for the
`class_name` statement, the class body for an inner class. This is the failure
mode that requirement 9 is about in a different place, and the reason the two are
in one change: nothing on the record said which class it was, and the code
telling them apart did not either.

Worse than a missing field, because the consumer checks members against the base
it is given, so an inner class was checked against the outer script's base class.
Every member of `Node` looked available on a plain `RefCounted` helper, and any
call the helper's real base does not have looked fine. In the fixture corpus:
19 inner classes, 1 of them given the file's base.

### 10. Report every `$Path` and `%Name` expression

Filed by the consumer while building a check that verifies node paths against
the scenes a script is attached to. The check needs every place a script fetches
a node, and the index showed it only some of them:

```gdscript
@onready var button: Button = $Panel/Button   # declaration.default text only
self.button = $Panel/Button                   # nothing
add_child($Panel/Button)                      # argument text only
$Panel/Button.pressed.connect(_on)            # a node_path segment
```

Reading the path back out of `default` and argument text is the text scanning
this index exists to retire, and the assignment form has no text to scan.

Add a `node_path` record per `$` or `%` expression, with the path decoded the
way a string literal is, and a flag for the unique-name form. Nothing existing
changes shape; the chain segment stays as it is, since a consumer resolving
types still needs to know where a chain left the script's own members.

> **Done.** One `node_path` record per `get_node` node in the grammar, with
> `path`, `unique` for `%Name`, and the usual `scope`, `range` and `context`.
> The corpus test now holds `get_node` nodes equal to `node_path` records, so
> a form the collector misses fails the build rather than the consumer.
>
> `get_node("...")` calls were deliberately left as they were: the
> `string_literal` record with `argument_of` already names the callee and the
> position, and a second record for the same literal would be two facts about
> one thing.
