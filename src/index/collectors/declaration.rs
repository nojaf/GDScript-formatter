//! Emits one `declaration` record per named thing in the file.
//!
//! Annotations and modifiers are fields rather than something the consumer has
//! to recognise in source text. That is the point of the record: an
//! `@abstract func` is a declaration with an annotation, not a line that a
//! regular expression failed to match.

use tree_sitter::Node;

use crate::index::collectors::{
    CollectorContext, find_first_child_of_kind, find_first_named_child_of_kind, write_scope_field,
};
use crate::index::{
    json_push_bool_field, json_push_escaped_string, json_push_field_name, json_push_range_field,
    json_push_string_field,
};
use crate::linter::lib::{SourceRange, get_node_text, get_range};
use crate::node_kind::GDScriptNodeKind;

pub const TARGET_NODE_KINDS: &[GDScriptNodeKind] = &[
    GDScriptNodeKind::ClassName,
    GDScriptNodeKind::ClassDefinition,
    GDScriptNodeKind::InnerClass,
    GDScriptNodeKind::Variable,
    GDScriptNodeKind::ExportVariable,
    GDScriptNodeKind::OnReadyVariable,
    GDScriptNodeKind::Const,
    GDScriptNodeKind::Enum,
    GDScriptNodeKind::Enumerator,
    GDScriptNodeKind::Signal,
    GDScriptNodeKind::Function,
    GDScriptNodeKind::Constructor,
    GDScriptNodeKind::Lambda,
    GDScriptNodeKind::Parameters,
    GDScriptNodeKind::ForStatement,
];

pub struct AnnotationRecord<'a> {
    pub name: &'a str,
    pub arguments: Vec<&'a str>,
    pub range: SourceRange,
}

pub struct ParameterRecord<'a> {
    pub name: &'a str,
    pub declared_type: Option<&'a str>,
    pub default_value: Option<&'a str>,
    /// The parameter as written, type and default included.
    pub range: SourceRange,
    /// The parameter name alone.
    pub name_range: SourceRange,
}

pub struct DeclarationRecord<'a> {
    pub kind: &'static str,
    pub name: &'a str,
    /// The whole declaration, annotations included.
    pub range: SourceRange,
    /// The identifier alone.
    pub name_range: SourceRange,
    pub annotations: Vec<AnnotationRecord<'a>>,
    pub is_static: bool,
    pub is_abstract: bool,
    pub parameters: Vec<ParameterRecord<'a>>,
    /// The declared type as written, or None when there is none or when the
    /// type is inferred with `:=`.
    pub declared_type: Option<&'a str>,
    pub default_value: Option<&'a str>,
    pub body_range: Option<SourceRange>,
    pub body_is_pass_only: bool,
    /// Classes only: what the class extends, as written.
    pub extends: Option<&'a str>,
    /// True on the one class record per file that is the file's own
    /// `class_name`, false on inner classes.
    pub is_file_class: bool,
}

impl<'a> DeclarationRecord<'a> {
    fn new(kind: &'static str, name: &'a str, range: SourceRange, name_range: SourceRange) -> Self {
        Self {
            kind,
            name,
            range,
            name_range,
            annotations: Vec::new(),
            is_static: false,
            is_abstract: false,
            parameters: Vec::new(),
            declared_type: None,
            default_value: None,
            body_range: None,
            body_is_pass_only: false,
            extends: None,
            is_file_class: false,
        }
    }
}

pub fn write_annotations_field(annotations: &[AnnotationRecord], output: &mut String) {
    if annotations.is_empty() {
        return;
    }
    json_push_field_name("annotations", output);
    output.push('[');
    for (annotation_index, annotation) in annotations.iter().enumerate() {
        if annotation_index > 0 {
            output.push(',');
        }
        output.push_str("{\"name\":");
        json_push_escaped_string(annotation.name, output);
        if !annotation.arguments.is_empty() {
            json_push_field_name("arguments", output);
            output.push('[');
            for (argument_index, argument) in annotation.arguments.iter().enumerate() {
                if argument_index > 0 {
                    output.push(',');
                }
                json_push_escaped_string(argument, output);
            }
            output.push(']');
        }
        json_push_range_field("range", &annotation.range, output);
        output.push('}');
    }
    output.push(']');
}

