//! Emits one `member_chain` record per attribute access.
//!
//! Every hop in the chain is a segment, and every segment says what it is. A
//! flat list of names loses what a consumer cannot recover: `$Clock.text` would
//! arrive as a lone `text` and look like a member of the enclosing script, and
//! `self.get_thing().field` would make `get_thing` look like a property, so
//! `field` gets checked against the wrong type. Both produce findings that are
//! wrong rather than missing, which is the direction that costs trust.
//!
//! A consumer walking types stops at the first segment that is not `self` or
//! `identifier`. `kind` is not resolution, it is an honest signal that the chain
//! left the ground where hop-by-hop resolution is valid.

use tree_sitter::Node;

use crate::index::collectors::{
    ArgumentRecord, CollectorContext, collect_arguments, find_argument_position,
    find_expression_context, find_first_named_child_of_kind, write_argument_of_field,
    write_arguments_field, write_scope_field,
};
use crate::index::{
    json_push_bool_field, json_push_escaped_string, json_push_field_name, json_push_range_field,
    json_push_string_field,
};
use crate::linter::lib::{SourceRange, get_node_text, get_range};
use crate::node_kind::GDScriptNodeKind;

pub const TARGET_NODE_KINDS: &[GDScriptNodeKind] = &[GDScriptNodeKind::Attribute];

pub struct SegmentRecord<'a> {
    /// `identifier`, `self`, `call`, `node_path`, `subscript` or `other`.
    pub kind: &'static str,
    /// The member name, when the segment has one. `$Clock` and `(a + b)` do not.
    pub name: Option<&'a str>,
    /// The segment as written, for segments with no name.
    pub text: Option<&'a str>,
    pub is_call: bool,
    pub range: SourceRange,
}

pub struct MemberChainRecord<'a> {
    pub segments: Vec<SegmentRecord<'a>>,
    pub range: SourceRange,
    pub is_call: bool,
    pub arguments: Vec<ArgumentRecord<'a>>,
    pub context: &'static str,
    pub argument_of: Option<crate::index::collectors::ArgumentPosition<'a>>,
}

pub fn collect(node: &Node, context: &CollectorContext, output: &mut String) {
    let mut record = MemberChainRecord {
        segments: Vec::with_capacity(node.named_child_count()),
        range: get_range(node),
        is_call: false,
        arguments: Vec::new(),
        context: find_expression_context(node, context.source),
        argument_of: find_argument_position(node, context.source),
    };

    for child_index in 0..node.named_child_count() {
        let Some(child) = node.named_child(child_index as u32) else {
            continue;
        };
        record.segments.push(build_segment(&child, context.source));
    }

    // The chain is a call when its last hop is one. The arguments belong to the
    // chain rather than to the segment so that a consumer reading a call reads
    // it the same way whether it wrote `foo(1)` or `bar.foo(1)`.
    if let Some(last_segment) = record.segments.last()
        && last_segment.is_call
    {
        record.is_call = true;
        if let Some(last_child) = node.named_child(node.named_child_count() as u32 - 1)
            && let Some(arguments_node) = last_child.child_by_field_name("arguments")
        {
            collect_arguments(&arguments_node, context.source, &mut record.arguments);
        }
    }

    write_member_chain_record(&record, context.scope, output);
}

/// Classifies one hop of a chain.
///
/// `super` is reported as `other` rather than as an identifier on purpose:
/// resolving through it means resolving against the base class, which this index
/// does not know. Calling it an identifier would invite exactly the wrong
/// lookup.
fn build_segment<'a>(node: &Node, source: &'a str) -> SegmentRecord<'a> {
    let node_kind = GDScriptNodeKind::get_kind_from_ast_node(*node);
    let range = get_range(node);

    if node_kind == GDScriptNodeKind::Identifier {
        let name = get_node_text(node, source);
        let kind = match name {
            "self" => "self",
            "super" => "other",
            _ => "identifier",
        };
        return SegmentRecord {
            kind,
            name: Some(name),
            text: None,
            is_call: false,
            range,
        };
    }

    // `foo()` and `foo[0]` wrap the member name together with their brackets,
    // both inside a chain and when they start one.
    if matches!(
        node_kind,
        GDScriptNodeKind::AttributeCall
            | GDScriptNodeKind::AttributeSubscript
            | GDScriptNodeKind::Call
            | GDScriptNodeKind::Subscript
    ) {
        let is_call = matches!(
            node_kind,
            GDScriptNodeKind::AttributeCall | GDScriptNodeKind::Call
        );
        let name_node = find_first_named_child_of_kind(node, GDScriptNodeKind::Identifier);
        let mut name = None;
        if let Some(name_node) = name_node {
            name = Some(get_node_text(&name_node, source));
        }
        return SegmentRecord {
            kind: if is_call { "call" } else { "subscript" },
            name,
            text: if name.is_some() {
                None
            } else {
                Some(get_node_text(node, source).trim())
            },
            is_call,
            range,
        };
    }

    if node_kind == GDScriptNodeKind::GetNode {
        return SegmentRecord {
            kind: "node_path",
            name: None,
            text: Some(get_node_text(node, source).trim()),
            is_call: false,
            range,
        };
    }

    SegmentRecord {
        kind: "other",
        name: None,
        text: Some(get_node_text(node, source).trim()),
        is_call: false,
        range,
    }
}

fn write_member_chain_record(record: &MemberChainRecord, scope: &str, output: &mut String) {
    output.push_str("{\"record\":\"member_chain\"");
    json_push_field_name("segments", output);
    output.push('[');
    for (segment_index, segment) in record.segments.iter().enumerate() {
        if segment_index > 0 {
            output.push(',');
        }
        output.push_str("{\"kind\":");
        json_push_escaped_string(segment.kind, output);
        if let Some(name) = segment.name {
            json_push_string_field("name", name, output);
        }
        if let Some(text) = segment.text {
            json_push_string_field("text", text, output);
        }
        if segment.is_call {
            json_push_bool_field("is_call", true, output);
        }
        json_push_range_field("range", &segment.range, output);
        output.push('}');
    }
    output.push(']');

    write_scope_field(scope, output);
    json_push_range_field("range", &record.range, output);
    if record.is_call {
        json_push_bool_field("is_call", true, output);
    }
    write_arguments_field(&record.arguments, output);
    json_push_string_field("context", record.context, output);
    if let Some(argument_of) = &record.argument_of {
        write_argument_of_field(argument_of, output);
    }
    output.push_str("}\n");
}
