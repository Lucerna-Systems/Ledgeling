"""Measure Perch against the performance budget.

    python3 scripts/benchmark.py 600            # whole process tree
    python3 scripts/benchmark.py 600 --native   # the Rust process only

The budget in doc 02 is about what the user's machine pays, so the default now
includes the WebKit helpers. WindowServer's share is still not attributable to
one app and is not counted; that is stated in the report it writes.
"""
import json
import os
import statistics
import subprocess
import sys
import time
from pathlib import Path

root = Path(__file__).resolve().parents[1]
duration = int(sys.argv[1]) if len(sys.argv) > 1 and sys.argv[1].isdigit() else 600
native_only = '--native' in sys.argv
default_binary = root / 'src-tauri/target/release/bundle/macos/Perch.app/Contents/MacOS/perch'
binary = Path(os.environ.get('PERCH_BENCH_BINARY', str(default_binary)))


def descendants(pid):
    """Every process in the tree, so WebKit helpers are counted."""
    out = subprocess.run(['ps', '-Ao', 'pid=,ppid='], capture_output=True, text=True).stdout
    children = {}
    for line in out.splitlines():
        parts = line.split()
        if len(parts) == 2:
            children.setdefault(int(parts[1]), []).append(int(parts[0]))
    found, queue = [pid], [pid]
    while queue:
        for child in children.get(queue.pop(), []):
            found.append(child)
            queue.append(child)
    return found


def sample(pid):
    pids = [pid] if native_only else descendants(pid)
    cpu = rss = 0.0
    for one in pids:
        out = subprocess.run(['ps', '-p', str(one), '-o', '%cpu=,rss='],
                             capture_output=True, text=True).stdout.split()
        if len(out) == 2:
            cpu += float(out[0])
            rss += int(out[1])
    return cpu, rss, len(pids)


if not binary.exists():
    sys.exit(f'{binary} does not exist. Run: npm run tauri build')

process = subprocess.Popen([str(binary)], env={**os.environ, 'PERCH_DIAGNOSTICS': '1'},
                           stderr=subprocess.DEVNULL)
rows = []
try:
    for second in range(duration):
        time.sleep(1)
        if process.poll() is not None:
            raise RuntimeError(f'Perch exited early with {process.returncode}')
        cpu, rss, count = sample(process.pid)
        rows.append({'second': second + 1, 'cpu': cpu, 'rss_kib': rss, 'processes': count})
        if (second + 1) % 30 == 0:
            print(f'{second + 1}s  CPU {cpu:.1f}%  RSS {rss / 1024:.1f} MiB  ({count} processes)',
                  flush=True)
finally:
    process.terminate()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
    scope = 'native process only' if native_only else 'whole process tree, excluding WindowServer'
    cpus = [r['cpu'] for r in rows]
    report = {
        'duration_seconds': len(rows),
        'scope': scope,
        'cpu_mean': statistics.mean(cpus) if cpus else None,
        'cpu_median': statistics.median(cpus) if cpus else None,
        'cpu_p95': sorted(cpus)[int(len(cpus) * 0.95)] if cpus else None,
        'rss_peak_mib': max((r['rss_kib'] for r in rows), default=0) / 1024,
        'samples': rows,
    }
    name = 'benchmark-native.json' if native_only else 'benchmark.json'
    (root / 'docs' / name).write_text(json.dumps(report, indent=2) + '\n')
    print(f'\nWrote docs/{name}')