pub fn collect(node: &Node, context: &CollectorContext, output: &mut String) {
    let node_kind = GDScriptNodeKind::get_kind_from_ast_node(*node);
    match node_kind {
        GDScriptNodeKind::ClassName
        | GDScriptNodeKind::ClassDefinition
        | GDScriptNodeKind::InnerClass => collect_class(node, node_kind, context, output),
        GDScriptNodeKind::Variable
        | GDScriptNodeKind::ExportVariable
        | GDScriptNodeKind::OnReadyVariable => collect_variable(node, "variable", context, output),
        GDScriptNodeKind::Const => collect_variable(node, "constant", context, output),
        GDScriptNodeKind::Enum => collect_enum(node, context, output),
        GDScriptNodeKind::Enumerator => collect_enum_member(node, context, output),
        GDScriptNodeKind::Signal => collect_signal(node, context, output),
        GDScriptNodeKind::Function | GDScriptNodeKind::Constructor | GDScriptNodeKind::Lambda => {
            collect_function(node, node_kind, context, output)
        }
        GDScriptNodeKind::Parameters => collect_parameters(node, context, output),
        GDScriptNodeKind::ForStatement => collect_loop_variable(node, context, output),
        _ => {}
    }
}

/// A `class_name` statement and an inner class both report `kind: "class"` at
/// file scope, and nothing else on the record separates them. Position does not
/// decide it either: a file with inner classes and no `class_name` would hand
/// back an inner class in answer to "what does this file declare". The node kind
/// does, so the record says it outright.
fn collect_class(
    node: &Node,
    node_kind: GDScriptNodeKind,
    context: &CollectorContext,
    output: &mut String,
) {
    let Some(name_node) = node.child_by_field_name("name") else {
        return;
    };
    let mut record = DeclarationRecord::new(
        "class",
        get_node_text(&name_node, context.source),
        get_range(node),
        get_range(&name_node),
    );
    collect_annotations(node, context.source, &mut record.annotations);
    record.range = get_declaration_range(node, &record.annotations);
    record.extends = find_extends_for_class(node, node_kind, context.source);
    record.is_file_class = node_kind == GDScriptNodeKind::ClassName;
    write_declaration_record(&record, context.scope, output);
}

fn collect_variable(
    node: &Node,
    kind: &'static str,
    context: &CollectorContext,
    output: &mut String,
) {
    let Some(name_node) = node.child_by_field_name("name") else {
        return;
    };
    let mut record = DeclarationRecord::new(
        kind,
        get_node_text(&name_node, context.source),
        get_range(node),
        get_range(&name_node),
    );
    collect_annotations(node, context.source, &mut record.annotations);
    record.range = get_declaration_range(node, &record.annotations);
    record.is_abstract = has_annotation_named(&record.annotations, "abstract");
    record.is_static = has_static_modifier(node);
    record.declared_type = read_declared_type(node, context.source);
    record.default_value = read_field_text(node, "value", context.source);
    write_declaration_record(&record, context.scope, output);
}

fn collect_enum(node: &Node, context: &CollectorContext, output: &mut String) {
    // An enum without a name declares its members straight into the enclosing
    // scope, so there is nothing to name here.
    let Some(name_node) = node.child_by_field_name("name") else {
        return;
    };
    let mut record = DeclarationRecord::new(
        "enum",
        get_node_text(&name_node, context.source),
        get_range(node),
        get_range(&name_node),
    );
    collect_annotations(node, context.source, &mut record.annotations);
    record.range = get_declaration_range(node, &record.annotations);
    write_declaration_record(&record, context.scope, output);
}

