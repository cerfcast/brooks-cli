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
}
