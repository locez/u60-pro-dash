"""Exercise the real recovery script against fake firmware services, never adb."""
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "device-trial.sh"


class TrialRecovery(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="u60-trial-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.lock = self.root / "lock"
        self.vendor = self.root / "vendor-up"
        self.vendor.touch()
        self.light = self.root / "brightness"
        self.light.write_text("23\n")
        self.log = self.root / "service.log"
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.env = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}",
                        TEST_VENDOR=str(self.vendor), TEST_LOG=str(self.log),
                        TEST_LIGHT=str(self.light), TEST_RUN_SECONDS="30")
        self.executable(self.bin / "id", '#!/bin/sh\necho 0\n')
        self.executable(self.bin / "pidof", '#!/bin/sh\ntest -f "$TEST_VENDOR" && echo 123\n')
        self.executable(self.bin / "ubus", '#!/bin/sh\nprintf \'{"noSyncModuleName":"%s"}\\n\' "${TEST_SYNC:-sync success}"\n')
        self.executable(self.root / "vendor", '''#!/bin/sh
echo "$1" >> "$TEST_LOG"
case "$1" in
 stop) (sleep "${TEST_STOP_DELAY:-0}"; rm -f "$TEST_VENDOR") & ;;
 start|restart) touch "$TEST_VENDOR";;
 *) exit 99;;
esac
''')
        source = SOURCE.read_text().replace('LOCK=/tmp/u60-native-screen-trial', f'LOCK={self.lock}')
        source = source.replace('VENDOR=/etc/init.d/zte_topsw_devui', f'VENDOR={self.root / "vendor"}')
        source = source.replace('BACKLIGHT=/sys/class/leds/led:lcd/brightness', f'BACKLIGHT={self.light}')
        self.script = self.root / "device-trial.sh"
        self.script.write_text(source)
        c = self.root / "screen.c"
        c.write_text('''#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
int main(void) {
 FILE *f = fopen(getenv("TEST_LIGHT"), "w"); if (!f) return 90;
 fputs("99\\n", f); fclose(f);
 sleep(atoi(getenv("TEST_RUN_SECONDS")));
 return getenv("TEST_EXIT_CODE") ? atoi(getenv("TEST_EXIT_CODE")) : 0;
}
''')
        subprocess.run([shutil.which("cc") or "cc", str(c), "-o", str(self.root / "u60-screen")], check=True)

    @staticmethod
    def executable(path, text):
        path.write_text(text)
        path.chmod(0o755)

    def start(self, **env):
        output = open(self.root / "trial.log", "w")
        self.addCleanup(output.close)
        proc = subprocess.Popen(["sh", str(self.script), "--seconds", "5"],
                                env=dict(self.env, **env), stdout=output, stderr=subprocess.STDOUT)
        self.addCleanup(lambda: proc.kill() if proc.poll() is None else None)
        return proc

    def wait_for(self, predicate, timeout=15):
        until = time.monotonic() + timeout
        while time.monotonic() < until:
            if predicate():
                return
            time.sleep(0.05)
        self.fail((self.root / "trial.log").read_text())

    def restored(self):
        self.wait_for(lambda: not self.lock.exists() and self.vendor.exists())
        self.assertEqual(self.light.read_text(), "23\n")
        self.assertEqual(self.log.read_text().splitlines(), ["stop", "start"])

    def test_normal_exit_and_error_exit_restore_vendor(self):
        for code in ("0", "17"):
            with self.subTest(code=code):
                self.log.unlink(missing_ok=True)
                proc = self.start(TEST_RUN_SECONDS="1", TEST_EXIT_CODE=code)
                self.assertEqual(proc.wait(timeout=15), int(code))
                self.restored()

    def test_guard_survives_parent_sigkill(self):
        proc = self.start()
        self.wait_for(lambda: (self.lock / "screen.pid").exists())
        proc.kill()
        proc.wait(timeout=5)
        self.restored()

    def test_guard_survives_screen_sigkill(self):
        proc = self.start()
        self.wait_for(lambda: (self.lock / "screen.pid").exists())
        os.kill(int((self.lock / "screen.pid").read_text()), signal.SIGKILL)
        self.assertNotEqual(proc.wait(timeout=15), 0)
        self.restored()

    def test_incomplete_firmware_boot_never_stops_vendor(self):
        proc = self.start(TEST_SYNC="waiting for devui")
        self.assertNotEqual(proc.wait(timeout=5), 0)
        self.assertTrue(self.vendor.exists())
        self.assertFalse(self.lock.exists())
        self.assertFalse(self.log.exists())

    def test_asynchronous_vendor_stop_is_fully_finished_before_handoff(self):
        proc = self.start(TEST_STOP_DELAY="3", TEST_RUN_SECONDS="1")
        self.assertEqual(proc.wait(timeout=20), 0)
        self.restored()

    def test_parent_killed_during_vendor_stop_cannot_race_recovery(self):
        proc = self.start(TEST_STOP_DELAY="3")
        self.wait_for(lambda: self.log.exists() and "stop" in self.log.read_text())
        proc.kill()
        proc.wait(timeout=5)
        self.restored()

    def test_guard_deadline_kills_a_hung_screen_and_restores_vendor(self):
        # Exercise the deployed watchdog path with a shorter test-only deadline.
        self.script.write_text(self.script.read_text().replace('"$((seconds + 30))"', '"3"'))
        proc = self.start(TEST_RUN_SECONDS="120")
        self.assertNotEqual(proc.wait(timeout=15), 0)
        self.restored()


if __name__ == "__main__":
    unittest.main()