fn collect_enum_member(node: &Node, context: &CollectorContext, output: &mut String) {
    let Some(name_node) = node.child_by_field_name("left") else {
        return;
    };
    let mut record = DeclarationRecord::new(
        "enum_member",
        get_node_text(&name_node, context.source),
        get_range(node),
        get_range(&name_node),
    );
    record.default_value = read_field_text(node, "right", context.source);
    write_declaration_record(&record, context.scope, output);
}

fn collect_signal(node: &Node, context: &CollectorContext, output: &mut String) {
    let Some(name_node) = node.child_by_field_name("name") else {
        return;
    };
    let mut record = DeclarationRecord::new(
        "signal",
        get_node_text(&name_node, context.source),
        get_range(node),
        get_range(&name_node),
    );
    collect_annotations(node, context.source, &mut record.annotations);
    record.range = get_declaration_range(node, &record.annotations);
    if let Some(parameters_node) = node.child_by_field_name("parameters") {
        collect_parameter_records(&parameters_node, context.source, &mut record.parameters);
    }
    write_declaration_record(&record, context.scope, output);
}

fn collect_function(
    node: &Node,
    node_kind: GDScriptNodeKind,
    context: &CollectorContext,
    output: &mut String,
) {
    // A constructor has no name field: `_init` is a keyword in the grammar.
    // An anonymous lambda has no name to report at all.
    let name_node = match node_kind {
        // `_init` is a keyword rather than a name node, and keywords are
        // anonymous, so this has to look past the named children.
        GDScriptNodeKind::Constructor => find_first_child_of_kind(node, GDScriptNodeKind::NameInit),
        _ => node.child_by_field_name("name"),
    };
    let Some(name_node) = name_node else {
        return;
    };

    let mut record = DeclarationRecord::new(
        "function",
        get_node_text(&name_node, context.source),
        get_range(node),
        get_range(&name_node),
    );
    collect_annotations(node, context.source, &mut record.annotations);
    record.range = get_declaration_range(node, &record.annotations);
    record.is_abstract = has_annotation_named(&record.annotations, "abstract");
    record.is_static = has_static_modifier(node);
    record.declared_type = read_field_text(node, "return_type", context.source);
    if let Some(parameters_node) = node.child_by_field_name("parameters") {
        collect_parameter_records(&parameters_node, context.source, &mut record.parameters);
    }
    if let Some(body_node) = node.child_by_field_name("body") {
        record.body_range = Some(get_range(&body_node));
        record.body_is_pass_only = is_body_pass_only(&body_node);
    }
    write_declaration_record(&record, context.scope, output);
}

/// Emits one record per parameter. Parameters are declarations in their own
/// right so that a consumer can resolve an occurrence of a name to a parameter
/// instead of to a member that happens to share it.
fn collect_parameters(node: &Node, context: &CollectorContext, output: &mut String) {
    let mut parameters = Vec::with_capacity(node.named_child_count());
    collect_parameter_records(node, context.source, &mut parameters);
    for parameter in &parameters {
        let mut record = DeclarationRecord::new(
            "parameter",
            parameter.name,
            parameter.range,
            parameter.name_range,
        );
        record.declared_type = parameter.declared_type;
        record.default_value = parameter.default_value;
        write_declaration_record(&record, context.scope, output);
    }
}

/// A `for` binding declares a name that lives for the loop. The grammar has no
/// declaration node for it, but a consumer resolving names per scope needs it
/// as much as it needs a `var`.
fn collect_loop_variable(node: &Node, context: &CollectorContext, output: &mut String) {
    let Some(name_node) = node.child_by_field_name("left") else {
        return;
    };
    let mut record = DeclarationRecord::new(
        "variable",
        get_node_text(&name_node, context.source),
        get_range(&name_node),
        get_range(&name_node),
    );
    record.declared_type = read_declared_type(node, context.source);
    write_declaration_record(&record, context.scope, output);
}

