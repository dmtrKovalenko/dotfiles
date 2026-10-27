#!/usr/bin/env python3
"""Exercise the installed remote plugin through a Mac Herdr SSH client."""
import argparse
import base64
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import shlex
import signal
import struct
import subprocess
import tempfile
import termios
import time
import uuid

SETUP = r'''
import json,os,shlex,subprocess,sys,tempfile,time
from pathlib import Path
name=sys.argv[1]
server_binary=sys.argv[2]
home=Path.home()
root=Path(tempfile.mkdtemp(prefix='copy-ssh-',dir='/tmp'))
herdr=str(home/'.local/bin/herdr')
env=dict(os.environ,HERDR_DISABLE_SOUND='1',HERDR_FINGERS_OPEN_STATE=str(root/'fingers-state'))
config=root/'config.toml'
config.write_text((home/'.config/herdr/config.toml').read_text()+'\n[terminal]\ndefault_shell = "/bin/sh"\n')
env['HERDR_CONFIG_PATH']=str(config)
with (root/'server.log').open('wb') as log:
    server=subprocess.Popen([server_binary or herdr,'--session',name,'server'],env=env,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
socket=home/'.config/herdr/sessions'/name/'herdr.sock'
deadline=time.monotonic()+10
while not socket.exists():
    if time.monotonic()>deadline: raise RuntimeError('test server did not start')
    time.sleep(.05)
def cli(*args):
    output=subprocess.check_output([herdr,'--session',name,*args],text=True)
    return json.loads(output)['result'] if output.strip() else None
cli('plugin','link',str(home/'.local/share/herdr/copy-search'))
workspace=cli('workspace','create','--cwd',str(root),'--label','SSH copy verification')
pane=workspace['root_pane']['pane_id']
fixture=root/'fixture.py'
fixture.write_text("import time\nprint(chr(27)+'[31mremote_token'+chr(27)+'[0m SSH_COPY_READY',flush=True)\ntime.sleep(120)\n")
cli('pane','run',pane,shlex.join(['/usr/bin/python3','-u',str(fixture)]))
print(json.dumps({'root':str(root),'pane':pane,'session_dir':str(socket.parent)}))
'''


def verify(host, server_binary):
    name='copy-verify-'+uuid.uuid4().hex[:8]
    herdr=str(Path.home()/'.local/bin/herdr')
    local=Path(tempfile.mkdtemp(prefix='copy-ssh-client-',dir='/tmp'))
    info=None
    child=master=None
    captured=bytearray()

    def pump_until(predicate, timeout=20):
        deadline=time.monotonic()+timeout
        while time.monotonic()<deadline:
            if predicate(): return
            if select.select([master],[],[],.05)[0]:
                captured.extend(os.read(master,65536))
        raise RuntimeError('Timed out waiting for SSH client output')

    try:
        setup=subprocess.run(['ssh','-o','BatchMode=yes',host,'python3 - '+shlex.quote(name)+' '+shlex.quote(server_binary or '')],
                             input=SETUP,text=True,capture_output=True,timeout=30)
        if setup.returncode:
            raise RuntimeError(setup.stderr or setup.stdout)
        info=json.loads(setup.stdout)
        env={key:value for key,value in os.environ.items() if not key.startswith('HERDR_')}
        env.pop('NO_COLOR',None)
        env.update(XDG_CONFIG_HOME=str(local/'config'),XDG_STATE_HOME=str(local/'state'),
                   XDG_CACHE_HOME=str(local/'cache'),XDG_DATA_HOME=str(local/'data'),
                   TERM='xterm-256color',COLORTERM='truecolor',SSH_TTY='/dev/test-osc52')
        config=local/'config/herdr'
        config.mkdir(parents=True)
        (config/'config.toml').write_text('onboarding = false\n[ui.sound]\nenabled = false\n')
        child,master=pty.fork()
        if child==0:
            os.execve(herdr,[herdr,'--remote',host,'--session',name,'--remote-keybindings','server'],env)
        fcntl.ioctl(master,termios.TIOCSWINSZ,struct.pack('HHHH',40,140,0,0))
        pump_until(lambda:b'SSH_COPY_READY' in captured,timeout=35)
        captured.clear()
        start=time.monotonic()
        os.write(master,b'\x00[')
        pump_until(lambda:b'[copy]' in captured,timeout=5)
        elapsed=time.monotonic()-start
        assert b'48;2;255;158;100' in captured, 'orange cursor missing over SSH'
        for key in b'/remote_token\rgx':
            os.write(master,bytes([key]))
            time.sleep(.08)
        expected=b'\x1b]52;c;'+base64.b64encode(b'remote_token')+b'\x07'
        pump_until(lambda:expected in captured,timeout=10)
        print(f'PASS: {host}: Mac SSH client opened prefix+[ in {elapsed:.3f}s; orange cursor rendered; gx returned the exact clipboard payload through SSH.',flush=True)
    finally:
        (local/'client.ansi').write_bytes(captured)
        if master is not None: os.close(master)
        if child:
            try: os.kill(child,signal.SIGKILL)
            except ProcessLookupError: pass
            os.waitpid(child,0)
        cleanup='''import subprocess,sys,shutil\nfrom pathlib import Path\nname=sys.argv[1]\nassert name.startswith('copy-verify-')\nsubprocess.run([str(Path.home()/'.local/bin/herdr'),'--session',name,'server','stop'],capture_output=True,timeout=10)\nshutil.rmtree(Path.home()/'.config/herdr/sessions'/name,ignore_errors=True)\n'''
        subprocess.run(['ssh','-o','BatchMode=yes',host,'python3 - '+shlex.quote(name)],input=cleanup,text=True,capture_output=True,timeout=15)


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('host')
    parser.add_argument('--server-binary')
    args=parser.parse_args()
    verify(args.host,args.server_binary)
