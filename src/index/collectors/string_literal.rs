//! Emits one `string_literal` record per string, `StringName` or `NodePath`
//! literal, with its escapes resolved.
//!
//! `argument_of` is what tells `self.call("late_bound")` apart from
//! `print("all done")`. One names a method, the other is prose. Counting string
//! contents as references without that distinction is what kept dead functions
//! looking alive.

use tree_sitter::Node;

use crate::index::collectors::{
    ArgumentPosition, CollectorContext, find_argument_position, write_argument_of_field,
    write_scope_field,
};
use crate::index::{json_push_range_field, json_push_string_field};
use crate::linter::lib::{SourceRange, get_node_text, get_range};
use crate::node_kind::GDScriptNodeKind;

pub const TARGET_NODE_KINDS: &[GDScriptNodeKind] = &[
    GDScriptNodeKind::String,
    GDScriptNodeKind::StringName,
    GDScriptNodeKind::NodePath,
];

pub struct StringLiteralRecord<'a> {
    pub value: String,
    /// `string`, `string_name` or `node_path`. Omitted from the output when it
    /// is a plain string.
    pub literal_kind: &'static str,
    pub range: SourceRange,
    pub argument_of: Option<ArgumentPosition<'a>>,
}

pub fn collect(node: &Node, context: &CollectorContext, output: &mut String) {
    let node_kind = GDScriptNodeKind::get_kind_from_ast_node(*node);
    let record = StringLiteralRecord {
        value: decode_string_literal(get_node_text(node, context.source)),
        literal_kind: match node_kind {
            GDScriptNodeKind::StringName => "string_name",
            GDScriptNodeKind::NodePath => "node_path",
            _ => "string",
        },
        range: get_range(node),
        argument_of: find_argument_position(node, context.source),
    };
    write_string_literal_record(&record, context.scope, output);
}

/// Returns the contents of a string literal with its escapes resolved.
///
/// Unknown escapes keep the character that follows the backslash: the parser
/// already accepted the file, so refusing to decode here would only lose text.
fn decode_string_literal(literal_text: &str) -> String {
    let without_prefix = literal_text
        .strip_prefix('&')
        .or_else(|| literal_text.strip_prefix('^'))
        .unwrap_or(literal_text);

    let mut contents = without_prefix;
    for delimiter in ["\"\"\"", "'''", "\"", "'"] {
        if let Some(after_opening) = contents.strip_prefix(delimiter) {
            contents = after_opening
                .strip_suffix(delimiter)
                .unwrap_or(after_opening);
            break;
        }
    }

    if !contents.contains('\\') {
        return contents.to_string();
    }

    let mut decoded = String::with_capacity(contents.len());
    let mut characters = contents.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        let Some(escaped_character) = characters.next() else {
            decoded.push('\\');
            break;
        };
        match escaped_character {
            'n' => decoded.push('\n'),
            't' => decoded.push('\t'),
            'r' => decoded.push('\r'),
            'a' => decoded.push('\u{07}'),
            'b' => decoded.push('\u{08}'),
            'f' => decoded.push('\u{0C}'),
            'v' => decoded.push('\u{0B}'),
            '0' => decoded.push('\0'),
            'u' => push_unicode_escape(&mut characters, 4, &mut decoded),
            'U' => push_unicode_escape(&mut characters, 6, &mut decoded),
            _ => decoded.push(escaped_character),
        }
    }
    decoded
}

fn push_unicode_escape(characters: &mut std::str::Chars, digit_count: usize, decoded: &mut String) {
    let mut code_point = 0u32;
    let mut digits_read = 0;
    while digits_read < digit_count {
        let Some(digit_character) = characters.clone().next() else {
            break;
        };
        let Some(digit) = digit_character.to_digit(16) else {
            break;
        };
        characters.next();
        code_point = code_point * 16 + digit;
        digits_read += 1;
    }
    match char::from_u32(code_point) {
        Some(character) => decoded.push(character),
        None => decoded.push('\u{FFFD}'),
    }
}

fn write_string_literal_record(record: &StringLiteralRecord, scope: &str, output: &mut String) {
    output.push_str("{\"record\":\"string_literal\"");
    json_push_string_field("value", &record.value, output);
    if record.literal_kind != "string" {
        json_push_string_field("literal_kind", record.literal_kind, output);
    }
    write_scope_field(scope, output);
    json_push_range_field("range", &record.range, output);
    if let Some(argument_of) = &record.argument_of {
        write_argument_of_field(argument_of, output);
    }
    output.push_str("}\n");
}
