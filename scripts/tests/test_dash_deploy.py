"""Exercise deployment scripts against local fake firmware and real processes."""
import hashlib
import importlib.util
import os
from pathlib import Path
import shutil
import signal
import struct
import subprocess
import tempfile
import time
import unittest

REPO = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('dash_deploy', REPO / 'scripts/dash/deploy.py')
DEPLOY = importlib.util.module_from_spec(spec)
spec.loader.exec_module(DEPLOY)

class HostTests(unittest.TestCase):
    def test_remote_status_is_required_even_when_adb_reports_success(self):
        for text in ['failure', '\n__U60_DASH_RC__1\n']:
            with self.assertRaises(DEPLOY.Failure):
                DEPLOY.decode(subprocess.CompletedProcess([], 0, text, ''))
        self.assertEqual(DEPLOY.decode(subprocess.CompletedProcess([], 0, 'safe\r\n__U60_DASH_RC__0\r\n', '')), 'safe')
    def test_remote_shell_is_quoted_as_code_not_interpolated_data(self):
        result = subprocess.run(['sh', '-c', DEPLOY.shell("printf '%s' 'literal $HOME and `false`'" )], capture_output=True, text=True)
        self.assertEqual(DEPLOY.decode(result), 'literal $HOME and `false`')
    def test_dynamic_and_foreign_architecture_artifacts_are_refused(self):
        with tempfile.TemporaryDirectory() as root:
            p=Path(root)/'agent';b=bytearray(120);b[:6]=b'\x7fELF\x02\x01';struct.pack_into('<H',b,18,183);struct.pack_into('<Q',b,32,64);struct.pack_into('<HH',b,54,56,1);p.write_bytes(b);DEPLOY.elf(p)
            struct.pack_into('<I',b,64,3);p.write_bytes(b)
            with self.assertRaises(DEPLOY.Failure):DEPLOY.elf(p)
            struct.pack_into('<H',b,18,62);p.write_bytes(b)
            with self.assertRaises(DEPLOY.Failure):DEPLOY.elf(p)

class DeviceTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='u60-pair-test-');self.addCleanup(self.temp.cleanup)
        self.base=Path(self.temp.name);self.root=self.base/'data';self.root.mkdir();self.run=self.base/'run'
        self.start=self.base/'startup.sh';self.legacy=self.base/'zte-agent';self.vendor=self.base/'vendor-up';self.vendor.touch();self.light=self.base/'brightness';self.light.write_text('23\n');self.bin=self.base/'bin';self.bin.mkdir()
        self.env={**os.environ,'PATH':str(self.bin)+':'+os.environ['PATH'],'TEST_ROOT':str(self.root),'TEST_RUN':str(self.run),'TEST_VENDOR':str(self.vendor),'TEST_LIGHT':str(self.light)}
        self.tool('id','#!/bin/sh\necho 0\n')
        self.tool('ubus','#!/bin/sh\nprintf \'{"noSyncModuleName":"%s"}\\n\' "${TEST_SYNC:-sync success}"\n')
        self.tool('pidof','#!/bin/sh\ntest -f "$TEST_VENDOR" && echo 1\n')
        self.tool('sync','#!/bin/sh\nexit 0\n')
        self.tool('sleep','#!/usr/bin/python3\nimport sys,time\ntime.sleep(float(sys.argv[1])*0.03)\n')
        self.tool('cut','#!/usr/bin/python3\nimport os,sys,time\nif sys.argv[-1]=="/proc/uptime": print(int(time.monotonic()*30))\nelse: os.execv("/usr/bin/cut",sys.argv)\n')
        self.vendor_script=self.base/'vendor';self.vendor_script.write_text('''#!/bin/sh
case "$1" in stop) rm -f "$TEST_VENDOR";; start|restart) touch "$TEST_VENDOR";; *) exit 9;; esac
''');self.vendor_script.chmod(0o700)
        manager=(REPO/'scripts/dash/manage.sh').read_text()
        manager=manager.replace('ROOT=/data/u60-dash',f'ROOT={self.root}').replace('RUN=/tmp/u60-dash-run',f'RUN={self.run}').replace('START=/data/local/tmp/start_zte_agent.sh',f'START={self.start}').replace('LEGACY=/data/zte-agent',f'LEGACY={self.legacy}')
        manager=manager.replace('grep -qF "$START" /etc/rc.local','grep -qF "$START" '+str(self.base/'rc.local')).replace('/data/local/tmp/zte-agent',str(self.base/'absent-agent')).replace('/data/open-u60/update/pending',str(self.base/'no-ota')).replace('/tmp/u60-native-screen-trial',str(self.base/'no-trial'))
        self.manager=self.root/'manage.sh';self.manager.write_text(manager)
        self.service=(REPO/'scripts/dash/service.sh').read_text().replace('ROOT=/data/u60-dash',f'ROOT={self.root}').replace('RUN=/tmp/u60-dash-run',f'RUN={self.run}').replace('VENDOR=/etc/init.d/zte_topsw_devui',f'VENDOR={self.vendor_script}').replace('BACKLIGHT=/sys/class/leds/led:lcd/brightness',f'BACKLIGHT={self.light}')
        self.service=self.service.replace('PREFS=/data/local/tmp/u60-native-screen/appearance.json',f'PREFS={self.base}/appearance.json')
        self.native=self.base/'fake.c';self.native.write_text(r'''
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>
#include <signal.h>
static void writefile(const char *dir,const char *name,const char *s){char p[512];snprintf(p,sizeof p,"%s/%s",dir,name);FILE*f=fopen(p,"w");if(!f)exit(8);fputs(s,f);fclose(f);}
int main(int argc,char**argv){
 char root[512];snprintf(root,sizeof root,"%s",getenv("TEST_ROOT"));
 if(argc>1&&!strcmp(argv[1],"--version")){puts("example 1.0");return 0;}
 if(argc>1&&!strcmp(argv[1],"dash-export-env")){writefile(root,"agent.env","export ZTE_AGENT_PASSWORD='example-only'\n");writefile(root,"health.json","{}");return 0;}
 if(argc>1&&!strcmp(argv[1],"dash-health"))return getenv("TEST_BAD_HEALTH")?1:0;
 if(argc>1&&!strcmp(argv[1],"--probe-agent"))return getenv("TEST_BAD_PROBE")?1:0;
 int screen=strstr(argv[0],"u60-screen")!=NULL;
 if(screen){if(getenv("TEST_BAD_SCREEN"))return 17;fprintf(stderr,"display: first frame submitted\n");fflush(stderr);FILE*f=fopen(getenv("TEST_LIGHT"),"w");fputs("99\n",f);fclose(f);}
 for(int n=1;;n++){if(screen&&!getenv("TEST_HUNG_SCREEN")){char text[64];snprintf(text,sizeof text,"%d",n);writefile(getenv("TEST_RUN"),"heartbeat",text);}usleep(10000);}
}
''')
        subprocess.run([shutil.which('cc') or 'cc',str(self.native),'-o',str(self.legacy)],check=True)
        self.start.write_text(f'#!/bin/sh\nnohup {self.legacy} >/dev/null 2>&1 </dev/null &\n')
        self.start.chmod(0o700);(self.base/'rc.local').write_text(f'sh {self.start}\nexit 0\n')
        self.original_start=self.start.read_bytes();self.original_hash=hashlib.sha256(self.legacy.read_bytes()).hexdigest()
        self.old=subprocess.Popen([str(self.legacy)],env=self.env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        self.addCleanup(self.cleanup_processes)
    def tool(self,name,content):
        p=self.bin/name;p.write_text(content);p.chmod(0o700)
    def cleanup_processes(self):
        for folder in Path('/proc').iterdir():
            if not folder.name.isdigit():continue
            try:
                cmd=folder.joinpath('cmdline').read_bytes()
                if str(self.base).encode() in cmd and int(folder.name)!=os.getpid():os.kill(int(folder.name),signal.SIGKILL)
            except (OSError,ProcessLookupError):pass
        try:self.old.wait(timeout=2)
        except subprocess.TimeoutExpired:pass
    def release(self,name):
        folder=self.root/'releases'/name;folder.mkdir(parents=True)
        for binary in ['zte-agent','u60-screen']:
            shutil.copyfile(self.legacy,folder/binary);(folder/binary).chmod(0o700)
        (folder/'service.sh').write_text(self.service);(folder/'service.sh').chmod(0o700)
        files=['zte-agent','u60-screen','service.sh'];(folder/'manifest.sha256').write_text(''.join(hashlib.sha256((folder/f).read_bytes()).hexdigest()+'  '+f+'\n' for f in files))
        return folder
    def manage(self,command,*args,env=None):
        return subprocess.run(['sh',str(self.manager),command,*args],env=env or self.env,capture_output=True,text=True,timeout=15)
    def wait(self,predicate):
        until=time.monotonic()+8
        while time.monotonic()<until:
            if predicate():return
            time.sleep(.03)
        logs='\n'.join(p.read_text(errors='replace') for p in self.root.glob('*.log'))
        self.fail(logs+'\n'+self.manage('status').stdout)
    def installed(self,name):
        self.wait(lambda:(self.run/'ready').exists() and not (self.root/'pending').exists() and (self.root/'current').read_text().strip()==name)
    def test_install_update_rollback_stop_start_and_restore(self):
        self.release('one');r=self.manage('install','one');self.assertEqual(r.returncode,0,r.stdout+r.stderr);self.installed('one')
        self.assertEqual(self.start.read_text().count('manage.sh boot'),1);self.assertFalse(self.vendor.exists())
        self.release('two');r=self.manage('install','two');self.assertEqual(r.returncode,0,r.stdout+r.stderr);self.installed('two')
        self.assertEqual((self.root/'previous').read_text().strip(),'one')
        r=self.manage('rollback');self.assertEqual(r.returncode,0,r.stdout+r.stderr);self.installed('one')
        r=self.manage('stop');self.assertEqual(r.returncode,0,r.stdout+r.stderr);self.assertTrue(self.vendor.exists());self.assertFalse(self.run.exists());self.assertEqual(self.light.read_text(),'23\n');self.assertIn('agent=running',self.manage('status').stdout)
        r=self.manage('start');self.assertEqual(r.returncode,0,r.stdout+r.stderr);self.wait(lambda:(self.run/'ready').exists())
        r=self.manage('restore');self.assertEqual(r.returncode,0,r.stdout+r.stderr);self.assertEqual(self.start.read_bytes(),self.original_start);self.assertEqual(hashlib.sha256(self.legacy.read_bytes()).hexdigest(),self.original_hash);self.assertTrue(self.vendor.exists())
    def test_bad_screen_returns_to_original_pair_without_losing_startup(self):
        self.release('bad');r=self.manage('install','bad',env={**self.env,'TEST_BAD_SCREEN':'1'});self.assertEqual(r.returncode,0,r.stdout+r.stderr)
        self.wait(lambda:(self.root/'current').exists() and (self.root/'current').read_text().strip()=='none' and not (self.root/'pending').exists())
        self.assertEqual(self.start.read_bytes(),self.original_start);self.assertTrue(self.vendor.exists());self.assertEqual(self.light.read_text(),'23\n')
    def test_failed_authenticated_probe_never_takes_over_display(self):
        self.release('bad');r=self.manage('install','bad',env={**self.env,'TEST_BAD_PROBE':'1'});self.assertEqual(r.returncode,0,r.stdout+r.stderr)
        self.wait(lambda:(self.root/'current').exists() and (self.root/'current').read_text().strip()=='none' and not (self.root/'pending').exists())
        self.assertTrue(self.vendor.exists());self.assertEqual(self.light.read_text(),'23\n')
        result=self.manage('restore');self.assertEqual(result.returncode,0);self.assertIn('already selected',result.stdout)
    def test_killed_supervisor_recovers_vendor_and_keeps_admin_agent(self):
        self.release('one');self.manage('install','one');self.installed('one')
        agent_pid=int((self.run/'agent').read_text())
        os.kill(int((self.run/'owner').read_text()),signal.SIGKILL)
        self.wait(lambda:not self.run.exists() and self.vendor.exists());self.assertEqual(self.light.read_text(),'23\n')
        os.kill(agent_pid,0)
        self.assertIn('agent=running',self.manage('status').stdout)
    def test_failed_network_health_returns_to_previous_pair(self):
        self.release('one');self.manage('install','one');self.installed('one')
        self.release('two');r=self.manage('install','two',env={**self.env,'TEST_BAD_HEALTH':'1'});self.assertEqual(r.returncode,0,r.stdout+r.stderr)
        self.installed('one')
        self.assertIn('was rolled back',self.manage('status').stdout)
    def test_frozen_screen_loop_restores_vendor(self):
        self.release('one');self.manage('install','one');self.installed('one')
        agent_pid=int((self.run/'agent').read_text())
        os.kill(int((self.run/'screen').read_text()),signal.SIGSTOP)
        self.wait(lambda:not self.run.exists() and self.vendor.exists())
        os.kill(agent_pid,0)
    def test_frozen_supervisor_cannot_resume_after_recovery(self):
        self.release('one');self.manage('install','one');self.installed('one')
        owner=int((self.run/'owner').read_text())
        os.kill(owner,signal.SIGSTOP)
        self.wait(lambda:not self.run.exists() and self.vendor.exists())
        cmd=Path(f'/proc/{owner}/cmdline')
        self.assertTrue(not cmd.exists() or not cmd.read_bytes())
    def test_relative_ota_link_is_preserved_with_its_binary(self):
        self.old.terminate();self.old.wait()
        original=self.base/'slot-a'/'zte-agent';original.parent.mkdir();self.legacy.rename(original)
        self.legacy.symlink_to('slot-a/zte-agent')
        self.old=subprocess.Popen([str(original)],env=self.env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        self.release('one');self.manage('install','one');self.installed('one')
        r=self.manage('restore');self.assertEqual(r.returncode,0,r.stdout+r.stderr)
        self.assertTrue(self.legacy.is_symlink());self.assertEqual(os.readlink(self.legacy),'slot-a/zte-agent')
        self.assertEqual(hashlib.sha256(self.legacy.read_bytes()).hexdigest(),self.original_hash)
    def test_power_loss_marker_selects_original_before_starting_candidate(self):
        self.release('one');self.manage('install','one');self.installed('one');self.manage('stop')
        (self.root/'pending').write_text('broken\nnone\n')
        r=self.manage('boot');self.assertEqual(r.returncode,0,r.stdout+r.stderr);self.assertEqual(self.start.read_bytes(),self.original_start);self.assertEqual((self.root/'current').read_text().strip(),'none')
    def test_checksums_fail_before_any_startup_change(self):
        folder=self.release('bad');(folder/'u60-screen').write_bytes(b'broken')
        r=self.manage('install','bad');self.assertNotEqual(r.returncode,0);self.assertEqual(self.start.read_bytes(),self.original_start);self.assertFalse((self.root/'pending').exists())

if __name__=='__main__':unittest.main()
