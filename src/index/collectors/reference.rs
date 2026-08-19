//! Emits one `reference` record per bare identifier used as a value or called
//! directly.
//!
//! Every identifier is emitted, locals and parameters included. Filtering here
//! would push the consumer back to counting occurrences: to know that `target`
//! on one line is the local declared two lines above rather than the member
//! declared at the top of the file, it needs both the declaration and the
//! occurrence, with scopes.
//!
//! Identifiers that another record already covers are skipped: declaration
//! names, the segments of a member chain, annotation names and parameter names.

use tree_sitter::Node;

use crate::index::collectors::{
    ArgumentRecord, CollectorContext, collect_arguments, find_expression_context, is_field_of,
    write_arguments_field, write_scope_field,
};
use crate::index::{json_push_bool_field, json_push_range_field, json_push_string_field};
use crate::linter::lib::{SourceRange, get_node_text, get_range};
use crate::node_kind::GDScriptNodeKind;

pub const TARGET_NODE_KINDS: &[GDScriptNodeKind] = &[GDScriptNodeKind::Identifier];

pub struct ReferenceRecord<'a> {
    pub name: &'a str,
    /// The whole call expression when this is a call, the identifier otherwise.
    pub range: SourceRange,
    pub name_range: SourceRange,
    pub is_call: bool,
    pub arguments: Vec<ArgumentRecord<'a>>,
    pub context: &'static str,
}

pub fn collect(node: &Node, context: &CollectorContext, output: &mut String) {
    let Some(parent) = node.parent() else {
        return;
    };
    if is_covered_by_another_record(node, &parent) {
        return;
    }

    let name_range = get_range(node);
    let mut record = ReferenceRecord {
        name: get_node_text(node, context.source),
        range: name_range,
        name_range,
        is_call: false,
        arguments: Vec::new(),
        context: find_expression_context(node),
    };

    // `print(a)` is one reference to `print`, not a reference plus an unrelated
    // argument list, so the call node decides the range and the context.
    let parent_kind = GDScriptNodeKind::get_kind_from_ast_node(parent);
    if parent_kind == GDScriptNodeKind::Call && is_first_named_child(&parent, node) {
        record.range = get_range(&parent);
        record.is_call = true;
        record.context = find_expression_context(&parent);
        if let Some(arguments_node) = parent.child_by_field_name("arguments") {
            collect_arguments(&arguments_node, context.source, &mut record.arguments);
        }
    }

    write_reference_record(&record, context.scope, output);
}

/// The grammar spells a declaration's name with the same node kind it uses for
/// a reference, and member chains carry their own per-segment ranges. Emitting
/// those identifiers here would report the same text twice under two meanings.
fn is_covered_by_another_record(node: &Node, parent: &Node) -> bool {
    if is_field_of(parent, "name", node) {
        return true;
    }

    let parent_kind = GDScriptNodeKind::get_kind_from_ast_node(*parent);
    if matches!(
        parent_kind,
        GDScriptNodeKind::Attribute
            | GDScriptNodeKind::AttributeCall
            | GDScriptNodeKind::AttributeSubscript
            | GDScriptNodeKind::Annotation
            | GDScriptNodeKind::Parameters
    ) {
        return true;
    }
    if matches!(
        parent_kind,
        GDScriptNodeKind::Parameter | GDScriptNodeKind::VariadicParameter
    ) && is_first_named_child(parent, node)
    {
        return true;
    }
    if parent_kind == GDScriptNodeKind::Enumerator && is_field_of(parent, "left", node) {
        return true;
    }
    if parent_kind == GDScriptNodeKind::ForStatement && is_field_of(parent, "left", node) {
        return true;
    }

    false
}

fn is_first_named_child(parent: &Node, node: &Node) -> bool {
    match parent.named_child(0) {
        Some(first_child) => first_child.id() == node.id(),
        None => false,
    }
}

fn write_reference_record(record: &ReferenceRecord, scope: &str, output: &mut String) {
    output.push_str("{\"record\":\"reference\"");
    json_push_string_field("name", record.name, output);
    write_scope_field(scope, output);
    json_push_range_field("range", &record.range, output);
    json_push_range_field("name_range", &record.name_range, output);
    if record.is_call {
        json_push_bool_field("is_call", true, output);
    }
    write_arguments_field(&record.arguments, output);
    json_push_string_field("context", record.context, output);
    output.push_str("}\n");
}
