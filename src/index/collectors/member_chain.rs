//! Emits one `member_chain` record per attribute access.
//!
//! Segments carry their own ranges because `self.clock.ziggy` should be
//! reported at `ziggy`, not at the start of the line.

use tree_sitter::Node;

use crate::index::collectors::{
    ArgumentRecord, CollectorContext, collect_arguments, find_expression_context,
    find_first_named_child_of_kind, write_arguments_field, write_scope_field,
};
use crate::index::{
    json_push_bool_field, json_push_escaped_string, json_push_field_name, json_push_range_field,
    json_push_string_field,
};
use crate::linter::lib::{SourceRange, get_node_text, get_range};
use crate::node_kind::GDScriptNodeKind;

pub const TARGET_NODE_KINDS: &[GDScriptNodeKind] = &[GDScriptNodeKind::Attribute];

pub struct SegmentRecord<'a> {
    pub name: &'a str,
    pub range: SourceRange,
}

pub struct MemberChainRecord<'a> {
    pub segments: Vec<SegmentRecord<'a>>,
    /// The expression the chain starts from when it is not a name, such as the
    /// `$Clock` in `$Clock.text` or the `get_tree()` in `get_tree().paused`.
    /// A consumer resolving the chain needs to know it cannot start from a name.
    pub base: Option<ArgumentRecord<'a>>,
    pub range: SourceRange,
    pub is_call: bool,
    pub arguments: Vec<ArgumentRecord<'a>>,
    pub context: &'static str,
}

pub fn collect(node: &Node, context: &CollectorContext, output: &mut String) {
    let mut record = MemberChainRecord {
        segments: Vec::with_capacity(node.named_child_count()),
        base: None,
        range: get_range(node),
        is_call: false,
        arguments: Vec::new(),
        context: find_expression_context(node),
    };

    let last_named_child_index = node.named_child_count().saturating_sub(1);
    for child_index in 0..node.named_child_count() {
        let Some(child) = node.named_child(child_index as u32) else {
            continue;
        };
        let child_kind = GDScriptNodeKind::get_kind_from_ast_node(child);

        if child_kind == GDScriptNodeKind::Identifier {
            record.segments.push(SegmentRecord {
                name: get_node_text(&child, context.source),
                range: get_range(&child),
            });
            continue;
        }

        // `foo()` and `foo[0]` inside a chain wrap the member name together
        // with their brackets, so the segment is the name they wrap.
        if matches!(
            child_kind,
            GDScriptNodeKind::AttributeCall | GDScriptNodeKind::AttributeSubscript
        ) {
            let Some(name_node) =
                find_first_named_child_of_kind(&child, GDScriptNodeKind::Identifier)
            else {
                continue;
            };
            record.segments.push(SegmentRecord {
                name: get_node_text(&name_node, context.source),
                range: get_range(&name_node),
            });
            if child_kind == GDScriptNodeKind::AttributeCall
                && child_index == last_named_child_index
            {
                record.is_call = true;
                if let Some(arguments_node) = child.child_by_field_name("arguments") {
                    collect_arguments(&arguments_node, context.source, &mut record.arguments);
                }
            }
            continue;
        }

        if record.segments.is_empty() && record.base.is_none() {
            record.base = Some(ArgumentRecord {
                text: get_node_text(&child, context.source).trim(),
                range: get_range(&child),
            });
        }
    }

    if record.segments.is_empty() {
        return;
    }

    write_member_chain_record(&record, context.scope, output);
}

fn write_member_chain_record(record: &MemberChainRecord, scope: &str, output: &mut String) {
    output.push_str("{\"record\":\"member_chain\"");
    json_push_field_name("segments", output);
    output.push('[');
    for (segment_index, segment) in record.segments.iter().enumerate() {
        if segment_index > 0 {
            output.push(',');
        }
        output.push_str("{\"name\":");
        json_push_escaped_string(segment.name, output);
        json_push_range_field("range", &segment.range, output);
        output.push('}');
    }
    output.push(']');

    if let Some(base) = &record.base {
        json_push_field_name("base", output);
        output.push_str("{\"text\":");
        json_push_escaped_string(base.text, output);
        json_push_range_field("range", &base.range, output);
        output.push('}');
    }

    write_scope_field(scope, output);
    json_push_range_field("range", &record.range, output);
    if record.is_call {
        json_push_bool_field("is_call", true, output);
    }
    write_arguments_field(&record.arguments, output);
    json_push_string_field("context", record.context, output);
    output.push_str("}\n");
}
