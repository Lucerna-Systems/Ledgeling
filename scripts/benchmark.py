"""Measure the native process, excluding WindowServer and WebKit helpers.
Usage: python3 scripts/benchmark.py small 600
"""
import subprocess, time, os, sys, json, statistics
from pathlib import Path
root = Path(__file__).resolve().parents[1]
mode = sys.argv[1] if len(sys.argv)>1 else 'small'
duration = int(sys.argv[2]) if len(sys.argv)>2 else 600
binary = Path(os.environ.get('PERCH_BENCH_BINARY', str(root / 'src-tauri/target/release/bundle/macos/Perch.app/Contents/MacOS/perch')))
p = subprocess.Popen([str(binary)], env={**os.environ, 'PERCH_RENDER_SPIKE': mode})
rows = []
try:
    for i in range(duration):
        time.sleep(1)
        if p.poll() is not None: raise RuntimeError(f'Perch exited: {p.returncode}')
        output = subprocess.check_output(['ps','-p',str(p.pid),'-o','%cpu=,rss=']).decode().split()
        rows.append({'second':i+1,'cpu':float(output[0]),'rss_kib':int(output[1])})
        if (i+1)%30 == 0: print(f'{mode} {i+1}s CPU {output[0]}% RSS {int(output[1])/1024:.1f} MiB',flush=True)
finally:
    p.terminate()
    p.wait(timeout=10)
    report = {'mode':mode,'duration_seconds':len(rows),'scope':'native process only; excludes WebKit and WindowServer','cpu_mean':statistics.mean(r['cpu'] for r in rows) if rows else None,'samples':rows}
    (root/'docs'/f'benchmark-{mode}.json').write_text(json.dumps(report,indent=2)+'\n')
