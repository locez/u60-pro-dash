"""Exercise the real cleanup subprocess using fake network utilities.
Run: U60_AGENT_TEST_BINARY=/path/to/zte-agent python3 zte-agent/tests/proxy_supervisor.py
No root privileges or real network changes are needed.
"""
import fcntl
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time
import unittest

BINARY = os.environ.get('U60_AGENT_TEST_BINARY')

@unittest.skipUnless(BINARY, 'Set U60_AGENT_TEST_BINARY to the host agent build')
class SupervisorTest(unittest.TestCase):
    def exercise(self, engine_exits):
        with tempfile.TemporaryDirectory(prefix='u60-proxy-supervisor-') as directory:
            root = Path(directory)
            root.joinpath('runtime.json').write_text('{}')
            root.joinpath('capture-owned').write_text('1')
            engine = root / 'engine'
            engine.write_text(f'#!{sys.executable}\nimport os, time\nfrom pathlib import Path\nPath({str(root / "engine-pid")!r}).write_text(str(os.getpid()))\ntime.sleep({0.2 if engine_exits else 120})\n')
            engine.chmod(0o700)
            for name in ['ip', 'iptables', 'ip6tables']:
                utility = root / name
                utility.write_text(f'#!{sys.executable}\nimport sys\nwith open({str(root / "calls")!r},"a") as f: f.write(" ".join(sys.argv)+"\\n")\n')
                utility.chmod(0o700)
            owner, child = socket.socketpair()
            with open(root / 'service.lock', 'a') as lease:
                fcntl.flock(lease, fcntl.LOCK_EX | fcntl.LOCK_NB)
                process = subprocess.Popen([BINARY, '--proxy-watchdog', str(root), 'compatible-v1'], stdin=child, stdout=lease, stderr=subprocess.PIPE, env={**os.environ, 'PATH':directory})
            child.close()
            try:
                deadline = time.monotonic() + 5
                while not root.joinpath('engine-pid').exists() and time.monotonic() < deadline:
                    time.sleep(0.01)
                self.assertTrue(root.joinpath('engine-pid').exists())
                with open(root / 'service.lock', 'a') as competing:
                    with self.assertRaises(BlockingIOError):
                        fcntl.flock(competing, fcntl.LOCK_EX | fcntl.LOCK_NB)
                if not engine_exits:
                    owner.close()  # Same EOF as an owning agent crash or kill.
                self.assertEqual(process.wait(timeout=30), 1 if engine_exits else 0)
                self.assertFalse(root.joinpath('capture-owned').exists())
                self.assertFalse(root.joinpath('runtime.json').exists())
                with self.assertRaises(ProcessLookupError):
                    os.kill(int(root.joinpath('engine-pid').read_text()), 0)
                with open(root / 'service.lock', 'a') as recovered:
                    fcntl.flock(recovered, fcntl.LOCK_EX | fcntl.LOCK_NB)
                calls = root.joinpath('calls').read_text()
                self.assertIn('rule del priority 10070', calls)
                self.assertIn('-D PREROUTING -j U60_PROXY_DNS', calls)
                self.assertNotIn('table 52', calls)
                self.assertNotIn('flush', calls)
            finally:
                owner.close()
                if process.poll() is None:
                    process.kill()
                process.communicate(timeout=5)
    def test_agent_disconnect_reaps_engine_and_removes_owned_rules(self):
        self.exercise(False)
    def test_engine_failure_restores_routes_while_agent_is_alive(self):
        self.exercise(True)

if __name__ == '__main__':
    unittest.main()
