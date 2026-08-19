//! The collector registry and the helpers collectors share.
//!
//! A collector is the index's counterpart to a linter `Rule`: it names the node
//! kinds it cares about and turns a matching node into records. Collectors hold
//! no state, so the registry stores plain function pointers rather than trait
//! objects.

pub mod comment;
pub mod comparison;
pub mod declaration;
pub mod member_chain;
pub mod reference;
pub mod string_literal;

use tree_sitter::Node;

use crate::index::{
    json_push_escaped_string, json_push_field_name, json_push_range_field, json_push_string_field,
};
use crate::linter::lib::{SourceRange, get_node_text, get_range};
use crate::node_kind::GDScriptNodeKind;

/// What every collector needs beyond the node itself.
pub struct CollectorContext<'a> {
    pub source: &'a str,
    /// Dot separated enclosing declarations, empty at file level.
    pub scope: &'a str,
}

pub struct CollectorDefinition {
    /// The node kinds this collector runs on. The walk calls a collector only
    /// when it meets one of them.
    pub target_node_kinds: &'static [GDScriptNodeKind],
    /// Writes zero or more complete JSON Lines records, newline terminated.
    pub collect: fn(&Node, &CollectorContext, &mut String),
}

pub const ALL_COLLECTORS: &[CollectorDefinition] = &[
    CollectorDefinition {
        target_node_kinds: declaration::TARGET_NODE_KINDS,
        collect: declaration::collect,
    },
    CollectorDefinition {
        target_node_kinds: reference::TARGET_NODE_KINDS,
        collect: reference::collect,
    },
    CollectorDefinition {
        target_node_kinds: member_chain::TARGET_NODE_KINDS,
        collect: member_chain::collect,
    },
    CollectorDefinition {
        target_node_kinds: string_literal::TARGET_NODE_KINDS,
        collect: string_literal::collect,
    },
    CollectorDefinition {
        target_node_kinds: comparison::TARGET_NODE_KINDS,
        collect: comparison::collect,
    },
    CollectorDefinition {
        target_node_kinds: comment::TARGET_NODE_KINDS,
        collect: comment::collect,
    },
];

/// One argument in a call, annotation or signal emission, as written.
pub struct ArgumentRecord<'a> {
    pub text: &'a str,
    pub range: SourceRange,
}

/// Reads the arguments of an `arguments` node. Comments can appear between
/// arguments in wrapped calls, and they are not arguments.
pub fn collect_arguments<'a>(
    arguments_node: &Node,
    source: &'a str,
    arguments: &mut Vec<ArgumentRecord<'a>>,
) {
    for child_index in 0..arguments_node.named_child_count() {
        let Some(child) = arguments_node.named_child(child_index as u32) else {
            continue;
        };
        if GDScriptNodeKind::get_kind_from_ast_node(child) == GDScriptNodeKind::Comment {
            continue;
        }
        arguments.push(ArgumentRecord {
            text: get_node_text(&child, source).trim(),
            range: get_range(&child),
        });
    }
}

pub fn write_arguments_field(arguments: &[ArgumentRecord], output: &mut String) {
    if arguments.is_empty() {
        return;
    }
    json_push_field_name("arguments", output);
    output.push('[');
    for (argument_index, argument) in arguments.iter().enumerate() {
        if argument_index > 0 {
            output.push(',');
        }
        output.push_str("{\"text\":");
        json_push_escaped_string(argument.text, output);
        json_push_range_field("range", &argument.range, output);
        output.push('}');
    }
    output.push(']');
}

/// Writes the `scope` field. It is always present, including when empty: a
/// consumer resolving a name has to know it looked at a file level record
/// rather than at a record whose scope was dropped.
pub fn write_scope_field(scope: &str, output: &mut String) {
    json_push_string_field("scope", scope, output);
}

/// Returns true when `node` is the named field `field_name` of `parent`.
pub fn is_field_of(parent: &Node, field_name: &str, node: &Node) -> bool {
    match parent.child_by_field_name(field_name) {
        Some(field_node) => field_node.id() == node.id(),
        None => false,
    }
}

pub fn find_first_named_child_of_kind<'tree>(
    node: &Node<'tree>,
    node_kind: GDScriptNodeKind,
) -> Option<Node<'tree>> {
    for child_index in 0..node.named_child_count() {
        let child = node.named_child(child_index as u32)?;
        if GDScriptNodeKind::get_kind_from_ast_node(child) == node_kind {
            return Some(child);
        }
    }
    None
}

/// Classifies what an expression is doing where it sits.
///
/// `statement` is the value that unlocks new checks on the consumer side: an
/// expression whose context is `statement` and which is not a call does nothing
/// at runtime.
pub fn find_expression_context(node: &Node) -> &'static str {
    let Some(parent) = node.parent() else {
        return "other";
    };
    let parent_kind = GDScriptNodeKind::get_kind_from_ast_node(parent);

    if parent_kind == GDScriptNodeKind::Arguments {
        return "argument";
    }
    if parent_kind == GDScriptNodeKind::ExpressionStatement {
        return "statement";
    }
    if parent_kind == GDScriptNodeKind::ReturnStatement {
        return "return_value";
    }
    if matches!(
        parent_kind,
        GDScriptNodeKind::Assignment | GDScriptNodeKind::AugmentedAssignment
    ) {
        if is_field_of(&parent, "left", node) {
            return "assignment_target";
        }
        if is_field_of(&parent, "right", node) {
            return "assignment_value";
        }
    }
    // The initializer of a declaration is an assignment in everything but
    // spelling, and consumers treat it the same way.
    if matches!(
        parent_kind,
        GDScriptNodeKind::Variable
            | GDScriptNodeKind::ExportVariable
            | GDScriptNodeKind::OnReadyVariable
            | GDScriptNodeKind::Const
    ) && is_field_of(&parent, "value", node)
    {
        return "assignment_value";
    }
    if is_field_of(&parent, "condition", node) {
        return "condition";
    }
    if is_inside_type_annotation(node) {
        return "type";
    }

    "other"
}

/// True when the node sits in a type position, such as `Label` in
/// `var clock: Label` or `int` in `Array[int]`.
///
/// A name written as a type is not a value use, and a consumer that resolves
/// members would otherwise have to guess which of the two it is looking at.
fn is_inside_type_annotation(node: &Node) -> bool {
    let mut ancestor = node.parent();
    while let Some(current) = ancestor {
        let current_kind = GDScriptNodeKind::get_kind_from_ast_node(current);
        if current_kind == GDScriptNodeKind::Type {
            return true;
        }
        if !matches!(
            current_kind,
            GDScriptNodeKind::Subscript | GDScriptNodeKind::SubscriptArguments
        ) {
            return false;
        }
        ancestor = current.parent();
    }
    false
}
