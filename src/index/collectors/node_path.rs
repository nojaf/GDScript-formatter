//! Emits one `node_path` record per `$Path` or `%Name` expression, with the
//! path as Godot will read it.
//!
//! A `$Clock` at the head of a member chain already arrives as a `node_path`
//! segment. A bare one does not: `self.button = $Panel/Button`, `add_child($X)`
//! and the value of an `@onready` variable left no record at all, so a consumer
//! checking paths against the scene could see `$Panel/Button.text` and miss the
//! line that actually assigns the node. Every `$` and `%` expression is a record
//! here, whatever it sits in.

use tree_sitter::Node;

use crate::index::collectors::string_literal::decode_string_literal;
use crate::index::collectors::{CollectorContext, find_expression_context, write_scope_field};
use crate::index::{json_push_bool_field, json_push_range_field, json_push_string_field};
use crate::linter::lib::{get_node_text, get_range};
use crate::node_kind::GDScriptNodeKind;

pub const TARGET_NODE_KINDS: &[GDScriptNodeKind] = &[GDScriptNodeKind::GetNode];

pub fn collect(node: &Node, context: &CollectorContext, output: &mut String) {
    let written = get_node_text(node, context.source).trim();
    let Some(sigil) = written.chars().next() else {
        return;
    };
    let after_sigil = written[sigil.len_utf8()..].trim_start();

    // `$"Panel/With Space"` quotes the path and `$Panel/Button` does not. The
    // quoted form is a string in the grammar, so it decodes like one.
    let path = if after_sigil.starts_with('"') || after_sigil.starts_with('\'') {
        decode_string_literal(after_sigil)
    } else {
        after_sigil.to_string()
    };

    output.push_str("{\"record\":\"node_path\"");
    json_push_string_field("path", &path, output);
    if sigil == '%' {
        json_push_bool_field("unique", true, output);
    }
    write_scope_field(context.scope, output);
    json_push_range_field("range", &get_range(node), output);
    json_push_string_field(
        "context",
        find_expression_context(node, context.source),
        output,
    );
    output.push_str("}\n");
}