fn collect_parameter_records<'a>(
    parameters_node: &Node,
    source: &'a str,
    parameters: &mut Vec<ParameterRecord<'a>>,
) {
    for child_index in 0..parameters_node.named_child_count() {
        let Some(child) = parameters_node.named_child(child_index as u32) else {
            continue;
        };
        let child_kind = GDScriptNodeKind::get_kind_from_ast_node(child);
        if child_kind == GDScriptNodeKind::Identifier {
            parameters.push(ParameterRecord {
                name: get_node_text(&child, source),
                declared_type: None,
                default_value: None,
                range: get_range(&child),
                name_range: get_range(&child),
            });
            continue;
        }
        if !matches!(
            child_kind,
            GDScriptNodeKind::Parameter | GDScriptNodeKind::VariadicParameter
        ) {
            continue;
        }
        let Some(name_node) = find_first_named_child_of_kind(&child, GDScriptNodeKind::Identifier)
        else {
            continue;
        };
        parameters.push(ParameterRecord {
            name: get_node_text(&name_node, source),
            declared_type: read_declared_type(&child, source),
            default_value: read_field_text(&child, "value", source),
            range: get_range(&child),
            name_range: get_range(&name_node),
        });
    }
}

/// True when a declaration of this kind takes the annotations written above it.
///
/// The file header claims the ones this leaves behind, so that between them no
/// annotation is reported twice and none is dropped.
pub fn absorbs_preceding_annotations(node_kind: GDScriptNodeKind) -> bool {
    matches!(
        node_kind,
        GDScriptNodeKind::ClassName
            | GDScriptNodeKind::ClassDefinition
            | GDScriptNodeKind::InnerClass
            | GDScriptNodeKind::Variable
            | GDScriptNodeKind::ExportVariable
            | GDScriptNodeKind::OnReadyVariable
            | GDScriptNodeKind::Const
            | GDScriptNodeKind::Enum
            | GDScriptNodeKind::Signal
            | GDScriptNodeKind::Function
            | GDScriptNodeKind::Constructor
    )
}

/// Reads every annotation that applies to this declaration, in source order.
///
/// The grammar attaches an annotation written on the same line to the
/// declaration and leaves one written on its own line as a sibling in front of
/// it. Both apply to the declaration in Godot, so both belong on the record.
/// Reading only the attached ones drops `@abstract` written above a function,
/// which is the same bug in a different spelling as the one this index exists
/// to end.
fn collect_annotations<'a>(
    node: &Node,
    source: &'a str,
    annotations: &mut Vec<AnnotationRecord<'a>>,
) {
    // Walk back over the annotations written above the declaration. Comments
    // can sit between them and the declaration without breaking the binding.
    let mut annotations_above = Vec::new();
    let mut previous_sibling = node.prev_named_sibling();
    while let Some(sibling) = previous_sibling {
        let sibling_kind = GDScriptNodeKind::get_kind_from_ast_node(sibling);
        if sibling_kind == GDScriptNodeKind::Comment {
            previous_sibling = sibling.prev_named_sibling();
            continue;
        }
        if sibling_kind != GDScriptNodeKind::Annotation {
            break;
        }
        annotations_above.push(sibling);
        previous_sibling = sibling.prev_named_sibling();
    }
    for annotation_index in (0..annotations_above.len()).rev() {
        push_annotation(&annotations_above[annotation_index], source, annotations);
    }

    let Some(annotations_node) =
        find_first_named_child_of_kind(node, GDScriptNodeKind::Annotations)
    else {
        return;
    };
    for child_index in 0..annotations_node.named_child_count() {
        let Some(annotation_node) = annotations_node.named_child(child_index as u32) else {
            continue;
        };
        if GDScriptNodeKind::get_kind_from_ast_node(annotation_node) != GDScriptNodeKind::Annotation
        {
            continue;
        }
        push_annotation(&annotation_node, source, annotations);
    }
}

