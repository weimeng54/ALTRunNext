"""Bound synthetic Windows test runs and retain partial logs on a GUI deadlock."""
import pathlib, subprocess, sys, threading
sys.stdout.reconfigure(encoding='utf-8', errors='replace')

path = pathlib.Path('target/test-progress.log')
path.parent.mkdir(exist_ok=True)
with path.open('w', encoding='utf-8') as log:
    process = subprocess.Popen(['cargo', 'test', '--locked', '--no-fail-fast'], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding='utf-8', errors='replace', bufsize=1)
    def stream():
        for line in process.stdout:
            log.write(line)
            log.flush()
            print(line, end='', flush=True)
    reader = threading.Thread(target=stream, daemon=True)
    reader.start()
    try:
        result = process.wait(timeout=180)
    except subprocess.TimeoutExpired:
        print('Synthetic test timeout: terminating this test process tree.', flush=True)
        subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
        process.wait(timeout=15)
        result = 1
    reader.join(timeout=10)
sys.exit(result)
