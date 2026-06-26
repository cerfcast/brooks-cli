#[cfg(test)]
mod cli_tests {
    use insta_cmd::{assert_cmd_snapshot, get_cargo_bin};
    use std::process::Command;

    #[test]
    fn help_test() {
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).arg("--help"));
    }

    #[test]
    fn simple_test() {
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).args([
            "compile",
            "--path",
            "tests/simple.mel"
        ]));
    }

    #[test]
    fn binary_test() {
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).args([
            "compile",
            "--path",
            "tests/binary.mel"
        ]));
    }

    #[test]
    fn interp_test_path_element() {
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).args([
            "interpret",
            "--path",
            "tests/path_element.mel"
        ]));
    }

    #[test]
    fn interp_test_path_element_name() {
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).args([
            "interpret",
            "--path",
            "tests/function_name.mel"
        ]));
    }

    #[test]
    fn interp_test_reqs_name() {
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).args([
            "interpret",
            "--path",
            "tests/struct_name.mel"
        ]));
    }

    #[test]
    fn interp_test_reqs() {
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).args([
            "interpret",
            "--path",
            "tests/reqs.mel"
        ]));
    }

    #[test]
    fn interp_test_boolean_builtin() {
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).args([
            "interpret",
            "--path",
            "tests/boolean.mel"
        ]));
    }
}