pub fn push_annotation<'a>(
    annotation_node: &Node,
    source: &'a str,
    annotations: &mut Vec<AnnotationRecord<'a>>,
) {
    let Some(name_node) =
        find_first_named_child_of_kind(annotation_node, GDScriptNodeKind::Identifier)
    else {
        return;
    };
    let mut arguments = Vec::new();
    if let Some(arguments_node) = annotation_node.child_by_field_name("arguments") {
        for argument_index in 0..arguments_node.named_child_count() {
            let Some(argument_node) = arguments_node.named_child(argument_index as u32) else {
                continue;
            };
            arguments.push(get_node_text(&argument_node, source).trim());
        }
    }
    annotations.push(AnnotationRecord {
        name: get_node_text(&name_node, source),
        arguments,
        range: get_range(annotation_node),
    });
}

/// The declaration's range covers its annotations, including the ones written
/// above it that the grammar leaves outside the declaration node.
fn get_declaration_range(node: &Node, annotations: &[AnnotationRecord]) -> SourceRange {
    let mut range = get_range(node);
    let Some(first_annotation) = annotations.first() else {
        return range;
    };
    if first_annotation.range.start_byte >= range.start_byte {
        return range;
    }
    range.start_row = first_annotation.range.start_row;
    range.start_column = first_annotation.range.start_column;
    range.start_byte = first_annotation.range.start_byte;
    range
}

/// Reads what a class extends, as written: a class name such as `Node` or
/// `Qux.Inner`, or a `"res://path.gd"` string with its quotes.
///
/// The string form keeps its quotes because every other source-text field does,
/// and the `string_literal` record for that same string already carries the
/// path with its escapes resolved.
pub fn read_extends_text<'a>(extends_statement: &Node, source: &'a str) -> Option<&'a str> {
    for child_index in 0..extends_statement.named_child_count() {
        let child = extends_statement.named_child(child_index as u32)?;
        if matches!(
            GDScriptNodeKind::get_kind_from_ast_node(child),
            GDScriptNodeKind::Type | GDScriptNodeKind::String
        ) {
            return Some(get_node_text(&child, source).trim());
        }
    }
    None
}

/// Finds the `extends` that applies to a class declaration.
///
/// Written on the class line, `class_name Foo extends Bar` or
/// `class Inner extends Bar:`, the grammar nests it in the declaration and
/// there is nothing to search for.
///
/// Written on its own line it lands somewhere different for each of the two,
/// and where it is decides whose base it is. The file's `extends` is a sibling
/// of the `class_name` statement, and an inner class's is a statement in that
/// class's body. Looking in the wrong one of those hands an inner class the
/// file's base, which is a wrong answer rather than a missing one: the consumer
/// would check members against a class the code never inherits from.
fn find_extends_for_class<'a>(
    node: &Node,
    node_kind: GDScriptNodeKind,
    source: &'a str,
) -> Option<&'a str> {
    if let Some(extends_statement) = node.child_by_field_name("extends") {
        return read_extends_text(&extends_statement, source);
    }
    let container = if node_kind == GDScriptNodeKind::ClassName {
        node.parent()?
    } else {
        node.child_by_field_name("body")?
    };
    let extends_statement = find_first_named_child_of_kind(&container, GDScriptNodeKind::Extends)?;
    read_extends_text(&extends_statement, source)
}

fn has_annotation_named(annotations: &[AnnotationRecord], name: &str) -> bool {
    for annotation in annotations {
        if annotation.name == name {
            return true;
        }
    }
    false
}

/// `static var` puts the keyword in a field, `static func` does not.
fn has_static_modifier(node: &Node) -> bool {
    if node.child_by_field_name("static").is_some() {
        return true;
    }
    find_first_named_child_of_kind(node, GDScriptNodeKind::KeywordStatic).is_some()
}

