"""Long stability run: memory drift and file-descriptor growth over hours.

    python3 scripts/soak.py 8          # eight hours

Leaves docs/soak.json behind. Between samples it does nothing to the machine, so
you can lock the screen, sleep the laptop, plug a monitor in and unplug it again;
that is the point of the exercise. A pass is a flat RSS line and a flat handle
count, plus a process that is still alive at the end.
"""
import json
import os
import statistics
import subprocess
import sys
import time
from pathlib import Path

root = Path(__file__).resolve().parents[1]
hours = float(sys.argv[1]) if len(sys.argv) > 1 else 8.0
interval = 60
default_binary = root / 'src-tauri/target/release/bundle/macos/Ledgeling.app/Contents/MacOS/ledgeling'
binary = Path(os.environ.get('PERCH_BENCH_BINARY', str(default_binary)))

if not binary.exists():
    sys.exit(f'{binary} does not exist. Run: npm run tauri build')


def handles(pid):
    """Open file descriptors: the cheap proxy for leaked OS resources."""
    out = subprocess.run(['lsof', '-p', str(pid)], capture_output=True, text=True).stdout
    return max(len(out.splitlines()) - 1, 0)


def rss_kib(pid):
    out = subprocess.run(['ps', '-p', str(pid), '-o', 'rss='], capture_output=True, text=True).stdout
    return int(out.strip() or 0)


process = subprocess.Popen([str(binary)], env={**os.environ, 'PERCH_DIAGNOSTICS': '1'},
                           stderr=subprocess.DEVNULL)
rows = []
start = time.time()
print(f'Soaking for {hours} h. Use the machine normally; lock it, sleep it, move windows.')
try:
    while time.time() - start < hours * 3600:
        time.sleep(interval)
        if process.poll() is not None:
            raise RuntimeError(f'Ledgeling exited after {(time.time() - start) / 60:.0f} minutes '
                               f'with code {process.returncode}')
        minute = (time.time() - start) / 60
        rows.append({'minute': round(minute, 1), 'rss_kib': rss_kib(process.pid),
                     'handles': handles(process.pid)})
        if len(rows) % 15 == 0:
            last = rows[-1]
            print(f"{last['minute']:.0f} min  RSS {last['rss_kib'] / 1024:.1f} MiB  "
                  f"{last['handles']} handles", flush=True)
finally:
    survived = process.poll() is None
    process.terminate()
    try:
        process.wait(timeout=15)
    except subprocess.TimeoutExpired:
        process.kill()
    if rows:
        first_hour = [r['rss_kib'] for r in rows[:60]] or [rows[0]['rss_kib']]
        last_hour = [r['rss_kib'] for r in rows[-60:]]
        drift = statistics.mean(last_hour) - statistics.mean(first_hour)
        report = {
            'hours': round((time.time() - start) / 3600, 2),
            'survived': survived,
            'rss_drift_mib': round(drift / 1024, 2),
            'rss_peak_mib': round(max(r['rss_kib'] for r in rows) / 1024, 2),
            'handles_start': rows[0]['handles'],
            'handles_end': rows[-1]['handles'],
            'samples': rows,
        }
        (root / 'docs' / 'soak.json').write_text(json.dumps(report, indent=2) + '\n')
        print(f"\nRSS drift {report['rss_drift_mib']} MiB · handles "
              f"{report['handles_start']} → {report['handles_end']} · wrote docs/soak.json")
