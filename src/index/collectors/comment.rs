//! Emits one `comment` record per comment.
//!
//! The consumer reads comments for suppression directives such as
//! `# gdlint:ignore-next-line:unknown-member`, and binding a directive to the
//! declaration it applies to is exactly the kind of line anchoring that has
//! broken before. With comment ranges and declaration ranges in the same
//! stream, binding is an interval comparison rather than a guess.

use tree_sitter::Node;

use crate::index::collectors::{CollectorContext, write_scope_field};
use crate::index::{json_push_bool_field, json_push_range_field, json_push_string_field};
use crate::linter::lib::{SourceRange, get_node_text, get_range};
use crate::node_kind::GDScriptNodeKind;

pub const TARGET_NODE_KINDS: &[GDScriptNodeKind] = &[GDScriptNodeKind::Comment];

pub struct CommentRecord<'a> {
    /// The comment as written, leading `#` included.
    pub text: &'a str,
    pub range: SourceRange,
    pub is_documentation: bool,
    pub is_trailing: bool,
}

pub fn collect(node: &Node, context: &CollectorContext, output: &mut String) {
    let text = get_node_text(node, context.source);
    let record = CommentRecord {
        text,
        range: get_range(node),
        is_documentation: text.starts_with("##"),
        is_trailing: has_code_before_on_line(node.start_byte(), context.source),
    };
    write_comment_record(&record, context.scope, output);
}

fn has_code_before_on_line(start_byte: usize, source: &str) -> bool {
    let source_bytes = source.as_bytes();
    let mut scan_position = start_byte;
    while scan_position > 0 {
        let previous_byte = source_bytes[scan_position - 1];
        if previous_byte == b'\n' {
            return false;
        }
        if !previous_byte.is_ascii_whitespace() {
            return true;
        }
        scan_position -= 1;
    }
    false
}

fn write_comment_record(record: &CommentRecord, scope: &str, output: &mut String) {
    output.push_str("{\"record\":\"comment\"");
    json_push_string_field("text", record.text, output);
    write_scope_field(scope, output);
    json_push_range_field("range", &record.range, output);
    if record.is_documentation {
        json_push_bool_field("is_documentation", true, output);
    }
    if record.is_trailing {
        json_push_bool_field("is_trailing", true, output);
    }
    output.push_str("}\n");
}