/// Returns the type as written. `:=` is an inferred type: nothing was written,
/// and tree-sitter cannot resolve what it would be, so there is nothing to
/// report.
fn read_declared_type<'a>(node: &Node, source: &'a str) -> Option<&'a str> {
    let type_node = node.child_by_field_name("type")?;
    if GDScriptNodeKind::get_kind_from_ast_node(type_node) == GDScriptNodeKind::InferredType {
        return None;
    }
    Some(get_node_text(&type_node, source).trim())
}

fn read_field_text<'a>(node: &Node, field_name: &str, source: &'a str) -> Option<&'a str> {
    let field_node = node.child_by_field_name(field_name)?;
    Some(get_node_text(&field_node, source).trim())
}

fn is_body_pass_only(body_node: &Node) -> bool {
    let mut statement_count = 0;
    let mut pass_count = 0;
    for child_index in 0..body_node.named_child_count() {
        let Some(child) = body_node.named_child(child_index as u32) else {
            continue;
        };
        let child_kind = GDScriptNodeKind::get_kind_from_ast_node(child);
        if child_kind == GDScriptNodeKind::Comment {
            continue;
        }
        statement_count += 1;
        if child_kind == GDScriptNodeKind::PassStatement {
            pass_count += 1;
        }
    }
    statement_count > 0 && statement_count == pass_count
}

fn write_declaration_record(record: &DeclarationRecord, scope: &str, output: &mut String) {
    output.push_str("{\"record\":\"declaration\"");
    json_push_string_field("kind", record.kind, output);
    json_push_string_field("name", record.name, output);
    write_scope_field(scope, output);
    json_push_range_field("range", &record.range, output);
    json_push_range_field("name_range", &record.name_range, output);

    if let Some(declared_type) = record.declared_type {
        json_push_string_field("type", declared_type, output);
    }
    if let Some(default_value) = record.default_value {
        json_push_string_field("default", default_value, output);
    }
    if let Some(extends) = record.extends {
        json_push_string_field("extends", extends, output);
    }
    if record.is_file_class {
        json_push_bool_field("is_file_class", true, output);
    }

    write_annotations_field(&record.annotations, output);

    if record.is_static || record.is_abstract {
        json_push_field_name("modifiers", output);
        output.push('[');
        if record.is_static {
            output.push_str("\"static\"");
        }
        if record.is_abstract {
            if record.is_static {
                output.push(',');
            }
            output.push_str("\"abstract\"");
        }
        output.push(']');
    }

    if !record.parameters.is_empty() {
        json_push_field_name("parameters", output);
        output.push('[');
        for (parameter_index, parameter) in record.parameters.iter().enumerate() {
            if parameter_index > 0 {
                output.push(',');
            }
            output.push_str("{\"name\":");
            json_push_escaped_string(parameter.name, output);
            if let Some(declared_type) = parameter.declared_type {
                json_push_string_field("type", declared_type, output);
            }
            if let Some(default_value) = parameter.default_value {
                json_push_string_field("default", default_value, output);
            }
            json_push_range_field("range", &parameter.range, output);
            output.push('}');
        }
        output.push(']');
    }

    // A function always reports `body_range`, null when it has no body. Every
    // other absent field means empty; this one has to separate "no body" from
    // "not a function", and a consumer cannot read that off an absence.
    if record.kind == "function" {
        if let Some(body_range) = record.body_range {
            json_push_range_field("body_range", &body_range, output);
        } else {
            json_push_field_name("body_range", output);
            output.push_str("null");
        }
    } else if let Some(body_range) = record.body_range {
        json_push_range_field("body_range", &body_range, output);
    }
    if record.body_is_pass_only {
        json_push_bool_field("body_is_pass_only", true, output);
    }

    output.push_str("}\n");
}
