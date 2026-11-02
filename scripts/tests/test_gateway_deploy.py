#!/usr/bin/env python3
"""An OTA device must be rejected before legacy deploy or rollback writes."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

DEPLOY = Path(__file__).resolve().parents[2] / "local-tools/u60-home-gateway/deploy.py"


class GatewayDeployTests(unittest.TestCase):
    def test_ota_layout_is_read_only_for_all_modes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fake_adb = root / "adb"
            fake_adb.write_text("#!/bin/sh\n"
                                'printf "%s\\n" "$1" >> "$U60_TEST_ADB_LOG"\n'
                                'case "$1" in\n'
                                'get-state) echo device ;;\n'
                                'shell) printf "ota\\n__U60_GATEWAY_RC__0\\n" ;;\n'
                                '*) exit 99 ;;\n'
                                'esac\n')
            fake_adb.chmod(0o755)
            log = root / "adb.log"
            env = dict(os.environ, PATH=str(root) + os.pathsep + os.environ["PATH"],
                       U60_TEST_ADB_LOG=str(log))
            env.pop("ANDROID_SERIAL", None)
            for mode in [[], ["--check"], ["--rollback"]]:
                with self.subTest(mode=mode):
                    log.write_text("")
                    result = subprocess.run([sys.executable, str(DEPLOY), *mode],
                                            env=env, text=True, capture_output=True)
                    self.assertEqual(result.returncode, 1)
                    self.assertIn("检测到 OTA 槽位安装", result.stdout)
                    self.assertEqual(log.read_text().splitlines(), ["get-state", "shell"])


if __name__ == "__main__":
    unittest.main()
