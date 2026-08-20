//! Emits an `annotation` record for every annotation no declaration claims.
//!
//! Most annotations belong to a declaration and are reported on it. The rest
//! have nothing to attach to: `@tool` above a bare `extends`, or
//! `@warning_ignore(...)` above a statement inside a `match`. They used to fall
//! out of the index entirely.
//!
//! Between this record and the `annotations` field on declarations, every
//! annotation in a file is reported exactly once.

use tree_sitter::Node;

use crate::index::collectors::declaration::absorbs_preceding_annotations;
use crate::index::collectors::{
    CollectorContext, find_first_named_child_of_kind, write_scope_field,
};
use crate::index::{
    json_push_escaped_string, json_push_field_name, json_push_range_field, json_push_string_field,
};
use crate::linter::lib::{get_node_text, get_range};
use crate::node_kind::GDScriptNodeKind;

pub const TARGET_NODE_KINDS: &[GDScriptNodeKind] = &[GDScriptNodeKind::Annotation];

pub fn collect(node: &Node, context: &CollectorContext, output: &mut String) {
    if is_claimed_by_a_declaration(node) {
        return;
    }
    let Some(name_node) = find_first_named_child_of_kind(node, GDScriptNodeKind::Identifier) else {
        return;
    };

    output.push_str("{\"record\":\"annotation\"");
    json_push_string_field("name", get_node_text(&name_node, context.source), output);

    if let Some(arguments_node) = node.child_by_field_name("arguments")
        && arguments_node.named_child_count() > 0
    {
        json_push_field_name("arguments", output);
        output.push('[');
        for argument_index in 0..arguments_node.named_child_count() {
            let Some(argument_node) = arguments_node.named_child(argument_index as u32) else {
                continue;
            };
            if argument_index > 0 {
                output.push(',');
            }
            json_push_escaped_string(get_node_text(&argument_node, context.source).trim(), output);
        }
        output.push(']');
    }

    write_scope_field(context.scope, output);
    json_push_range_field("range", &get_range(node), output);
    output.push_str("}\n");
}

/// True when a declaration already reports this annotation.
///
/// An annotation written on the same line sits inside the declaration's
/// `annotations` node. One written above a declaration is a sibling, and the
/// declaration walks back over the whole run to claim it, so this has to look
/// past the rest of the run the same way.
fn is_claimed_by_a_declaration(node: &Node) -> bool {
    if let Some(parent) = node.parent()
        && GDScriptNodeKind::get_kind_from_ast_node(parent) == GDScriptNodeKind::Annotations
    {
        return true;
    }

    let mut following_sibling = node.next_named_sibling();
    while let Some(sibling) = following_sibling {
        if !matches!(
            GDScriptNodeKind::get_kind_from_ast_node(sibling),
            GDScriptNodeKind::Comment | GDScriptNodeKind::Annotation
        ) {
            break;
        }
        following_sibling = sibling.next_named_sibling();
    }
    match following_sibling {
        Some(sibling) => {
            absorbs_preceding_annotations(GDScriptNodeKind::get_kind_from_ast_node(sibling))
        }
        None => false,
    }
}
