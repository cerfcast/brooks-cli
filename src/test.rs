// brooks-cli, Copyright 2026, Will Hawkins
//
// This file is part of brooks-cli.

// This file is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#[cfg(test)]
mod cli_tests {
    use insta_cmd::{assert_cmd_snapshot, get_cargo_bin};
    use std::process::Command;

    #[test]
    fn help_test() {
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).arg("--help"));
    }

    #[test]
    fn bad_path_test() {
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).args([
            "proxy",
            "--port",
            "8080",
            "--path",
            "./not-found/"
        ]));
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

    #[test]
    fn compile_test_errors() {
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).args([
            "analyze",
            "--path",
            "tests/compile_error.mel"
        ]));
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).args([
            "analyze",
            "--path",
            "tests/compile_error2.mel"
        ]));
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).args([
            "analyze",
            "--path",
            "tests/compile_error3.mel"
        ]));
    }

    #[test]
    fn analyze_test_errors() {
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).args([
            "analyze",
            "--path",
            "tests/analysis_error.mel"
        ]));
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).args([
            "analyze",
            "--path",
            "tests/analysis_error2.mel"
        ]));
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).args([
            "analyze",
            "--path",
            "tests/analysis_error3.mel"
        ]));
        assert_cmd_snapshot!(Command::new(get_cargo_bin("brooks-cli")).args([
            "analyze",
            "--path",
            "tests/analysis_error4.mel"
        ]));
    }
}
