//! Emits one `comparison` record per comparison expression.
//!
//! Every comparison is emitted, not only those against `null`. The linter's
//! `comparison-with-itself` rule wants the same data, so the record earns its
//! place beyond any single consumer.

use tree_sitter::Node;

use crate::index::collectors::{CollectorContext, write_scope_field};
use crate::index::{
    json_push_escaped_string, json_push_field_name, json_push_range_field, json_push_string_field,
};
use crate::linter::lib::{SourceRange, get_node_text, get_range};
use crate::node_kind::GDScriptNodeKind;

pub const TARGET_NODE_KINDS: &[GDScriptNodeKind] = &[GDScriptNodeKind::BinaryOperator];

pub struct ComparisonOperand<'a> {
    pub text: &'a str,
    pub range: SourceRange,
}

pub struct ComparisonRecord<'a> {
    pub operator: String,
    pub left: ComparisonOperand<'a>,
    pub right: ComparisonOperand<'a>,
    pub range: SourceRange,
}

pub fn collect(node: &Node, context: &CollectorContext, output: &mut String) {
    let Some(operator) = read_comparison_operator(node, context.source) else {
        return;
    };
    let (Some(left_node), Some(right_node)) = (
        node.child_by_field_name("left"),
        node.child_by_field_name("right"),
    ) else {
        return;
    };

    let record = ComparisonRecord {
        operator,
        left: ComparisonOperand {
            text: get_node_text(&left_node, context.source).trim(),
            range: get_range(&left_node),
        },
        right: ComparisonOperand {
            text: get_node_text(&right_node, context.source).trim(),
            range: get_range(&right_node),
        },
        range: get_range(node),
    };
    write_comparison_record(&record, context.scope, output);
}

/// Returns the operator when this binary expression compares, None when it does
/// something else such as adding.
///
/// `not in` and `is not` are spelled with two operator tokens in the grammar,
/// and reporting either of them as its positive form would invert the meaning.
fn read_comparison_operator(node: &Node, source: &str) -> Option<String> {
    let mut operator = String::new();
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            if cursor.field_name() == Some("op") {
                if !operator.is_empty() {
                    operator.push(' ');
                }
                operator.push_str(get_node_text(&cursor.node(), source));
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    if matches!(
        operator.as_str(),
        "==" | "!=" | "<" | "<=" | ">" | ">=" | "in" | "is" | "not in" | "is not"
    ) {
        return Some(operator);
    }
    None
}

fn write_comparison_record(record: &ComparisonRecord, scope: &str, output: &mut String) {
    output.push_str("{\"record\":\"comparison\"");
    json_push_string_field("operator", &record.operator, output);
    write_operand_field("left", &record.left, output);
    write_operand_field("right", &record.right, output);
    write_scope_field(scope, output);
    json_push_range_field("range", &record.range, output);
    output.push_str("}\n");
}

fn write_operand_field(name: &str, operand: &ComparisonOperand, output: &mut String) {
    json_push_field_name(name, output);
    output.push_str("{\"text\":");
    json_push_escaped_string(operand.text, output);
    json_push_range_field("range", &operand.range, output);
    output.push('}');
}
