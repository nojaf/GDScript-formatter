//! Emits a machine-readable index of GDScript source code as JSON Lines.
//!
//! The formatter knows where everything sits in a file and nothing about what
//! any of it means. Tools that run inside Godot know the opposite: they can ask
//! the engine for every member of a class but have no source positions. This
//! sub-command hands the syntactic half over so those tools stop guessing at it
//! with regular expressions. See `docs/specification_index.md` for the record
//! shapes and the reasoning behind them.
//!
//! The walk mirrors the linter: we visit the tree once and dispatch to
//! collectors by node kind. The one piece of state the walk carries that the
//! linter does not need is the scope stack, because per-scope shadowing and
//! inner classes are the whole point of the output.

use std::collections::HashMap;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use tree_sitter::Node;

use crate::FormatterConfiguration;
use crate::linter::lib::{SourceRange, get_node_text};
use crate::node_kind::GDScriptNodeKind;
use crate::parser::ParseInput;

pub mod collectors;

#[cfg(test)]
mod tests;

use collectors::{ALL_COLLECTORS, CollectorContext};

/// Bumped on any breaking change to record shapes or field meanings. Adding a
/// record kind or an optional field is not breaking.
pub const INDEX_SCHEMA_VERSION: usize = 1;

/// Writes the index for one file: a `file` header record followed by one line
/// per content record. Returns false when the source has parse errors, in which
/// case only the header is written.
///
/// The header goes out either way so a consumer can tell "no records because
/// the file is broken" from "no records because nothing matched".
pub fn index_source(source: &str, display_path: &str, output: &mut String) -> bool {
    let config = FormatterConfiguration::default();
    let Some(parsed) = ParseInput::new(source, &config) else {
        write_file_header_record(display_path, true, output);
        return false;
    };

    if parsed.has_parse_errors {
        write_file_header_record(display_path, true, output);
        return false;
    }

    write_file_header_record(display_path, false, output);

    let collectors_by_node_kind = build_collector_lookup();
    let mut state = IndexWalkState {
        source,
        collectors_by_node_kind: &collectors_by_node_kind,
        current_scope: String::new(),
    };
    visit_node(&parsed.tree.root_node(), &mut state, output);
    true
}

/// Indexes every file and writes the result to stdout. Returns true when at
/// least one file failed to parse; the caller turns that into an exit code.
///
/// Diagnostics go to stderr so stdout stays parseable.
pub fn index_gdscript_files(input_files: &[PathBuf]) -> Result<bool, Box<dyn std::error::Error>> {
    let mut standard_output = BufWriter::new(std::io::stdout().lock());
    let mut project_root_cache: HashMap<PathBuf, Option<PathBuf>> = HashMap::new();
    let mut record_buffer = String::new();
    let mut had_parse_errors = false;

    for file_path in input_files {
        let source = fs::read_to_string(file_path)
            .map_err(|error| format!("Failed to read file {}: {}", file_path.display(), error))?;
        let display_path = build_display_path(file_path, &mut project_root_cache);

        record_buffer.clear();
        let parsed_without_errors = index_source(&source, &display_path, &mut record_buffer);
        if !parsed_without_errors {
            had_parse_errors = true;
            eprintln!(
                "Failed to index {}: the GDScript code contains parse errors",
                file_path.display()
            );
        }
        standard_output.write_all(record_buffer.as_bytes())?;
    }

    standard_output.flush()?;
    Ok(had_parse_errors)
}

/// Turns a file system path into the `res://` path Godot uses, when the file
/// sits inside a Godot project. The consumer of the index matches records
/// against scripts it loaded from the engine, and the engine only knows
/// `res://` paths.
///
/// Falls back to the path as given when no `project.godot` is found above the
/// file, so the sub-command stays useful outside a project.
fn build_display_path(
    file_path: &Path,
    project_root_cache: &mut HashMap<PathBuf, Option<PathBuf>>,
) -> String {
    let absolute_path = match fs::canonicalize(file_path) {
        Ok(absolute_path) => absolute_path,
        Err(_) => return file_path.display().to_string(),
    };
    let Some(directory) = absolute_path.parent() else {
        return file_path.display().to_string();
    };

    if !project_root_cache.contains_key(directory) {
        let mut project_root = None;
        let mut current_directory = Some(directory);
        while let Some(candidate_directory) = current_directory {
            if candidate_directory.join("project.godot").is_file() {
                project_root = Some(candidate_directory.to_path_buf());
                break;
            }
            current_directory = candidate_directory.parent();
        }
        project_root_cache.insert(directory.to_path_buf(), project_root);
    }

    let Some(Some(project_root)) = project_root_cache.get(directory) else {
        return file_path.display().to_string();
    };
    let Ok(relative_path) = absolute_path.strip_prefix(project_root) else {
        return file_path.display().to_string();
    };

    let mut display_path = String::from("res://");
    display_path.push_str(&relative_path.to_string_lossy().replace('\\', "/"));
    display_path
}

