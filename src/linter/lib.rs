use tree_sitter::Node;

pub fn get_node_text<'a>(node: &Node, source_code: &'a str) -> &'a str {
    &source_code[node.start_byte()..node.end_byte()]
}

pub fn get_line_column(node: &Node) -> (usize, usize) {
    let start_position = node.start_position();
    (start_position.row + 1, start_position.column + 1)
}

/// Both ends of one AST node, in the two coordinate systems tools need.
///
/// Rows and columns are one-based to match [`get_line_column`], so anything
/// built on this agrees with what the linter prints. Columns count bytes within
/// the line, which is what tree-sitter reports. Byte offsets come along because
/// they are the only unambiguous way to slice the original source, and a
/// consumer that needs character columns can compute them from the source and
/// the offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRange {
    pub start_row: usize,
    pub start_column: usize,
    pub end_row: usize,
    pub end_column: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

pub fn get_range(node: &Node) -> SourceRange {
    let start_position = node.start_position();
    let end_position = node.end_position();
    SourceRange {
        start_row: start_position.row + 1,
        start_column: start_position.column + 1,
        end_row: end_position.row + 1,
        end_column: end_position.column + 1,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
    }
}
