#!/usr/bin/env python3
"""Check installer error handling without contacting a router or running setup."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT / "setup.sh").read_text()
NAMES = ["sha256_stream", "sha256", "sha256_file", "shell_quote", "ubus_call", "remote_ok"]
HELPERS = "\n".join(re.search(r"(?ms)^" + name + r"\(\) \{\n.*?^\}", SOURCE).group()
                    for name in NAMES)
BASH = shutil.which("bash")


class SetupHelperTests(unittest.TestCase):
    def run_shell(self, command, args=(), env=None):
        return subprocess.run([BASH, "-c", HELPERS + "\n" + command, "test", *args],
                              env=env, text=True, capture_output=True)

    def test_hashes_work_on_linux_without_shasum(self):
        with tempfile.TemporaryDirectory() as directory:
            for tool in ["sha256sum", "awk"]:
                Path(directory, tool).symlink_to(shutil.which(tool))
            value = "literal %s and a trailing newline\n"
            result = self.run_shell('sha256 "$1"', [value], dict(os.environ, PATH=directory))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip(), hashlib.sha256(value.encode()).hexdigest())

    def test_missing_hash_tool_fails_instead_of_returning_empty_success(self):
        with tempfile.TemporaryDirectory() as directory:
            result = self.run_shell('sha256 "$1"', ["example"], dict(os.environ, PATH=directory))
            self.assertNotEqual(result.returncode, 0)

    def test_remote_shell_quoting_preserves_special_characters(self):
        value = "quotes ' and \"; dollar $HOME; literal $(false); slash \\"
        result = self.run_shell('quoted=$(shell_quote "$1"); eval "printf %s $quoted"', [value])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, value)

    def test_legacy_adb_success_does_not_hide_failed_remote_command(self):
        command = 'rcmd() { bash -c "$1"; return 0; }; remote_ok false'
        self.assertNotEqual(self.run_shell(command).returncode, 0)
        self.assertEqual(self.run_shell(command.replace('remote_ok false', 'remote_ok true')).returncode, 0)

    def call_ubus(self, payload):
        env = dict(os.environ, U60_TEST_JSON=payload)
        return self.run_shell('curl() { printf %s "$U60_TEST_JSON"; }; GATEWAY=example.invalid; '
                              'ubus_call anonymous zwrt_bsp.usb set "{}"', env=env)

    def test_http_success_with_ubus_error_is_rejected_without_dumping_payload(self):
        result = self.call_ubus(json.dumps([{"result": [3, {"session": "test-private-value"}]}]))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("method not found", result.stderr)
        self.assertNotIn("test-private-value", result.stdout + result.stderr)

    def test_successful_and_malformed_json_rpc_responses(self):
        result = self.call_ubus('{"result":[0,{"mode":"debug"}]}')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)[0]["result"][1]["mode"], "debug")
        for payload in ['not-json', '{"result":[]}', '{"result":[false]}', '{"error":{"code":-1}}']:
            with self.subTest(payload=payload):
                self.assertNotEqual(self.call_ubus(payload).returncode, 0)


if __name__ == "__main__":
    unittest.main()
