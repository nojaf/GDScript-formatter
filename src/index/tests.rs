#[cfg(test)]
mod index_tests {
    #![allow(clippy::unwrap_used)]
    use crate::FormatterConfiguration;
    use crate::index::{index_source, resolve_project_root};
    use crate::linter::lib::get_range;
    use crate::node_kind::GDScriptNodeKind;
    use crate::parser::ParseInput;
    use std::path::PathBuf;

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
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd","extends":"CanvasLayer"}
{"record":"declaration","kind":"class","name":"Hud","scope":"","range":{"start_row":1,"start_column":1,"end_row":1,"end_column":15,"start_byte":0,"end_byte":14},"name_range":{"start_row":1,"start_column":12,"end_row":1,"end_column":15,"start_byte":11,"end_byte":14},"extends":"CanvasLayer"}
{"record":"reference","name":"CanvasLayer","scope":"","range":{"start_row":2,"start_column":9,"end_row":2,"end_column":20,"start_byte":23,"end_byte":34},"name_range":{"start_row":2,"start_column":9,"end_row":2,"end_column":20,"start_byte":23,"end_byte":34},"context":"type"}
{"record":"declaration","kind":"variable","name":"clock","scope":"","range":{"start_row":4,"start_column":1,"end_row":4,"end_column":35,"start_byte":36,"end_byte":70},"name_range":{"start_row":4,"start_column":14,"end_row":4,"end_column":19,"start_byte":49,"end_byte":54},"type":"Label","default":"$Clock","annotations":[{"name":"onready","range":{"start_row":4,"start_column":1,"end_row":4,"end_column":9,"start_byte":36,"end_byte":44}}]}
{"record":"reference","name":"Label","scope":"","range":{"start_row":4,"start_column":21,"end_row":4,"end_column":26,"start_byte":56,"end_byte":61},"name_range":{"start_row":4,"start_column":21,"end_row":4,"end_column":26,"start_byte":56,"end_byte":61},"context":"type"}
{"record":"declaration","kind":"function","name":"_process","scope":"","range":{"start_row":6,"start_column":1,"end_row":8,"end_column":25,"start_byte":72,"end_byte":158},"name_range":{"start_row":6,"start_column":6,"end_row":6,"end_column":14,"start_byte":77,"end_byte":85},"type":"void","parameters":[{"name":"_delta","type":"float","range":{"start_row":6,"start_column":15,"end_row":6,"end_column":28,"start_byte":86,"end_byte":99}}],"body_range":{"start_row":6,"start_column":38,"end_row":8,"end_column":25,"start_byte":109,"end_byte":158}}
{"record":"declaration","kind":"parameter","name":"_delta","scope":"_process","range":{"start_row":6,"start_column":15,"end_row":6,"end_column":28,"start_byte":86,"end_byte":99},"name_range":{"start_row":6,"start_column":15,"end_row":6,"end_column":21,"start_byte":86,"end_byte":92},"type":"float"}
{"record":"reference","name":"float","scope":"_process","range":{"start_row":6,"start_column":23,"end_row":6,"end_column":28,"start_byte":94,"end_byte":99},"name_range":{"start_row":6,"start_column":23,"end_row":6,"end_column":28,"start_byte":94,"end_byte":99},"context":"type"}
{"record":"reference","name":"void","scope":"_process","range":{"start_row":6,"start_column":33,"end_row":6,"end_column":37,"start_byte":104,"end_byte":108},"name_range":{"start_row":6,"start_column":33,"end_row":6,"end_column":37,"start_byte":104,"end_byte":108},"context":"type"}
{"record":"member_chain","segments":[{"kind":"self","name":"self","range":{"start_row":7,"start_column":2,"end_row":7,"end_column":6,"start_byte":111,"end_byte":115}},{"kind":"identifier","name":"clock","range":{"start_row":7,"start_column":7,"end_row":7,"end_column":12,"start_byte":116,"end_byte":121}},{"kind":"identifier","name":"ziggy","range":{"start_row":7,"start_column":13,"end_row":7,"end_column":18,"start_byte":122,"end_byte":127}}],"scope":"_process","range":{"start_row":7,"start_column":2,"end_row":7,"end_column":18,"start_byte":111,"end_byte":127},"context":"assignment_target"}
{"record":"string_literal","value":"x","scope":"_process","range":{"start_row":7,"start_column":21,"end_row":7,"end_column":24,"start_byte":130,"end_byte":133}}
{"record":"member_chain","segments":[{"kind":"self","name":"self","range":{"start_row":8,"start_column":2,"end_row":8,"end_column":6,"start_byte":135,"end_byte":139}},{"kind":"call","name":"call","is_call":true,"range":{"start_row":8,"start_column":7,"end_row":8,"end_column":25,"start_byte":140,"end_byte":158}}],"scope":"_process","range":{"start_row":8,"start_column":2,"end_row":8,"end_column":25,"start_byte":135,"end_byte":158},"is_call":true,"arguments":[{"text":"\"late_bound\"","range":{"start_row":8,"start_column":12,"end_row":8,"end_column":24,"start_byte":145,"end_byte":157}}],"context":"statement"}
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
{"record":"declaration","kind":"function","name":"may_target","scope":"","range":{"start_row":1,"start_column":1,"end_row":1,"end_column":51,"start_byte":0,"end_byte":50},"name_range":{"start_row":1,"start_column":16,"end_row":1,"end_column":26,"start_byte":15,"end_byte":25},"type":"bool","annotations":[{"name":"abstract","range":{"start_row":1,"start_column":1,"end_row":1,"end_column":10,"start_byte":0,"end_byte":9}}],"modifiers":["abstract"],"parameters":[{"name":"candidate","type":"Node","range":{"start_row":1,"start_column":27,"end_row":1,"end_column":42,"start_byte":26,"end_byte":41}}],"body_range":null}
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
{"record":"reference","name":"target","scope":"Inner._ready","range":{"start_row":6,"start_column":9,"end_row":6,"end_column":15,"start_byte":80,"end_byte":86},"name_range":{"start_row":6,"start_column":9,"end_row":6,"end_column":15,"start_byte":80,"end_byte":86},"context":"argument","argument_of":{"callee":"print","index":0}}
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
{"record":"member_chain","segments":[{"kind":"self","name":"self","range":{"start_row":2,"start_column":2,"end_row":2,"end_column":6,"start_byte":24,"end_byte":28}},{"kind":"call","name":"call","is_call":true,"range":{"start_row":2,"start_column":7,"end_row":2,"end_column":25,"start_byte":29,"end_byte":47}}],"scope":"_ready","range":{"start_row":2,"start_column":2,"end_row":2,"end_column":25,"start_byte":24,"end_byte":47},"is_call":true,"arguments":[{"text":"\"late_bound\"","range":{"start_row":2,"start_column":12,"end_row":2,"end_column":24,"start_byte":34,"end_byte":46}}],"context":"statement"}
{"record":"string_literal","value":"late_bound","scope":"_ready","range":{"start_row":2,"start_column":12,"end_row":2,"end_column":24,"start_byte":34,"end_byte":46},"argument_of":{"callee":"call","index":0}}
{"record":"reference","name":"print","scope":"_ready","range":{"start_row":3,"start_column":2,"end_row":3,"end_column":19,"start_byte":49,"end_byte":66},"name_range":{"start_row":3,"start_column":2,"end_row":3,"end_column":7,"start_byte":49,"end_byte":54},"is_call":true,"arguments":[{"text":"\"all done\"","range":{"start_row":3,"start_column":8,"end_row":3,"end_column":18,"start_byte":55,"end_byte":65}}],"context":"statement"}
{"record":"string_literal","value":"all done","scope":"_ready","range":{"start_row":3,"start_column":8,"end_row":3,"end_column":18,"start_byte":55,"end_byte":65},"argument_of":{"callee":"print","index":0}}
{"record":"declaration","kind":"variable","name":"tabbed","scope":"_ready","range":{"start_row":4,"start_column":2,"end_row":4,"end_column":22,"start_byte":68,"end_byte":88},"name_range":{"start_row":4,"start_column":6,"end_row":4,"end_column":12,"start_byte":72,"end_byte":78},"default":"\"a\\tb\""}
{"record":"string_literal","value":"a\tb","scope":"_ready","range":{"start_row":4,"start_column":16,"end_row":4,"end_column":22,"start_byte":82,"end_byte":88}}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    #[test]
    fn test_segments_say_what_kind_of_hop_they_are() {
        let source = r#####"func _ready() -> void:
	self.get_thing().field = 1
	$Clock.text = "12:00"
	items[0].name = "a"
	super._process(1.0)
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd"}
{"record":"declaration","kind":"function","name":"_ready","scope":"","range":{"start_row":1,"start_column":1,"end_row":5,"end_column":21,"start_byte":0,"end_byte":115},"name_range":{"start_row":1,"start_column":6,"end_row":1,"end_column":12,"start_byte":5,"end_byte":11},"type":"void","body_range":{"start_row":1,"start_column":23,"end_row":5,"end_column":21,"start_byte":22,"end_byte":115}}
{"record":"reference","name":"void","scope":"_ready","range":{"start_row":1,"start_column":18,"end_row":1,"end_column":22,"start_byte":17,"end_byte":21},"name_range":{"start_row":1,"start_column":18,"end_row":1,"end_column":22,"start_byte":17,"end_byte":21},"context":"type"}
{"record":"member_chain","segments":[{"kind":"self","name":"self","range":{"start_row":2,"start_column":2,"end_row":2,"end_column":6,"start_byte":24,"end_byte":28}},{"kind":"call","name":"get_thing","is_call":true,"range":{"start_row":2,"start_column":7,"end_row":2,"end_column":18,"start_byte":29,"end_byte":40}},{"kind":"identifier","name":"field","range":{"start_row":2,"start_column":19,"end_row":2,"end_column":24,"start_byte":41,"end_byte":46}}],"scope":"_ready","range":{"start_row":2,"start_column":2,"end_row":2,"end_column":24,"start_byte":24,"end_byte":46},"context":"assignment_target"}
{"record":"member_chain","segments":[{"kind":"node_path","text":"$Clock","range":{"start_row":3,"start_column":2,"end_row":3,"end_column":8,"start_byte":52,"end_byte":58}},{"kind":"identifier","name":"text","range":{"start_row":3,"start_column":9,"end_row":3,"end_column":13,"start_byte":59,"end_byte":63}}],"scope":"_ready","range":{"start_row":3,"start_column":2,"end_row":3,"end_column":13,"start_byte":52,"end_byte":63},"context":"assignment_target"}
{"record":"string_literal","value":"12:00","scope":"_ready","range":{"start_row":3,"start_column":16,"end_row":3,"end_column":23,"start_byte":66,"end_byte":73}}
{"record":"member_chain","segments":[{"kind":"subscript","name":"items","range":{"start_row":4,"start_column":2,"end_row":4,"end_column":10,"start_byte":75,"end_byte":83}},{"kind":"identifier","name":"name","range":{"start_row":4,"start_column":11,"end_row":4,"end_column":15,"start_byte":84,"end_byte":88}}],"scope":"_ready","range":{"start_row":4,"start_column":2,"end_row":4,"end_column":15,"start_byte":75,"end_byte":88},"context":"assignment_target"}
{"record":"reference","name":"items","scope":"_ready","range":{"start_row":4,"start_column":2,"end_row":4,"end_column":7,"start_byte":75,"end_byte":80},"name_range":{"start_row":4,"start_column":2,"end_row":4,"end_column":7,"start_byte":75,"end_byte":80},"context":"other"}
{"record":"string_literal","value":"a","scope":"_ready","range":{"start_row":4,"start_column":18,"end_row":4,"end_column":21,"start_byte":91,"end_byte":94}}
{"record":"member_chain","segments":[{"kind":"other","name":"super","range":{"start_row":5,"start_column":2,"end_row":5,"end_column":7,"start_byte":96,"end_byte":101}},{"kind":"call","name":"_process","is_call":true,"range":{"start_row":5,"start_column":8,"end_row":5,"end_column":21,"start_byte":102,"end_byte":115}}],"scope":"_ready","range":{"start_row":5,"start_column":2,"end_row":5,"end_column":21,"start_byte":96,"end_byte":115},"is_call":true,"arguments":[{"text":"1.0","range":{"start_row":5,"start_column":17,"end_row":5,"end_column":20,"start_byte":111,"end_byte":114}}],"context":"statement"}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    #[test]
    fn test_nested_calls_report_their_argument_position() {
        let source = r#####"func _ready() -> void:
	assert(is_instance_valid(thing))
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd"}
{"record":"declaration","kind":"function","name":"_ready","scope":"","range":{"start_row":1,"start_column":1,"end_row":2,"end_column":34,"start_byte":0,"end_byte":56},"name_range":{"start_row":1,"start_column":6,"end_row":1,"end_column":12,"start_byte":5,"end_byte":11},"type":"void","body_range":{"start_row":1,"start_column":23,"end_row":2,"end_column":34,"start_byte":22,"end_byte":56}}
{"record":"reference","name":"void","scope":"_ready","range":{"start_row":1,"start_column":18,"end_row":1,"end_column":22,"start_byte":17,"end_byte":21},"name_range":{"start_row":1,"start_column":18,"end_row":1,"end_column":22,"start_byte":17,"end_byte":21},"context":"type"}
{"record":"reference","name":"assert","scope":"_ready","range":{"start_row":2,"start_column":2,"end_row":2,"end_column":34,"start_byte":24,"end_byte":56},"name_range":{"start_row":2,"start_column":2,"end_row":2,"end_column":8,"start_byte":24,"end_byte":30},"is_call":true,"arguments":[{"text":"is_instance_valid(thing)","range":{"start_row":2,"start_column":9,"end_row":2,"end_column":33,"start_byte":31,"end_byte":55}}],"context":"statement"}
{"record":"reference","name":"is_instance_valid","scope":"_ready","range":{"start_row":2,"start_column":9,"end_row":2,"end_column":33,"start_byte":31,"end_byte":55},"name_range":{"start_row":2,"start_column":9,"end_row":2,"end_column":26,"start_byte":31,"end_byte":48},"is_call":true,"arguments":[{"text":"thing","range":{"start_row":2,"start_column":27,"end_row":2,"end_column":32,"start_byte":49,"end_byte":54}}],"context":"argument","argument_of":{"callee":"assert","index":0}}
{"record":"reference","name":"thing","scope":"_ready","range":{"start_row":2,"start_column":27,"end_row":2,"end_column":32,"start_byte":49,"end_byte":54},"name_range":{"start_row":2,"start_column":27,"end_row":2,"end_column":32,"start_byte":49,"end_byte":54},"context":"argument","argument_of":{"callee":"is_instance_valid","index":0}}
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
{"record":"reference","name":"item","scope":"_ready","range":{"start_row":3,"start_column":9,"end_row":3,"end_column":13,"start_byte":57,"end_byte":61},"name_range":{"start_row":3,"start_column":9,"end_row":3,"end_column":13,"start_byte":57,"end_byte":61},"context":"argument","argument_of":{"callee":"print","index":0}}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    #[test]
    fn test_a_function_with_no_body_reports_body_range_null() {
        let source = r#####"@abstract func may_target(candidate: Node) -> bool
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd"}
{"record":"declaration","kind":"function","name":"may_target","scope":"","range":{"start_row":1,"start_column":1,"end_row":1,"end_column":51,"start_byte":0,"end_byte":50},"name_range":{"start_row":1,"start_column":16,"end_row":1,"end_column":26,"start_byte":15,"end_byte":25},"type":"bool","annotations":[{"name":"abstract","range":{"start_row":1,"start_column":1,"end_row":1,"end_column":10,"start_byte":0,"end_byte":9}}],"modifiers":["abstract"],"parameters":[{"name":"candidate","type":"Node","range":{"start_row":1,"start_column":27,"end_row":1,"end_column":42,"start_byte":26,"end_byte":41}}],"body_range":null}
{"record":"declaration","kind":"parameter","name":"candidate","scope":"may_target","range":{"start_row":1,"start_column":27,"end_row":1,"end_column":42,"start_byte":26,"end_byte":41},"name_range":{"start_row":1,"start_column":27,"end_row":1,"end_column":36,"start_byte":26,"end_byte":35},"type":"Node"}
{"record":"reference","name":"Node","scope":"may_target","range":{"start_row":1,"start_column":38,"end_row":1,"end_column":42,"start_byte":37,"end_byte":41},"name_range":{"start_row":1,"start_column":38,"end_row":1,"end_column":42,"start_byte":37,"end_byte":41},"context":"type"}
{"record":"reference","name":"bool","scope":"may_target","range":{"start_row":1,"start_column":47,"end_row":1,"end_column":51,"start_byte":46,"end_byte":50},"name_range":{"start_row":1,"start_column":47,"end_row":1,"end_column":51,"start_byte":46,"end_byte":50},"context":"type"}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    /// A method named but not called in a truth test is a `Callable`, which is
    /// always true, so the branch never varies. That check only works if the
    /// context survives the boolean operators people actually write.
    #[test]
    fn test_truth_tests_survive_boolean_operators() {
        let source = r#####"func _ready() -> void:
	if self.flag and self.predicate:
		pass
	if not self.predicate:
		pass
	var ready := self.a or self.b
	if self.a is not Node:
		pass
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd"}
{"record":"declaration","kind":"function","name":"_ready","scope":"","range":{"start_row":1,"start_column":1,"end_row":8,"end_column":7,"start_byte":0,"end_byte":156},"name_range":{"start_row":1,"start_column":6,"end_row":1,"end_column":12,"start_byte":5,"end_byte":11},"type":"void","body_range":{"start_row":1,"start_column":23,"end_row":8,"end_column":7,"start_byte":22,"end_byte":156}}
{"record":"reference","name":"void","scope":"_ready","range":{"start_row":1,"start_column":18,"end_row":1,"end_column":22,"start_byte":17,"end_byte":21},"name_range":{"start_row":1,"start_column":18,"end_row":1,"end_column":22,"start_byte":17,"end_byte":21},"context":"type"}
{"record":"member_chain","segments":[{"kind":"self","name":"self","range":{"start_row":2,"start_column":5,"end_row":2,"end_column":9,"start_byte":27,"end_byte":31}},{"kind":"identifier","name":"flag","range":{"start_row":2,"start_column":10,"end_row":2,"end_column":14,"start_byte":32,"end_byte":36}}],"scope":"_ready","range":{"start_row":2,"start_column":5,"end_row":2,"end_column":14,"start_byte":27,"end_byte":36},"context":"condition"}
{"record":"member_chain","segments":[{"kind":"self","name":"self","range":{"start_row":2,"start_column":19,"end_row":2,"end_column":23,"start_byte":41,"end_byte":45}},{"kind":"identifier","name":"predicate","range":{"start_row":2,"start_column":24,"end_row":2,"end_column":33,"start_byte":46,"end_byte":55}}],"scope":"_ready","range":{"start_row":2,"start_column":19,"end_row":2,"end_column":33,"start_byte":41,"end_byte":55},"context":"condition"}
{"record":"member_chain","segments":[{"kind":"self","name":"self","range":{"start_row":4,"start_column":9,"end_row":4,"end_column":13,"start_byte":72,"end_byte":76}},{"kind":"identifier","name":"predicate","range":{"start_row":4,"start_column":14,"end_row":4,"end_column":23,"start_byte":77,"end_byte":86}}],"scope":"_ready","range":{"start_row":4,"start_column":9,"end_row":4,"end_column":23,"start_byte":72,"end_byte":86},"context":"condition"}
{"record":"declaration","kind":"variable","name":"ready","scope":"_ready","range":{"start_row":6,"start_column":2,"end_row":6,"end_column":31,"start_byte":96,"end_byte":125},"name_range":{"start_row":6,"start_column":6,"end_row":6,"end_column":11,"start_byte":100,"end_byte":105},"default":"self.a or self.b"}
{"record":"member_chain","segments":[{"kind":"self","name":"self","range":{"start_row":6,"start_column":15,"end_row":6,"end_column":19,"start_byte":109,"end_byte":113}},{"kind":"identifier","name":"a","range":{"start_row":6,"start_column":20,"end_row":6,"end_column":21,"start_byte":114,"end_byte":115}}],"scope":"_ready","range":{"start_row":6,"start_column":15,"end_row":6,"end_column":21,"start_byte":109,"end_byte":115},"context":"condition"}
{"record":"member_chain","segments":[{"kind":"self","name":"self","range":{"start_row":6,"start_column":25,"end_row":6,"end_column":29,"start_byte":119,"end_byte":123}},{"kind":"identifier","name":"b","range":{"start_row":6,"start_column":30,"end_row":6,"end_column":31,"start_byte":124,"end_byte":125}}],"scope":"_ready","range":{"start_row":6,"start_column":25,"end_row":6,"end_column":31,"start_byte":119,"end_byte":125},"context":"condition"}
{"record":"comparison","operator":"is not","left":{"text":"self.a","range":{"start_row":7,"start_column":5,"end_row":7,"end_column":11,"start_byte":130,"end_byte":136}},"right":{"text":"Node","range":{"start_row":7,"start_column":19,"end_row":7,"end_column":23,"start_byte":144,"end_byte":148}},"scope":"_ready","range":{"start_row":7,"start_column":5,"end_row":7,"end_column":23,"start_byte":130,"end_byte":148}}
{"record":"member_chain","segments":[{"kind":"self","name":"self","range":{"start_row":7,"start_column":5,"end_row":7,"end_column":9,"start_byte":130,"end_byte":134}},{"kind":"identifier","name":"a","range":{"start_row":7,"start_column":10,"end_row":7,"end_column":11,"start_byte":135,"end_byte":136}}],"scope":"_ready","range":{"start_row":7,"start_column":5,"end_row":7,"end_column":11,"start_byte":130,"end_byte":136},"context":"other"}
{"record":"reference","name":"Node","scope":"_ready","range":{"start_row":7,"start_column":19,"end_row":7,"end_column":23,"start_byte":144,"end_byte":148},"name_range":{"start_row":7,"start_column":19,"end_row":7,"end_column":23,"start_byte":144,"end_byte":148},"context":"other"}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    /// Most scripts have no `class_name`, so the base of the file's own script
    /// goes on the file header. A consumer working out which script broke first
    /// needs the base of every script, not only of the named ones.
    #[test]
    fn test_extends_is_reported_for_a_script_with_no_class_name() {
        let source = r#####"extends "res://base/thing.gd"

func _ready() -> void:
	pass
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd","extends":"\"res://base/thing.gd\""}
{"record":"string_literal","value":"res://base/thing.gd","scope":"","range":{"start_row":1,"start_column":9,"end_row":1,"end_column":30,"start_byte":8,"end_byte":29}}
{"record":"declaration","kind":"function","name":"_ready","scope":"","range":{"start_row":3,"start_column":1,"end_row":4,"end_column":6,"start_byte":31,"end_byte":59},"name_range":{"start_row":3,"start_column":6,"end_row":3,"end_column":12,"start_byte":36,"end_byte":42},"type":"void","body_range":{"start_row":3,"start_column":23,"end_row":4,"end_column":6,"start_byte":53,"end_byte":59},"body_is_pass_only":true}
{"record":"reference","name":"void","scope":"_ready","range":{"start_row":3,"start_column":18,"end_row":3,"end_column":22,"start_byte":48,"end_byte":52},"name_range":{"start_row":3,"start_column":18,"end_row":3,"end_column":22,"start_byte":48,"end_byte":52},"context":"type"}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    /// An annotation written above a declaration applies to it in Godot, but the
    /// grammar leaves it as a sibling rather than attaching it. Reading only the
    /// attached ones drops `@abstract` written on its own line, which is the
    /// same bug in a different spelling as the one this index exists to end.
    #[test]
    fn test_annotations_written_above_a_declaration_still_attach() {
        let source = r#####"@abstract
func may_target(candidate: Node) -> bool


@export
var health := 5
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd"}
{"record":"declaration","kind":"function","name":"may_target","scope":"","range":{"start_row":1,"start_column":1,"end_row":2,"end_column":41,"start_byte":0,"end_byte":50},"name_range":{"start_row":2,"start_column":6,"end_row":2,"end_column":16,"start_byte":15,"end_byte":25},"type":"bool","annotations":[{"name":"abstract","range":{"start_row":1,"start_column":1,"end_row":1,"end_column":10,"start_byte":0,"end_byte":9}}],"modifiers":["abstract"],"parameters":[{"name":"candidate","type":"Node","range":{"start_row":2,"start_column":17,"end_row":2,"end_column":32,"start_byte":26,"end_byte":41}}],"body_range":null}
{"record":"declaration","kind":"parameter","name":"candidate","scope":"may_target","range":{"start_row":2,"start_column":17,"end_row":2,"end_column":32,"start_byte":26,"end_byte":41},"name_range":{"start_row":2,"start_column":17,"end_row":2,"end_column":26,"start_byte":26,"end_byte":35},"type":"Node"}
{"record":"reference","name":"Node","scope":"may_target","range":{"start_row":2,"start_column":28,"end_row":2,"end_column":32,"start_byte":37,"end_byte":41},"name_range":{"start_row":2,"start_column":28,"end_row":2,"end_column":32,"start_byte":37,"end_byte":41},"context":"type"}
{"record":"reference","name":"bool","scope":"may_target","range":{"start_row":2,"start_column":37,"end_row":2,"end_column":41,"start_byte":46,"end_byte":50},"name_range":{"start_row":2,"start_column":37,"end_row":2,"end_column":41,"start_byte":46,"end_byte":50},"context":"type"}
{"record":"declaration","kind":"variable","name":"health","scope":"","range":{"start_row":5,"start_column":1,"end_row":6,"end_column":16,"start_byte":53,"end_byte":76},"name_range":{"start_row":6,"start_column":5,"end_row":6,"end_column":11,"start_byte":65,"end_byte":71},"default":"5","annotations":[{"name":"export","range":{"start_row":5,"start_column":1,"end_row":5,"end_column":8,"start_byte":53,"end_byte":60}}]}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    /// `@tool` above a bare `extends` has no declaration to bind to, and it is
    /// the most common own-line annotation in real projects. Annotations no
    /// declaration claims become records of their own, so between the two none
    /// is reported twice and none falls out of the index.
    #[test]
    fn test_annotations_no_declaration_claims_become_their_own_records() {
        let source = r#####"@tool
extends EditorPlugin


func _ready() -> void:
	pass
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd","extends":"EditorPlugin"}
{"record":"annotation","name":"tool","scope":"","range":{"start_row":1,"start_column":1,"end_row":1,"end_column":6,"start_byte":0,"end_byte":5}}
{"record":"reference","name":"EditorPlugin","scope":"","range":{"start_row":2,"start_column":9,"end_row":2,"end_column":21,"start_byte":14,"end_byte":26},"name_range":{"start_row":2,"start_column":9,"end_row":2,"end_column":21,"start_byte":14,"end_byte":26},"context":"type"}
{"record":"declaration","kind":"function","name":"_ready","scope":"","range":{"start_row":5,"start_column":1,"end_row":6,"end_column":6,"start_byte":29,"end_byte":57},"name_range":{"start_row":5,"start_column":6,"end_row":5,"end_column":12,"start_byte":34,"end_byte":40},"type":"void","body_range":{"start_row":5,"start_column":23,"end_row":6,"end_column":6,"start_byte":51,"end_byte":57},"body_is_pass_only":true}
{"record":"reference","name":"void","scope":"_ready","range":{"start_row":5,"start_column":18,"end_row":5,"end_column":22,"start_byte":46,"end_byte":50},"name_range":{"start_row":5,"start_column":18,"end_row":5,"end_column":22,"start_byte":46,"end_byte":50},"context":"type"}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    /// `_init` is a keyword in the grammar rather than a name node, and keywords
    /// are anonymous, so a scan over named children never saw it and every
    /// constructor in a project was missing from the index.
    #[test]
    fn test_a_constructor_is_a_function_declaration_named_init() {
        let source = r#####"extends Node


func _init(a: int = 1) -> void:
	pass
"#####;
        let expected = r#####"{"record":"file","schema":1,"path":"res://test.gd","extends":"Node"}
{"record":"reference","name":"Node","scope":"","range":{"start_row":1,"start_column":9,"end_row":1,"end_column":13,"start_byte":8,"end_byte":12},"name_range":{"start_row":1,"start_column":9,"end_row":1,"end_column":13,"start_byte":8,"end_byte":12},"context":"type"}
{"record":"declaration","kind":"function","name":"_init","scope":"","range":{"start_row":4,"start_column":1,"end_row":5,"end_column":6,"start_byte":15,"end_byte":52},"name_range":{"start_row":4,"start_column":6,"end_row":4,"end_column":11,"start_byte":20,"end_byte":25},"type":"void","parameters":[{"name":"a","type":"int","default":"1","range":{"start_row":4,"start_column":12,"end_row":4,"end_column":22,"start_byte":26,"end_byte":36}}],"body_range":{"start_row":4,"start_column":32,"end_row":5,"end_column":6,"start_byte":46,"end_byte":52},"body_is_pass_only":true}
{"record":"declaration","kind":"parameter","name":"a","scope":"_init","range":{"start_row":4,"start_column":12,"end_row":4,"end_column":22,"start_byte":26,"end_byte":36},"name_range":{"start_row":4,"start_column":12,"end_row":4,"end_column":13,"start_byte":26,"end_byte":27},"type":"int","default":"1"}
{"record":"reference","name":"int","scope":"_init","range":{"start_row":4,"start_column":15,"end_row":4,"end_column":18,"start_byte":29,"end_byte":32},"name_range":{"start_row":4,"start_column":15,"end_row":4,"end_column":18,"start_byte":29,"end_byte":32},"context":"type"}
{"record":"reference","name":"void","scope":"_init","range":{"start_row":4,"start_column":27,"end_row":4,"end_column":31,"start_byte":41,"end_byte":45},"name_range":{"start_row":4,"start_column":27,"end_row":4,"end_column":31,"start_byte":41,"end_byte":45},"context":"type"}
"#####;
        assert_eq!(index_to_string(source), expected);
    }

    /// Pins the schema version.
    ///
    /// Not here to check arithmetic. It is here so that changing a record shape
    /// takes two deliberate steps rather than one: the exact-output tests above
    /// fail first, and updating them alone is not enough, because this fails too
    /// and asks the question those tests cannot. Would a consumer built against
    /// the old shape misread the new one?
    ///
    /// While this producer and its consumer are developed and updated together,
    /// the answer can be "yes, and that is fine, we will update both" without a
    /// bump. Record the change in the history in
    /// `docs/specification_index.md` either way, so a build that turns out to be
    /// older than expected can be diagnosed rather than guessed at.
    #[test]
    fn test_the_schema_version_is_deliberate() {
        assert_eq!(
            crate::index::INDEX_SCHEMA_VERSION,
            1,
            "the schema version changed: check that the history in docs/specification_index.md changed with it"
        );
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

    /// A run reports res:// paths or file system paths, never a mixture. These
    /// are the cases that would mix them, and each one has to name its fix
    /// rather than fall back quietly.
    #[test]
    fn test_a_run_refuses_to_mix_path_forms() {
        let temporary_directory = std::env::temp_dir().join(format!(
            "gdscript-formatter-index-roots-{}",
            std::process::id()
        ));
        let first_project = temporary_directory.join("first");
        let second_project = temporary_directory.join("second");
        let loose_directory = temporary_directory.join("loose");
        for directory in [&first_project, &second_project, &loose_directory] {
            std::fs::create_dir_all(directory).unwrap();
        }
        std::fs::write(first_project.join("project.godot"), "config_version=5\n").unwrap();
        std::fs::write(second_project.join("project.godot"), "config_version=5\n").unwrap();
        let first_script = first_project.join("a.gd");
        let second_script = second_project.join("b.gd");
        let loose_script = loose_directory.join("c.gd");
        for script in [&first_script, &second_script, &loose_script] {
            std::fs::write(script, "var value := 1\n").unwrap();
        }

        let one_project: Vec<PathBuf> = vec![first_script.clone()];
        assert!(resolve_project_root(&one_project, None).unwrap().is_some());

        let no_project: Vec<PathBuf> = vec![loose_script.clone()];
        assert!(resolve_project_root(&no_project, None).unwrap().is_none());

        let two_projects: Vec<PathBuf> = vec![first_script.clone(), second_script];
        let error = resolve_project_root(&two_projects, None).unwrap_err();
        assert!(error.contains("--project-root"), "{}", error);

        let mixed: Vec<PathBuf> = vec![first_script, loose_script];
        let error = resolve_project_root(&mixed, None).unwrap_err();
        assert!(error.contains("mix res:// paths"), "{}", error);

        std::fs::remove_dir_all(temporary_directory).unwrap();
    }

    /// Indexes every fixture in `tests/input` and checks that nothing falls out
    /// on the way.
    ///
    /// This is the test that found two real bugs: constructors produced no
    /// declaration at all because `_init` is an anonymous keyword, and
    /// annotations that belonged to no declaration were dropped. Both were
    /// invisible to tests that assert on hand-written fixtures, because the
    /// fixture only proves what someone thought to write down. Counting nodes
    /// against records over a corpus proves what is missing.
    #[test]
    fn test_no_construct_is_silently_dropped_across_the_fixture_corpus() {
        let fixture_directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/input");
        let config = FormatterConfiguration::default();

        let mut node_counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        let mut record_counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        let mut files_checked = 0;

        for entry in std::fs::read_dir(&fixture_directory).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|extension| extension != "gd") {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Some(parsed) = ParseInput::new(&source, &config) else {
                continue;
            };
            // A file with parse errors emits a header and nothing else, so it
            // would unbalance every count.
            if parsed.has_parse_errors {
                continue;
            }
            files_checked += 1;
            count_nodes(&parsed.tree.root_node(), &mut node_counts);

            let mut output = String::new();
            index_source(&source, "res://fixture.gd", &mut output);
            count_records(&output, &mut record_counts);
        }

        assert!(
            files_checked > 50,
            "expected a corpus, got {}",
            files_checked
        );

        let node = |kind: &str| node_counts.get(kind).copied().unwrap_or(0);
        let record = |kind: &str| record_counts.get(kind).copied().unwrap_or(0);

        assert_eq!(node("comment"), record("comment"), "comments");
        assert_eq!(
            node("string") + node("string_name") + node("node_path"),
            record("string_literal"),
            "string literals"
        );
        assert_eq!(node("attribute"), record("member_chain"), "member chains");
        // Every annotation is either reported on a declaration or is a record.
        assert_eq!(
            node("annotation"),
            record("annotations_on_declarations") + record("annotation"),
            "annotations"
        );
        assert_eq!(
            node("function_definition") + node("constructor_definition") + node("lambda_named"),
            record("declaration:function"),
            "functions, constructors and named lambdas"
        );
        assert_eq!(
            node("class_name_statement") + node("class_definition") + node("inner_class"),
            record("declaration:class"),
            "classes"
        );
        assert_eq!(
            node("signal_statement"),
            record("declaration:signal"),
            "signals"
        );
        assert_eq!(
            node("enumerator"),
            record("declaration:enum_member"),
            "enum members"
        );
        assert_eq!(
            node("const_statement"),
            record("declaration:constant"),
            "constants"
        );
        assert_eq!(
            node("enum_definition") - node("enum_anonymous"),
            record("declaration:enum"),
            "named enums"
        );
        assert_eq!(
            node("variable_statement")
                + node("export_variable_statement")
                + node("onready_variable_statement")
                + node("for_statement"),
            record("declaration:variable"),
            "variables and loop bindings"
        );
    }

    fn count_nodes(
        node: &tree_sitter::Node,
        counts: &mut std::collections::BTreeMap<String, usize>,
    ) {
        if node.is_named() {
            *counts.entry(node.kind().to_string()).or_insert(0) += 1;
            if node.kind() == "lambda" && node.child_by_field_name("name").is_some() {
                *counts.entry("lambda_named".to_string()).or_insert(0) += 1;
            }
            if node.kind() == "enum_definition" && node.child_by_field_name("name").is_none() {
                *counts.entry("enum_anonymous".to_string()).or_insert(0) += 1;
            }
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                count_nodes(&cursor.node(), counts);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Tallies records by kind without a JSON parser: the field order is fixed
    /// by the writer, so the leading text of each line identifies it.
    fn count_records(output: &str, counts: &mut std::collections::BTreeMap<String, usize>) {
        for line in output.lines() {
            let Some(record_start) = line.strip_prefix("{\"record\":\"") else {
                continue;
            };
            let Some(quote_position) = record_start.find('"') else {
                continue;
            };
            let record_kind = &record_start[..quote_position];
            *counts.entry(record_kind.to_string()).or_insert(0) += 1;

            if record_kind == "declaration" {
                let after_kind = record_start[quote_position..]
                    .strip_prefix("\",\"kind\":\"")
                    .expect("a declaration writes its kind straight after the record name");
                let kind_end = after_kind.find('"').expect("kind is quoted");
                *counts
                    .entry(format!("declaration:{}", &after_kind[..kind_end]))
                    .or_insert(0) += 1;
            }

            *counts
                .entry("annotations_on_declarations".to_string())
                .or_insert(0) += count_annotations_on_declaration(line);
        }
    }

    /// Counts the objects in a record's `annotations` array.
    ///
    /// `parameters` entries open with the same `{"name":` text, so this walks the
    /// array by bracket depth rather than searching for a substring, and skips
    /// brackets that appear inside strings.
    fn count_annotations_on_declaration(line: &str) -> usize {
        let marker = ",\"annotations\":[";
        let Some(annotations_start) = line.find(marker) else {
            return 0;
        };

        let bytes = line.as_bytes();
        let mut position = annotations_start + marker.len();
        let mut depth = 1;
        let mut is_inside_string = false;
        let mut is_escaped = false;
        let mut annotation_count = 0;

        while position < bytes.len() && depth > 0 {
            let byte = bytes[position];
            if is_inside_string {
                if is_escaped {
                    is_escaped = false;
                } else if byte == b'\\' {
                    is_escaped = true;
                } else if byte == b'"' {
                    is_inside_string = false;
                }
            } else if byte == b'"' {
                is_inside_string = true;
            } else if byte == b'{' {
                if depth == 1 {
                    annotation_count += 1;
                }
                depth += 1;
            } else if byte == b'[' {
                depth += 1;
            } else if byte == b']' || byte == b'}' {
                depth -= 1;
            }
            position += 1;
        }

        annotation_count
    }
}
