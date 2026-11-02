"""Exercise rule ownership and configured CIDRs without touching host networking."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[2] / 'local-tools/tailscale-gateway/tailscale-gateway.sh'


class GatewayRuleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.config = self.root / 'gateway.conf'
        self.state = self.root / 'rules.json'
        self.state.write_text('[]')
        self.env = dict(os.environ, PATH=str(self.root) + os.pathsep + os.environ['PATH'],
                        U60_TS_GATEWAY_CONFIG=str(self.config), U60_GW_STATE=str(self.state))
        (self.root / 'ip').write_text('#!/bin/sh\necho "2: br-test inet 10.8.0.1/23 brd 10.8.1.255 scope global br-test"\n')
        (self.root / 'cat').write_text('#!/bin/sh\nif [ "$1" = /proc/sys/net/ipv4/ip_forward ]; then echo 1; else exec /bin/cat "$@"; fi\n')
        (self.root / 'iptables').write_text('''#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
path=Path(os.environ['U60_GW_STATE']); rules=json.loads(path.read_text())
a=sys.argv[1:]; assert a[:3]==['-w','5','-t']
table, operation, chain=a[3:6]; tail=a[6:]
if operation=='-I': assert tail.pop(0)=='1'
key=[table,chain,*tail]
if operation=='-C': sys.exit(0 if key in rules else 1)
if operation=='-I': rules.insert(0,key)
elif operation=='-D': rules.remove(key)
else: raise AssertionError('unexpected mutation')
path.write_text(json.dumps(rules))
''')
        for name in ['ip', 'cat', 'iptables']:
            (self.root / name).chmod(0o755)
        self.configure()

    def configure(self, lan='10.8.0.0/23', remote='172.20.10.0/24 10.20.0.0/16'):
        self.config.write_text(f'LAN_IF=br-test\nLAN_NET={lan}\nREMOTE_NETS={remote}\n')

    def run_rules(self, action='apply'):
        return subprocess.run(['sh', str(SCRIPT), action], env=self.env,
                              capture_output=True, text=True, timeout=20)

    def test_multiple_subnets_are_idempotent_and_removal_preserves_unrelated_rules(self):
        unrelated = ['filter', 'FORWARD', '-j', 'ACCEPT']
        self.state.write_text(json.dumps([unrelated]))
        for _ in range(2):
            result = self.run_rules()
            self.assertEqual(result.returncode, 0, result.stderr)
            rules = json.loads(self.state.read_text())
            self.assertEqual(len(rules), 13)
            self.assertEqual(len({json.dumps(rule) for rule in rules}), 13)
            self.assertIn(unrelated, rules)
        self.assertEqual(self.run_rules('remove').returncode, 0)
        self.assertEqual(json.loads(self.state.read_text()), [unrelated])

    def test_migrates_matching_legacy_rules_only_after_installing_current_rules(self):
        self.assertEqual(self.run_rules().returncode, 0)
        rules = json.loads(self.state.read_text())
        legacy = [[part.replace('u60-tailscale-gateway', 'u60-home-gateway') for part in rule] for rule in rules]
        foreign = ['filter', 'FORWARD', '-s', '192.0.2.0/24', '-m', 'comment', '--comment', 'u60-home-gateway', '-j', 'ACCEPT']
        self.state.write_text(json.dumps(legacy + [foreign]))
        self.assertEqual(self.run_rules().returncode, 0)
        self.assertCountEqual(json.loads(self.state.read_text()), rules + [foreign])

    def test_bad_or_overlapping_configuration_never_changes_rules(self):
        for lan, remote in [('10.8.0.0/23', ''), ('10.8.0.0/23', '10.8.1.0/24'),
                            ('10.8.0.1/23', '172.20.0.0/16'), ('10.8.0.0/23', '999.0.0.0/8'),
                            ('10.9.0.0/24', '172.20.0.0/16'), ('10.8.0.0/23', '$(touch marker)')]:
            with self.subTest(lan=lan, remote=remote):
                self.configure(lan, remote)
                self.assertNotEqual(self.run_rules().returncode, 0)
                self.assertEqual(json.loads(self.state.read_text()), [])


if __name__ == '__main__':
    unittest.main()
