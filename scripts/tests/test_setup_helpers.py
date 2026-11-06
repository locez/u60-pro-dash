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

    def test_ota_bootstrap_verifies_both_slots_on_legacy_adb(self):
        start = SOURCE.index("# A bootstrap is an explicit repair/reset operation.")
        end = SOURCE.index('ok "OTA bootstrap and rollback supervisor installed."', start)
        step = SOURCE[start:end]
        for failure in (None, "probe", "fallback"):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / "slot-a").mkdir()
                if failure == "fallback":
                    (root / "slot-b").write_text("not a directory")
                else:
                    (root / "slot-b").mkdir()
                binary = root / "slot-a/zte-agent"
                binary.write_text('#!/bin/sh\ncase "$1" in\n'
                                  '--version) printf "zte-agent 2.4.0\\r\\n";;\n'
                                  'ota-probe) exit "${U60_TEST_PROBE_EXIT:-0}";;\n'
                                  '*) exit 1;;\nesac\n')
                binary.chmod(0o755)
                (root / "active").write_text("b\n")
                (root / "current").write_text("previous agent")
                (root / "supervisor-source").write_text("#!/bin/sh\n")
                harness = '''
set -e
info() { :; }; ok() { :; }; fail() { printf '%s' "$1" >&2; exit 1; }
rcmd() { bash -c "$1"; return 0; }
rpush() { cp "$1" "$2"; }
OTA_ROOT=$1
REMOTE_SLOT=$OTA_ROOT/slot-a/zte-agent
REMOTE_SLOT_META=$OTA_ROOT/slot-a/release.json
REMOTE_SLOT_BOOTSTRAP_SHA=$OTA_ROOT/slot-a/bootstrap.sha256
REMOTE_FALLBACK=$OTA_ROOT/slot-b/zte-agent
REMOTE_FALLBACK_META=$OTA_ROOT/slot-b/release.json
REMOTE_FALLBACK_BOOTSTRAP_SHA=$OTA_ROOT/slot-b/bootstrap.sha256
REMOTE_BIN=$OTA_ROOT/current
SUPERVISOR=$OTA_ROOT/supervisor
SUPERVISOR_LOCAL=$OTA_ROOT/supervisor-source
LOCAL_SHA=$(sha256_file "$REMOTE_SLOT")
'''
                result = self.run_shell(harness + step, [str(root)],
                                        dict(os.environ, U60_TEST_PROBE_EXIT="1" if failure == "probe" else "0"))
                if failure:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertEqual((root / "active").read_text(), "b\n")
                    self.assertEqual((root / "current").read_text(), "previous agent")
                    self.assertFalse((root / "supervisor").exists())
                else:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    expected = hashlib.sha256(binary.read_bytes()).hexdigest() + "\n2.4.0\n"
                    for slot in ("slot-a", "slot-b"):
                        self.assertEqual((root / slot / "zte-agent").read_bytes(), binary.read_bytes())
                        self.assertEqual((root / slot / "bootstrap.sha256").read_text(), expected)
                        self.assertEqual((root / slot / "bootstrap.sha256").stat().st_mode & 0o777, 0o600)
                    self.assertEqual((root / "active").read_text(), "a\n")
                    self.assertEqual((root / "current").resolve(), binary)

    def test_ota_startup_migration_preserves_gateway_and_password(self):
        step = SOURCE.split("# ── Step 5:", 1)[1].split("# ── Step 6:", 1)[0]
        step = step.split("\n", 1)[1]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            startup = root / "startup.sh"
            startup.write_text("#!/bin/sh\nexport ZTE_AGENT_TAILSCALE_GATEWAY=1\n"
                               "export ZTE_AGENT_PASSWORD='old example'\n"
                               "nohup /data/zte-agent &\n")
            password = "example with 'quotes' and $literal"
            harness = '''
set -e
info() { :; }; ok() { :; }; fail() { printf '%s' "$1" >&2; exit 1; }
rcmd() { bash -c "$1"; return 0; }
rpush() { test "$(stat -c %a "$1")" = 600; cp "$1" "$2"; }
STARTUP_SCRIPT=$1; AGENT_PASSWORD=$2
OTA_ROOT=/data/open-u60/update
SUPERVISOR=/data/local/tmp/zte-agent-supervisor.sh
REMOTE_BIN=/data/zte-agent
'''
            result = self.run_shell(harness + step, [str(startup), password],
                                    dict(os.environ, TMPDIR=directory))
            self.assertEqual(result.returncode, 0, result.stderr)
            content = startup.read_text()
            self.assertIn('export ZTE_AGENT_TAILSCALE_GATEWAY=1', content)
            self.assertIn('nohup /data/local/tmp/zte-agent-supervisor.sh', content)
            self.assertEqual(startup.stat().st_mode & 0o777, 0o700)
            self.assertEqual(sorted(p.name for p in root.iterdir()), ['startup.sh'])
            # Read only the export line, never execute the generated startup command.
            export = next(line for line in content.splitlines() if line.startswith('export ZTE_AGENT_PASSWORD='))
            decoded = subprocess.run([BASH, '-c', export + '; printf %s "$ZTE_AGENT_PASSWORD"'],
                                     text=True, capture_output=True, check=True)
            self.assertEqual(decoded.stdout, password)
            startup.write_text(content + '# custom local tuning\n')
            result = self.run_shell(harness + step, [str(startup), password])
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(startup.read_text(), content + '# custom local tuning\n')


if __name__ == "__main__":
    unittest.main()