fn write_file_header_record(display_path: &str, has_parse_error: bool, output: &mut String) {
    output.push_str("{\"record\":\"file\"");
    json_push_usize_field("schema", INDEX_SCHEMA_VERSION, output);
    json_push_string_field("path", display_path, output);
    if has_parse_error {
        json_push_bool_field("parse_error", true, output);
    }
    output.push_str("}\n");
}

struct IndexWalkState<'a> {
    source: &'a str,
    collectors_by_node_kind: &'a HashMap<GDScriptNodeKind, Vec<usize>>,
    /// Dot separated names of the declarations we are currently inside, empty
    /// at file level. Mutated in place while walking so we never rebuild it.
    current_scope: String,
}

fn build_collector_lookup() -> HashMap<GDScriptNodeKind, Vec<usize>> {
    let mut collectors_by_node_kind: HashMap<GDScriptNodeKind, Vec<usize>> = HashMap::new();
    for (collector_index, collector) in ALL_COLLECTORS.iter().enumerate() {
        for &node_kind in collector.target_node_kinds {
            collectors_by_node_kind
                .entry(node_kind)
                .or_default()
                .push(collector_index);
        }
    }
    collectors_by_node_kind
}

fn visit_node(node: &Node, state: &mut IndexWalkState, output: &mut String) {
    let node_kind = GDScriptNodeKind::get_kind_from_ast_node(*node);
    let source = state.source;

    if let Some(collector_indices) = state.collectors_by_node_kind.get(&node_kind) {
        let context = CollectorContext {
            source,
            scope: &state.current_scope,
        };
        for &collector_index in collector_indices {
            (ALL_COLLECTORS[collector_index].collect)(node, &context, output);
        }
    }

    // The declaration record above was written with the scope that encloses
    // this declaration. Everything below it, including its own parameters, is
    // inside it.
    let scope_length_before_push = state.current_scope.len();
    if let Some(scope_name) = get_scope_name(node, node_kind, source) {
        if !state.current_scope.is_empty() {
            state.current_scope.push('.');
        }
        state.current_scope.push_str(scope_name);
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            visit_node(&cursor.node(), state, output);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    state.current_scope.truncate(scope_length_before_push);
}

/// Returns the name this node contributes to the scope of everything inside it,
/// or None when the node does not open a scope.
///
/// Enums and signals open a scope so that their members and parameters do not
/// look like declarations at the level above them.
///
/// Lambdas are deliberately absent: they are expressions, they often have no
/// name, and giving half of them a scope segment would make the output harder
/// to reason about than leaving their parameters in the enclosing scope.
fn get_scope_name<'a>(
    node: &Node,
    node_kind: GDScriptNodeKind,
    source: &'a str,
) -> Option<&'a str> {
    match node_kind {
        GDScriptNodeKind::Function
        | GDScriptNodeKind::ClassDefinition
        | GDScriptNodeKind::InnerClass
        | GDScriptNodeKind::Enum
        | GDScriptNodeKind::Signal => {
            let name_node = node.child_by_field_name("name")?;
            Some(get_node_text(&name_node, source))
        }
        GDScriptNodeKind::Constructor => Some("_init"),
        _ => None,
    }
}

// JSON writing. We hand-write it rather than pulling in serde: the record
// shapes are fixed and few, and the formatter avoids external dependencies.
//
// Every record starts with its `record` field, so every other field is written
// with a leading comma and no separator bookkeeping is needed anywhere.

pub fn json_push_escaped_string(value: &str, output: &mut String) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0C}' => output.push_str("\\f"),
            _ => {
                if (character as u32) < 0x20 {
                    output.push_str(&format!("\\u{:04x}", character as u32));
                } else {
                    output.push(character);
                }
            }
        }
    }
    output.push('"');
}

pub fn json_push_field_name(name: &str, output: &mut String) {
    output.push_str(",\"");
    output.push_str(name);
    output.push_str("\":");
}

pub fn json_push_string_field(name: &str, value: &str, output: &mut String) {
    json_push_field_name(name, output);
    json_push_escaped_string(value, output);
}

pub fn json_push_bool_field(name: &str, value: bool, output: &mut String) {
    json_push_field_name(name, output);
    output.push_str(if value { "true" } else { "false" });
}

pub fn json_push_usize_field(name: &str, value: usize, output: &mut String) {
    json_push_field_name(name, output);
    output.push_str(&value.to_string());
}

pub fn json_push_range(range: &SourceRange, output: &mut String) {
    output.push_str("{\"start_row\":");
    output.push_str(&range.start_row.to_string());
    output.push_str(",\"start_column\":");
    output.push_str(&range.start_column.to_string());
    output.push_str(",\"end_row\":");
    output.push_str(&range.end_row.to_string());
    output.push_str(",\"end_column\":");
    output.push_str(&range.end_column.to_string());
    output.push_str(",\"start_byte\":");
    output.push_str(&range.start_byte.to_string());
    output.push_str(",\"end_byte\":");
    output.push_str(&range.end_byte.to_string());
    output.push('}');
}

pub fn json_push_range_field(name: &str, range: &SourceRange, output: &mut String) {
    json_push_field_name(name, output);
    json_push_range(range, output);
}
