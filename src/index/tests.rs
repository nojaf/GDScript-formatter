#[cfg(test)]
mod index_tests {
    #![allow(clippy::unwrap_used)]
    use crate::FormatterConfiguration;
    use crate::index::index_source;
    use crate::linter::lib::get_range;
    use crate::node_kind::GDScriptNodeKind;
    use crate::parser::ParseInput;

    /// Indexes one source and returns every line it wrote.
    ///
    /// The tests below compare whole records rather than counting them: a wrong
    /// range is invisible in any test that only checks names, and ranges are
    /// the part most likely to drift when the grammar updates.
    fn index_to_string(source: &str) -> String {
        let mut output = String::new();
        let parsed_without_errors = index_source(source, "res://test.gd", &mut output);
        assert!(parsed_without_errors, "the fixture should parse");
        output
    }

    #[test]
    fn test_worked_example_from_the_specification() {
        let source = r#####"class_name Hud
extends CanvasLayer

@onready var clock: Label = $Clock

func _process(_delta: float) -> void:
	self.clock.ziggy = "x"
	self.call("late_bound")
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd"}
{"record":"declaration","kind":"class","name":"Hud","scope":"","range":{"start_row":1,"start_column":1,"end_row":1,"end_column":15,"start_byte":0,"end_byte":14},"name_range":{"start_row":1,"start_column":12,"end_row":1,"end_column":15,"start_byte":11,"end_byte":14}}
{"record":"reference","name":"CanvasLayer","scope":"","range":{"start_row":2,"start_column":9,"end_row":2,"end_column":20,"start_byte":23,"end_byte":34},"name_range":{"start_row":2,"start_column":9,"end_row":2,"end_column":20,"start_byte":23,"end_byte":34},"context":"type"}
{"record":"declaration","kind":"variable","name":"clock","scope":"","range":{"start_row":4,"start_column":1,"end_row":4,"end_column":35,"start_byte":36,"end_byte":70},"name_range":{"start_row":4,"start_column":14,"end_row":4,"end_column":19,"start_byte":49,"end_byte":54},"type":"Label","default":"$Clock","annotations":[{"name":"onready","range":{"start_row":4,"start_column":1,"end_row":4,"end_column":9,"start_byte":36,"end_byte":44}}]}
{"record":"reference","name":"Label","scope":"","range":{"start_row":4,"start_column":21,"end_row":4,"end_column":26,"start_byte":56,"end_byte":61},"name_range":{"start_row":4,"start_column":21,"end_row":4,"end_column":26,"start_byte":56,"end_byte":61},"context":"type"}
{"record":"declaration","kind":"function","name":"_process","scope":"","range":{"start_row":6,"start_column":1,"end_row":8,"end_column":25,"start_byte":72,"end_byte":158},"name_range":{"start_row":6,"start_column":6,"end_row":6,"end_column":14,"start_byte":77,"end_byte":85},"type":"void","parameters":[{"name":"_delta","type":"float","range":{"start_row":6,"start_column":15,"end_row":6,"end_column":28,"start_byte":86,"end_byte":99}}],"body_range":{"start_row":6,"start_column":38,"end_row":8,"end_column":25,"start_byte":109,"end_byte":158}}
{"record":"declaration","kind":"parameter","name":"_delta","scope":"_process","range":{"start_row":6,"start_column":15,"end_row":6,"end_column":28,"start_byte":86,"end_byte":99},"name_range":{"start_row":6,"start_column":15,"end_row":6,"end_column":21,"start_byte":86,"end_byte":92},"type":"float"}
{"record":"reference","name":"float","scope":"_process","range":{"start_row":6,"start_column":23,"end_row":6,"end_column":28,"start_byte":94,"end_byte":99},"name_range":{"start_row":6,"start_column":23,"end_row":6,"end_column":28,"start_byte":94,"end_byte":99},"context":"type"}
{"record":"reference","name":"void","scope":"_process","range":{"start_row":6,"start_column":33,"end_row":6,"end_column":37,"start_byte":104,"end_byte":108},"name_range":{"start_row":6,"start_column":33,"end_row":6,"end_column":37,"start_byte":104,"end_byte":108},"context":"type"}
{"record":"member_chain","segments":[{"name":"self","range":{"start_row":7,"start_column":2,"end_row":7,"end_column":6,"start_byte":111,"end_byte":115}},{"name":"clock","range":{"start_row":7,"start_column":7,"end_row":7,"end_column":12,"start_byte":116,"end_byte":121}},{"name":"ziggy","range":{"start_row":7,"start_column":13,"end_row":7,"end_column":18,"start_byte":122,"end_byte":127}}],"scope":"_process","range":{"start_row":7,"start_column":2,"end_row":7,"end_column":18,"start_byte":111,"end_byte":127},"context":"assignment_target"}
{"record":"string_literal","value":"x","scope":"_process","range":{"start_row":7,"start_column":21,"end_row":7,"end_column":24,"start_byte":130,"end_byte":133}}
{"record":"member_chain","segments":[{"name":"self","range":{"start_row":8,"start_column":2,"end_row":8,"end_column":6,"start_byte":135,"end_byte":139}},{"name":"call","range":{"start_row":8,"start_column":7,"end_row":8,"end_column":11,"start_byte":140,"end_byte":144}}],"scope":"_process","range":{"start_row":8,"start_column":2,"end_row":8,"end_column":25,"start_byte":135,"end_byte":158},"is_call":true,"arguments":[{"text":"\"late_bound\"","range":{"start_row":8,"start_column":12,"end_row":8,"end_column":24,"start_byte":145,"end_byte":157}}],"context":"statement"}
{"record":"string_literal","value":"late_bound","scope":"_process","range":{"start_row":8,"start_column":12,"end_row":8,"end_column":24,"start_byte":145,"end_byte":157},"argument_of":{"callee":"call","index":0}}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    #[test]
    fn test_annotations_and_modifiers_are_fields() {
        let source = r#####"@abstract func may_target(candidate: Node) -> bool

@export_range(0, 10) static var health := 5

static func helper() -> void:
	pass
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd"}
{"record":"declaration","kind":"function","name":"may_target","scope":"","range":{"start_row":1,"start_column":1,"end_row":1,"end_column":51,"start_byte":0,"end_byte":50},"name_range":{"start_row":1,"start_column":16,"end_row":1,"end_column":26,"start_byte":15,"end_byte":25},"type":"bool","annotations":[{"name":"abstract","range":{"start_row":1,"start_column":1,"end_row":1,"end_column":10,"start_byte":0,"end_byte":9}}],"modifiers":["abstract"],"parameters":[{"name":"candidate","type":"Node","range":{"start_row":1,"start_column":27,"end_row":1,"end_column":42,"start_byte":26,"end_byte":41}}]}
{"record":"declaration","kind":"parameter","name":"candidate","scope":"may_target","range":{"start_row":1,"start_column":27,"end_row":1,"end_column":42,"start_byte":26,"end_byte":41},"name_range":{"start_row":1,"start_column":27,"end_row":1,"end_column":36,"start_byte":26,"end_byte":35},"type":"Node"}
{"record":"reference","name":"Node","scope":"may_target","range":{"start_row":1,"start_column":38,"end_row":1,"end_column":42,"start_byte":37,"end_byte":41},"name_range":{"start_row":1,"start_column":38,"end_row":1,"end_column":42,"start_byte":37,"end_byte":41},"context":"type"}
{"record":"reference","name":"bool","scope":"may_target","range":{"start_row":1,"start_column":47,"end_row":1,"end_column":51,"start_byte":46,"end_byte":50},"name_range":{"start_row":1,"start_column":47,"end_row":1,"end_column":51,"start_byte":46,"end_byte":50},"context":"type"}
{"record":"declaration","kind":"variable","name":"health","scope":"","range":{"start_row":3,"start_column":1,"end_row":3,"end_column":44,"start_byte":52,"end_byte":95},"name_range":{"start_row":3,"start_column":33,"end_row":3,"end_column":39,"start_byte":84,"end_byte":90},"default":"5","annotations":[{"name":"export_range","arguments":["0","10"],"range":{"start_row":3,"start_column":1,"end_row":3,"end_column":21,"start_byte":52,"end_byte":72}}],"modifiers":["static"]}
{"record":"declaration","kind":"function","name":"helper","scope":"","range":{"start_row":5,"start_column":1,"end_row":6,"end_column":6,"start_byte":97,"end_byte":132},"name_range":{"start_row":5,"start_column":13,"end_row":5,"end_column":19,"start_byte":109,"end_byte":115},"type":"void","modifiers":["static"],"body_range":{"start_row":5,"start_column":30,"end_row":6,"end_column":6,"start_byte":126,"end_byte":132},"body_is_pass_only":true}
{"record":"reference","name":"void","scope":"helper","range":{"start_row":5,"start_column":25,"end_row":5,"end_column":29,"start_byte":121,"end_byte":125},"name_range":{"start_row":5,"start_column":25,"end_row":5,"end_column":29,"start_byte":121,"end_byte":125},"context":"type"}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    #[test]
    fn test_scopes_follow_inner_classes_and_functions() {
        let source = r#####"var target := 1

class Inner:
	func _ready() -> void:
		var target := 2
		print(target)
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd"}
{"record":"declaration","kind":"variable","name":"target","scope":"","range":{"start_row":1,"start_column":1,"end_row":1,"end_column":16,"start_byte":0,"end_byte":15},"name_range":{"start_row":1,"start_column":5,"end_row":1,"end_column":11,"start_byte":4,"end_byte":10},"default":"1"}
{"record":"declaration","kind":"class","name":"Inner","scope":"","range":{"start_row":3,"start_column":1,"end_row":6,"end_column":16,"start_byte":17,"end_byte":87},"name_range":{"start_row":3,"start_column":7,"end_row":3,"end_column":12,"start_byte":23,"end_byte":28}}
{"record":"declaration","kind":"function","name":"_ready","scope":"Inner","range":{"start_row":4,"start_column":2,"end_row":6,"end_column":16,"start_byte":31,"end_byte":87},"name_range":{"start_row":4,"start_column":7,"end_row":4,"end_column":13,"start_byte":36,"end_byte":42},"type":"void","body_range":{"start_row":4,"start_column":24,"end_row":6,"end_column":16,"start_byte":53,"end_byte":87}}
{"record":"reference","name":"void","scope":"Inner._ready","range":{"start_row":4,"start_column":19,"end_row":4,"end_column":23,"start_byte":48,"end_byte":52},"name_range":{"start_row":4,"start_column":19,"end_row":4,"end_column":23,"start_byte":48,"end_byte":52},"context":"type"}
{"record":"declaration","kind":"variable","name":"target","scope":"Inner._ready","range":{"start_row":5,"start_column":3,"end_row":5,"end_column":18,"start_byte":56,"end_byte":71},"name_range":{"start_row":5,"start_column":7,"end_row":5,"end_column":13,"start_byte":60,"end_byte":66},"default":"2"}
{"record":"reference","name":"print","scope":"Inner._ready","range":{"start_row":6,"start_column":3,"end_row":6,"end_column":16,"start_byte":74,"end_byte":87},"name_range":{"start_row":6,"start_column":3,"end_row":6,"end_column":8,"start_byte":74,"end_byte":79},"is_call":true,"arguments":[{"text":"target","range":{"start_row":6,"start_column":9,"end_row":6,"end_column":15,"start_byte":80,"end_byte":86}}],"context":"statement"}
{"record":"reference","name":"target","scope":"Inner._ready","range":{"start_row":6,"start_column":9,"end_row":6,"end_column":15,"start_byte":80,"end_byte":86},"name_range":{"start_row":6,"start_column":9,"end_row":6,"end_column":15,"start_byte":80,"end_byte":86},"context":"argument"}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    #[test]
    fn test_enum_members_live_in_the_enum_scope() {
        let source = r#####"enum State { IDLE = 0, RUN }
enum { LOOSE }
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd"}
{"record":"declaration","kind":"enum","name":"State","scope":"","range":{"start_row":1,"start_column":1,"end_row":1,"end_column":29,"start_byte":0,"end_byte":28},"name_range":{"start_row":1,"start_column":6,"end_row":1,"end_column":11,"start_byte":5,"end_byte":10}}
{"record":"declaration","kind":"enum_member","name":"IDLE","scope":"State","range":{"start_row":1,"start_column":14,"end_row":1,"end_column":22,"start_byte":13,"end_byte":21},"name_range":{"start_row":1,"start_column":14,"end_row":1,"end_column":18,"start_byte":13,"end_byte":17},"default":"0"}
{"record":"declaration","kind":"enum_member","name":"RUN","scope":"State","range":{"start_row":1,"start_column":24,"end_row":1,"end_column":27,"start_byte":23,"end_byte":26},"name_range":{"start_row":1,"start_column":24,"end_row":1,"end_column":27,"start_byte":23,"end_byte":26}}
{"record":"declaration","kind":"enum_member","name":"LOOSE","scope":"","range":{"start_row":2,"start_column":8,"end_row":2,"end_column":13,"start_byte":36,"end_byte":41},"name_range":{"start_row":2,"start_column":8,"end_row":2,"end_column":13,"start_byte":36,"end_byte":41}}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    #[test]
    fn test_string_literals_report_their_argument_position() {
        let source = r#####"func _ready() -> void:
	self.call("late_bound")
	print("all done")
	var tabbed := "a\tb"
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd"}
{"record":"declaration","kind":"function","name":"_ready","scope":"","range":{"start_row":1,"start_column":1,"end_row":4,"end_column":22,"start_byte":0,"end_byte":88},"name_range":{"start_row":1,"start_column":6,"end_row":1,"end_column":12,"start_byte":5,"end_byte":11},"type":"void","body_range":{"start_row":1,"start_column":23,"end_row":4,"end_column":22,"start_byte":22,"end_byte":88}}
{"record":"reference","name":"void","scope":"_ready","range":{"start_row":1,"start_column":18,"end_row":1,"end_column":22,"start_byte":17,"end_byte":21},"name_range":{"start_row":1,"start_column":18,"end_row":1,"end_column":22,"start_byte":17,"end_byte":21},"context":"type"}
{"record":"member_chain","segments":[{"name":"self","range":{"start_row":2,"start_column":2,"end_row":2,"end_column":6,"start_byte":24,"end_byte":28}},{"name":"call","range":{"start_row":2,"start_column":7,"end_row":2,"end_column":11,"start_byte":29,"end_byte":33}}],"scope":"_ready","range":{"start_row":2,"start_column":2,"end_row":2,"end_column":25,"start_byte":24,"end_byte":47},"is_call":true,"arguments":[{"text":"\"late_bound\"","range":{"start_row":2,"start_column":12,"end_row":2,"end_column":24,"start_byte":34,"end_byte":46}}],"context":"statement"}
{"record":"string_literal","value":"late_bound","scope":"_ready","range":{"start_row":2,"start_column":12,"end_row":2,"end_column":24,"start_byte":34,"end_byte":46},"argument_of":{"callee":"call","index":0}}
{"record":"reference","name":"print","scope":"_ready","range":{"start_row":3,"start_column":2,"end_row":3,"end_column":19,"start_byte":49,"end_byte":66},"name_range":{"start_row":3,"start_column":2,"end_row":3,"end_column":7,"start_byte":49,"end_byte":54},"is_call":true,"arguments":[{"text":"\"all done\"","range":{"start_row":3,"start_column":8,"end_row":3,"end_column":18,"start_byte":55,"end_byte":65}}],"context":"statement"}
{"record":"string_literal","value":"all done","scope":"_ready","range":{"start_row":3,"start_column":8,"end_row":3,"end_column":18,"start_byte":55,"end_byte":65},"argument_of":{"callee":"print","index":0}}
{"record":"declaration","kind":"variable","name":"tabbed","scope":"_ready","range":{"start_row":4,"start_column":2,"end_row":4,"end_column":22,"start_byte":68,"end_byte":88},"name_range":{"start_row":4,"start_column":6,"end_row":4,"end_column":12,"start_byte":72,"end_byte":78},"default":"\"a\\tb\""}
{"record":"string_literal","value":"a\tb","scope":"_ready","range":{"start_row":4,"start_column":16,"end_row":4,"end_column":22,"start_byte":82,"end_byte":88}}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    #[test]
    fn test_member_chains_report_a_base_when_it_is_not_a_name() {
        let source = r#####"func _ready() -> void:
	$Clock.text = "12:00"
	get_tree().create_timer(1.0).timeout
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd"}
{"record":"declaration","kind":"function","name":"_ready","scope":"","range":{"start_row":1,"start_column":1,"end_row":3,"end_column":38,"start_byte":0,"end_byte":83},"name_range":{"start_row":1,"start_column":6,"end_row":1,"end_column":12,"start_byte":5,"end_byte":11},"type":"void","body_range":{"start_row":1,"start_column":23,"end_row":3,"end_column":38,"start_byte":22,"end_byte":83}}
{"record":"reference","name":"void","scope":"_ready","range":{"start_row":1,"start_column":18,"end_row":1,"end_column":22,"start_byte":17,"end_byte":21},"name_range":{"start_row":1,"start_column":18,"end_row":1,"end_column":22,"start_byte":17,"end_byte":21},"context":"type"}
{"record":"member_chain","segments":[{"name":"text","range":{"start_row":2,"start_column":9,"end_row":2,"end_column":13,"start_byte":31,"end_byte":35}}],"base":{"text":"$Clock","range":{"start_row":2,"start_column":2,"end_row":2,"end_column":8,"start_byte":24,"end_byte":30}},"scope":"_ready","range":{"start_row":2,"start_column":2,"end_row":2,"end_column":13,"start_byte":24,"end_byte":35},"context":"assignment_target"}
{"record":"string_literal","value":"12:00","scope":"_ready","range":{"start_row":2,"start_column":16,"end_row":2,"end_column":23,"start_byte":38,"end_byte":45}}
{"record":"member_chain","segments":[{"name":"create_timer","range":{"start_row":3,"start_column":13,"end_row":3,"end_column":25,"start_byte":58,"end_byte":70}},{"name":"timeout","range":{"start_row":3,"start_column":31,"end_row":3,"end_column":38,"start_byte":76,"end_byte":83}}],"base":{"text":"get_tree()","range":{"start_row":3,"start_column":2,"end_row":3,"end_column":12,"start_byte":47,"end_byte":57}},"scope":"_ready","range":{"start_row":3,"start_column":2,"end_row":3,"end_column":38,"start_byte":47,"end_byte":83},"context":"statement"}
{"record":"reference","name":"get_tree","scope":"_ready","range":{"start_row":3,"start_column":2,"end_row":3,"end_column":12,"start_byte":47,"end_byte":57},"name_range":{"start_row":3,"start_column":2,"end_row":3,"end_column":10,"start_byte":47,"end_byte":55},"is_call":true,"context":"other"}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    #[test]
    fn test_comparisons_keep_both_operands() {
        let source = r#####"func _ready() -> void:
	if target == null:
		pass
	if target not in [1]:
		pass
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd"}
{"record":"declaration","kind":"function","name":"_ready","scope":"","range":{"start_row":1,"start_column":1,"end_row":5,"end_column":7,"start_byte":0,"end_byte":79},"name_range":{"start_row":1,"start_column":6,"end_row":1,"end_column":12,"start_byte":5,"end_byte":11},"type":"void","body_range":{"start_row":1,"start_column":23,"end_row":5,"end_column":7,"start_byte":22,"end_byte":79}}
{"record":"reference","name":"void","scope":"_ready","range":{"start_row":1,"start_column":18,"end_row":1,"end_column":22,"start_byte":17,"end_byte":21},"name_range":{"start_row":1,"start_column":18,"end_row":1,"end_column":22,"start_byte":17,"end_byte":21},"context":"type"}
{"record":"comparison","operator":"==","left":{"text":"target","range":{"start_row":2,"start_column":5,"end_row":2,"end_column":11,"start_byte":27,"end_byte":33}},"right":{"text":"null","range":{"start_row":2,"start_column":15,"end_row":2,"end_column":19,"start_byte":37,"end_byte":41}},"scope":"_ready","range":{"start_row":2,"start_column":5,"end_row":2,"end_column":19,"start_byte":27,"end_byte":41}}
{"record":"reference","name":"target","scope":"_ready","range":{"start_row":2,"start_column":5,"end_row":2,"end_column":11,"start_byte":27,"end_byte":33},"name_range":{"start_row":2,"start_column":5,"end_row":2,"end_column":11,"start_byte":27,"end_byte":33},"context":"other"}
{"record":"comparison","operator":"not in","left":{"text":"target","range":{"start_row":4,"start_column":5,"end_row":4,"end_column":11,"start_byte":54,"end_byte":60}},"right":{"text":"[1]","range":{"start_row":4,"start_column":19,"end_row":4,"end_column":22,"start_byte":68,"end_byte":71}},"scope":"_ready","range":{"start_row":4,"start_column":5,"end_row":4,"end_column":22,"start_byte":54,"end_byte":71}}
{"record":"reference","name":"target","scope":"_ready","range":{"start_row":4,"start_column":5,"end_row":4,"end_column":11,"start_byte":54,"end_byte":60},"name_range":{"start_row":4,"start_column":5,"end_row":4,"end_column":11,"start_byte":54,"end_byte":60},"context":"other"}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    #[test]
    fn test_comments_report_documentation_and_trailing() {
        let source = r#####"## A documented variable.
var health := 5  # trailing
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd"}
{"record":"comment","text":"## A documented variable.","scope":"","range":{"start_row":1,"start_column":1,"end_row":1,"end_column":26,"start_byte":0,"end_byte":25},"is_documentation":true}
{"record":"declaration","kind":"variable","name":"health","scope":"","range":{"start_row":2,"start_column":1,"end_row":2,"end_column":16,"start_byte":26,"end_byte":41},"name_range":{"start_row":2,"start_column":5,"end_row":2,"end_column":11,"start_byte":30,"end_byte":36},"default":"5"}
{"record":"comment","text":"# trailing","scope":"","range":{"start_row":2,"start_column":18,"end_row":2,"end_column":28,"start_byte":43,"end_byte":53},"is_trailing":true}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    #[test]
    fn test_loop_variables_are_declarations() {
        let source = r#####"func _ready() -> void:
	for item: int in [1, 2]:
		print(item)
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd"}
{"record":"declaration","kind":"function","name":"_ready","scope":"","range":{"start_row":1,"start_column":1,"end_row":3,"end_column":14,"start_byte":0,"end_byte":62},"name_range":{"start_row":1,"start_column":6,"end_row":1,"end_column":12,"start_byte":5,"end_byte":11},"type":"void","body_range":{"start_row":1,"start_column":23,"end_row":3,"end_column":14,"start_byte":22,"end_byte":62}}
{"record":"reference","name":"void","scope":"_ready","range":{"start_row":1,"start_column":18,"end_row":1,"end_column":22,"start_byte":17,"end_byte":21},"name_range":{"start_row":1,"start_column":18,"end_row":1,"end_column":22,"start_byte":17,"end_byte":21},"context":"type"}
{"record":"declaration","kind":"variable","name":"item","scope":"_ready","range":{"start_row":2,"start_column":6,"end_row":2,"end_column":10,"start_byte":28,"end_byte":32},"name_range":{"start_row":2,"start_column":6,"end_row":2,"end_column":10,"start_byte":28,"end_byte":32},"type":"int"}
{"record":"reference","name":"int","scope":"_ready","range":{"start_row":2,"start_column":12,"end_row":2,"end_column":15,"start_byte":34,"end_byte":37},"name_range":{"start_row":2,"start_column":12,"end_row":2,"end_column":15,"start_byte":34,"end_byte":37},"context":"type"}
{"record":"reference","name":"print","scope":"_ready","range":{"start_row":3,"start_column":3,"end_row":3,"end_column":14,"start_byte":51,"end_byte":62},"name_range":{"start_row":3,"start_column":3,"end_row":3,"end_column":8,"start_byte":51,"end_byte":56},"is_call":true,"arguments":[{"text":"item","range":{"start_row":3,"start_column":9,"end_row":3,"end_column":13,"start_byte":57,"end_byte":61}}],"context":"statement"}
{"record":"reference","name":"item","scope":"_ready","range":{"start_row":3,"start_column":9,"end_row":3,"end_column":13,"start_byte":57,"end_byte":61},"name_range":{"start_row":3,"start_column":9,"end_row":3,"end_column":13,"start_byte":57,"end_byte":61},"context":"argument"}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    #[test]
    fn test_a_file_that_fails_to_parse_emits_only_its_header() {
        let mut output = String::new();
        let parsed_without_errors = index_source("func (:\n", "res://broken.gd", &mut output);
        assert!(!parsed_without_errors);
        assert_eq!(
            output,
            "{\"record\":\"file\",\"schema\":1,\"path\":\"res://broken.gd\",\"parse_error\":true}\n"
        );
    }

    #[test]
    fn test_every_line_is_one_complete_json_object() {
        let source = "class_name Hud\nextends Node\n\nfunc _ready() -> void:\n\tprint(\"hi\")\n";
        for line in index_to_string(source).lines() {
            assert!(
                line.starts_with('{'),
                "line does not start an object: {}",
                line
            );
            assert!(
                line.ends_with('}'),
                "line does not close its object: {}",
                line
            );
        }
    }

    /// Rows and columns are one-based to match what `gdscript-formatter lint`
    /// prints, and byte offsets slice the original source exactly.
    #[test]
    fn test_ranges_are_one_based_and_carry_byte_offsets() {
        let source = "var health := 5\n";
        let config = FormatterConfiguration::default();
        let parsed = ParseInput::new(source, &config).unwrap();
        let variable_node = parsed.tree.root_node().named_child(0).unwrap();
        assert_eq!(
            GDScriptNodeKind::get_kind_from_ast_node(variable_node),
            GDScriptNodeKind::Variable
        );

        let range = get_range(&variable_node);
        assert_eq!(range.start_row, 1);
        assert_eq!(range.start_column, 1);
        assert_eq!(range.end_row, 1);
        assert_eq!(range.end_column, 16);
        assert_eq!(range.start_byte, 0);
        assert_eq!(range.end_byte, 15);
        assert_eq!(&source[range.start_byte..range.end_byte], "var health := 5");
    }
}
