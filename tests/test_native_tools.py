"""Real owned-process tests. No game process is opened or terminated."""
import json
import os
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import time
import unittest

from hoi4companion.native_tools import ManagedTool,engine_info

# Windows Store activation plus a child interpreter can exceed five seconds
# under concurrent compilation. This bounds fixture startup only: cancellation
# and the supervisor's 150 ms deadline retain their independent strict checks.
FIXTURE_READY_SECONDS=15


@unittest.skipUnless(os.name=='nt','Windows job objects')
class NativeToolTests(unittest.TestCase):
    def test_exact_arguments_unicode_and_shell_metacharacters_are_never_interpreted(self):
        self.assertEqual(engine_info()['engine'],'rust-tools')
        args=['spaces here','quote"inside','trailing\\','é Łódź','& whoami | > not-a-command','']
        with ManagedTool([sys.executable,'-c','import json,sys;print(json.dumps(sys.argv[1:]))',*args]) as tool:
            out,err=tool.communicate(timeout=FIXTURE_READY_SECONDS)
            self.assertEqual(json.loads(out),args);self.assertEqual(err,'');self.assertEqual(tool.returncode,0)

    def test_nonzero_exit_stderr_and_immediate_exit_remain_distinct(self):
        with ManagedTool([sys.executable,'-c','import sys;print("problem",file=sys.stderr);sys.exit(7)']) as tool:
            out,err=tool.communicate(timeout=FIXTURE_READY_SECONDS)
            self.assertEqual(out,'');self.assertIn('problem',err);self.assertEqual(tool.returncode,7)

    def test_deadline_kills_own_tree_and_reaps_the_child(self):
        with ManagedTool([sys.executable,'-c','import time;time.sleep(30)'],max_seconds=.15) as tool:
            tool.wait(timeout=FIXTURE_READY_SECONDS)
            self.assertIsNotNone(tool.poll());self.assertEqual(tool.status()['reason'],'timeout')

    def test_cancel_close_and_parent_exit_kill_descendants(self):
        import ctypes
        kernel=ctypes.WinDLL('kernel32');kernel.OpenProcess.argtypes=[ctypes.c_uint32,ctypes.c_bool,ctypes.c_uint32];kernel.OpenProcess.restype=ctypes.c_void_p
        kernel.WaitForSingleObject.argtypes=[ctypes.c_void_p,ctypes.c_uint32];kernel.CloseHandle.argtypes=[ctypes.c_void_p]
        for terminate in ('cancel','close','parent_exit'):
            with self.subTest(terminate=terminate),TemporaryDirectory() as folder:
                pidfile=Path(folder)/'child.txt'
                exitfile=Path(folder)/'parent-exit'
                child='import os,time;from pathlib import Path;p=Path('+repr(str(pidfile))+');t=p.with_suffix(".tmp");t.write_text(str(os.getpid()));t.replace(p);time.sleep(30)'
                parent='import subprocess,sys,time;from pathlib import Path;subprocess.Popen([sys.executable,"-c",'+repr(child)+'])\nwhile not Path('+repr(str(exitfile))+').exists(): time.sleep(.01)'
                tool=ManagedTool([sys.executable,'-c',parent])
                try:
                    deadline=time.monotonic()+FIXTURE_READY_SECONDS
                    while not pidfile.exists() and time.monotonic()<deadline:time.sleep(.01)
                    self.assertTrue(pidfile.exists());pid=int(pidfile.read_text())
                    handle=kernel.OpenProcess(0x100000,False,pid);self.assertTrue(handle)
                    try:
                        if terminate=='cancel':tool.kill()
                        elif terminate=='close':tool.close()
                        else:exitfile.touch();tool.wait(timeout=FIXTURE_READY_SECONDS)
                        self.assertEqual(kernel.WaitForSingleObject(handle,5000),0)
                    finally:kernel.CloseHandle(handle)
                finally:tool.close()

    def test_output_limit_stops_writer_and_releases_files(self):
        with ManagedTool([sys.executable,'-u','-c','import sys,time\nwhile True: sys.stdout.write("x"*4096);sys.stdout.flush();time.sleep(.01)'],max_output_bytes=8192) as tool:
            tool.wait(timeout=FIXTURE_READY_SECONDS)
            self.assertEqual(tool.status()['reason'],'output_limit')

    def test_output_limit_is_checked_even_when_writer_has_already_exited(self):
        # A burst can finish between two monitor ticks. Completion is not an
        # exemption from the output bound, including externally supplied logs.
        with ManagedTool([sys.executable,'-c','import sys;sys.stdout.write("x"*1048576)'],max_output_bytes=8192) as tool:
            tool.wait(timeout=FIXTURE_READY_SECONDS)
            self.assertEqual(tool.status()['reason'],'output_limit')

    def test_cancel_is_scoped_to_owned_handle_and_other_tool_survives(self):
        with ManagedTool([sys.executable,'-c','import time;time.sleep(30)']) as first,ManagedTool([sys.executable,'-c','import time;time.sleep(30)']) as second:
            first.kill();self.assertIsNotNone(first.poll());self.assertIsNone(second.poll())

    def test_external_log_and_missing_executable_have_explicit_lifetimes(self):
        with TemporaryDirectory() as folder:
            path=Path(folder)/'tool.log'
            with path.open('w',encoding='utf8') as log:
                with ManagedTool([sys.executable,'-c','print("ok")'],stdout=log,stderr=log) as tool:self.assertEqual(tool.wait(timeout=FIXTURE_READY_SECONDS),0)
            self.assertEqual(path.read_text().strip(),'ok')
        with self.assertRaises(ValueError):ManagedTool(['missing-tool-865489.exe'])
