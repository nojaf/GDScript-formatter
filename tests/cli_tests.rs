use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn test_directory() -> std::path::PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after the Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "gdscript-formatter-cli-test-{}-{}",
        std::process::id(),
        timestamp
    ));
    fs::create_dir(&path).expect("should create temporary test directory");
    path
}

fn formatter_command(directory: &std::path::Path, args: &[&str]) -> Command {
    let binary = std::env::current_exe()
        .expect("test executable path should be available")
        .parent()
        .expect("test executable should have a parent")
        .parent()
        .expect("test executable should be in target/debug/deps")
        .join("gdscript-formatter");
    let mut command = Command::new(binary);
    command.current_dir(directory).args(args);
    command
}

#[test]
fn stdin_and_file_modes_apply_editorconfig_and_cli_overrides() {
    let directory = test_directory();
    fs::write(
        directory.join(".editorconfig"),
        "root = true\n\n[*.gd]\nindent_style = space\nindent_size = 8\nmax_line_length = 120\ngdscript_formatter_blank_lines_around_definitions = 2\ngdscript_formatter_quote_style = double\n",
    )
    .expect("should write EditorConfig");

    let mut stdin_command = formatter_command(
        &directory,
        &[
            "--blank-lines-around-definitions",
            "1",
            "--use-spaces",
            "--indent-size",
            "2",
            "--quote-style",
            "single",
        ],
    );

    let input = "func first():\n\tvar value = \"first\"\n\n\nfunc second():\n\tpass\n";
    let expected = "func first():\n  var value = 'first'\n\nfunc second():\n  pass\n";

    stdin_command.stdin(Stdio::piped()).stdout(Stdio::piped());
    let mut child = stdin_command.spawn().expect("should start formatter");
    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(input.as_bytes())
        .expect("should write formatter input");
    let stdin_output = child
        .wait_with_output()
        .expect("should collect formatter output");
    assert!(stdin_output.status.success());
    assert_eq!(
        String::from_utf8(stdin_output.stdout).expect("stdin output should be valid UTF-8"),
        expected,
    );

    let input_path = directory.join("input.gd");
    fs::write(&input_path, input).expect("should write input file");
    let file_output = formatter_command(
        &directory,
        &[
            "--stdout",
            "--blank-lines-around-definitions",
            "1",
            "--use-spaces",
            "--indent-size",
            "2",
            "--quote-style",
            "single",
            "input.gd",
        ],
    )
    .output()
    .expect("should format file");
    assert!(file_output.status.success());
    assert_eq!(
        String::from_utf8(file_output.stdout).expect("file output should be valid UTF-8"),
        expected,
    );

    let lint_path = directory.join("lint.gd");
    fs::write(&lint_path, format!("#{}\n", "1".repeat(119))).expect("should write lint input");
    let lint_output = formatter_command(&directory, &["lint", "lint.gd"])
        .output()
        .expect("should lint file");
    assert!(lint_output.status.success());
    assert!(lint_output.stdout.is_empty());

    let lint_override_output =
        formatter_command(&directory, &["lint", "--max-line-length", "100", "lint.gd"])
            .output()
            .expect("should lint file with an override");
    assert!(!lint_override_output.status.success());
    assert!(
        String::from_utf8(lint_override_output.stdout)
            .expect("lint output should be valid UTF-8")
            .contains("maximum allowed is 100")
    );

    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

#[test]
fn index_reports_project_relative_paths_and_fails_on_parse_errors() {
    let directory = test_directory();
    fs::write(directory.join("project.godot"), "config_version=5\n")
        .expect("should write a Godot project file");
    fs::create_dir(directory.join("hud")).expect("should create a subdirectory");
    fs::write(
        directory.join("hud/hud.gd"),
        "class_name Hud\nextends CanvasLayer\n",
    )
    .expect("should write an indexable file");
    fs::write(directory.join("skipped.gd"), "var skipped := 1\n")
        .expect("should write a file to exclude");

    let output = formatter_command(&directory, &["index", "-x", "skipped.gd", "."])
        .output()
        .expect("should index the project");
    assert!(output.status.success());
    let lines = String::from_utf8(output.stdout).expect("index output should be valid UTF-8");
    assert!(
        lines.contains("\"path\":\"res://hud/hud.gd\""),
        "expected a res:// path, got: {}",
        lines
    );
    assert!(!lines.contains("skipped"), "the excluded file was indexed");
    assert!(lines.contains("\"record\":\"declaration\",\"kind\":\"class\",\"name\":\"Hud\""));

    // Stdin is indexed under a synthetic path so editors can pipe a buffer in.
    let mut stdin_command = formatter_command(&directory, &["index"]);
    stdin_command.stdin(Stdio::piped()).stdout(Stdio::piped());
    let mut child = stdin_command.spawn().expect("should start the indexer");
    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(b"var health := 5\n")
        .expect("should write indexer input");
    let stdin_output = child
        .wait_with_output()
        .expect("should collect indexer output");
    assert!(stdin_output.status.success());
    let stdin_lines =
        String::from_utf8(stdin_output.stdout).expect("index output should be valid UTF-8");
    assert!(stdin_lines.starts_with("{\"record\":\"file\",\"path\":\"<stdin>\"}\n"));

    // An explicit root wins over discovery, which is the escape hatch for
    // layouts where walking up finds the wrong thing or nothing at all.
    let explicit_root_output =
        formatter_command(&directory, &["index", "--project-root", ".", "hud/hud.gd"])
            .output()
            .expect("should index with an explicit project root");
    assert!(explicit_root_output.status.success());
    assert!(
        String::from_utf8(explicit_root_output.stdout)
            .expect("index output should be valid UTF-8")
            .contains("\"path\":\"res://hud/hud.gd\"")
    );

    // A file outside the root has no res:// path. Reporting it as a file system
    // path would mix the two forms in one run, and a consumer joining on
    // res:// paths would silently match nothing.
    let outside_root_output = formatter_command(
        &directory,
        &["index", "--project-root", "hud", "skipped.gd"],
    )
    .output()
    .expect("should reject a file outside the project root");
    assert!(!outside_root_output.status.success());
    assert!(
        String::from_utf8(outside_root_output.stderr)
            .expect("stderr should be valid UTF-8")
            .contains("outside the project root")
    );

    // A file that does not parse still gets a header, and the run fails so a
    // consumer cannot mistake a broken project for a clean one.
    fs::write(directory.join("broken.gd"), "func (:\n").expect("should write a broken file");
    let broken_output = formatter_command(&directory, &["index", "broken.gd"])
        .output()
        .expect("should run the indexer on a broken file");
    assert!(!broken_output.status.success());
    let broken_lines =
        String::from_utf8(broken_output.stdout).expect("index output should be valid UTF-8");
    assert!(broken_lines.contains("\"parse_error\":true"));
    assert_eq!(broken_lines.lines().count(), 1);

    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}
