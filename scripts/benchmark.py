"""Measure Ledgeling against the performance budget.

    python3 scripts/benchmark.py 600            # the app and every helper it owns
    python3 scripts/benchmark.py 600 --native   # the Rust process only

The budget in doc 02 is about what the user's machine pays, so the default
includes the WebKit helpers. On macOS those (com.apple.WebKit.WebContent,
.Networking, .GPU) are XPC services parented to launchd, so walking ps
parent/child links finds nothing. The process set is therefore the app, its
descendants, and every process whose *responsible pid* is the app
(`responsibility_get_pid_responsible_for_pid` from libSystem).

Memory is the per-process physical footprint, the number Activity Monitor shows
in its Memory column, from `footprint --json`, summed over the set. RSS is kept
alongside but over-counts shared pages and is not the user's cost. Footprint is
slow, so it is sampled every FOOTPRINT_EVERY seconds. Near the end the app
process's footprint category summary (dirty/clean, malloc, IOSurface, ...) is
recorded. CPU is the sum of ps %cpu over the set; 'cpu_native_*' is the app
process alone.

WindowServer's share is still not attributable to one app and is not counted;
that is stated in the report it writes.
"""
import ctypes
import json
import os
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

root = Path(__file__).resolve().parents[1]
duration = int(sys.argv[1]) if len(sys.argv) > 1 and sys.argv[1].isdigit() else 600
native_only = '--native' in sys.argv
default_binary = root / 'src-tauri/target/release/bundle/macos/Ledgeling.app/Contents/MacOS/ledgeling'
bench_binary_env = os.environ.get('LEDGELING_BENCH_BINARY') or os.environ.get('PERCH_BENCH_BINARY')
binary = Path(bench_binary_env if bench_binary_env else str(default_binary))


FOOTPRINT_EVERY = 10
MIB = 1024 * 1024

try:
    _responsible = ctypes.CDLL('/usr/lib/libSystem.B.dylib').responsibility_get_pid_responsible_for_pid
    _responsible.argtypes = [ctypes.c_int]
    _responsible.restype = ctypes.c_int
except (OSError, AttributeError):
    _responsible = None


def process_table():
    """pid -> (ppid, short name) for every process."""
    out = subprocess.run(['ps', '-Ao', 'pid=,ppid=,comm='], capture_output=True, text=True).stdout
    table = {}
    for line in out.splitlines():
        parts = line.split(None, 2)
        if len(parts) == 3 and parts[0].isdigit() and parts[1].isdigit():
            table[int(parts[0])] = (int(parts[1]), os.path.basename(parts[2]))
    return table


def owned_processes(pid):
    """The app, its descendants, and processes it is responsible for (WebKit XPC helpers)."""
    table = process_table()
    children = {}
    for one, (parent, _) in table.items():
        children.setdefault(parent, []).append(one)
    found, queue = [pid], [pid]
    while queue:
        for child in children.get(queue.pop(), []):
            found.append(child)
            queue.append(child)
    if _responsible is not None:
        owner = _responsible(pid)
        for one in table:
            if one in found or _responsible(one) != owner:
                continue
            # Launched from a terminal the app is responsible to the terminal, so
            # accept only WebKit helpers there; launched by launchd it owns itself.
            if owner == pid or 'com.apple.WebKit' in table[one][1]:
                found.append(one)
    return [(one, table[one][1]) for one in found if one in table]


def sample(pid):
    procs = [(pid, 'ledgeling')] if native_only else owned_processes(pid)
    cpu = rss = native_cpu = 0.0
    for one, _ in procs:
        out = subprocess.run(['ps', '-p', str(one), '-o', '%cpu=,rss='],
                             capture_output=True, text=True).stdout.split()
        if len(out) == 2:
            cpu += float(out[0])
            rss += int(out[1])
            if one == pid:
                native_cpu = float(out[0])
    return cpu, rss, procs, native_cpu


def footprints(procs):
    """pid -> footprint bytes, and the full per-process JSON entries."""
    with tempfile.TemporaryDirectory() as tmp:
        target = Path(tmp) / 'fp.json'
        args = ['footprint', '-f', 'bytes', '-j', str(target)]
        for one, _ in procs:
            args += ['-p', str(one)]
        subprocess.run(args, capture_output=True, text=True)
        try:
            entries = json.loads(target.read_text()).get('processes', [])
        except (OSError, ValueError):
            return {}, {}
    return {e['pid']: e['footprint'] for e in entries}, {e['pid']: e for e in entries}


if not binary.exists():
    sys.exit(f'{binary} does not exist. Run: npm run tauri build')

class Launched:
    """The app, started through LaunchServices when it is inside a .app bundle.

    `open` makes the app its own responsible process, which is what lets the
    helper lookup attribute WebKit helpers to it alone. Started straight from
    this script the app is responsible to the terminal and helpers can only be
    matched by name, which also catches other apps' helpers started from it.
    """

    def __init__(self):
        bundle = next((p for p in binary.parents if p.suffix == '.app'), None)
        self.popen = None
        self.pid = None
        if sys.platform == 'darwin' and bundle is not None and binary == bundle / 'Contents/MacOS' / binary.name:
            subprocess.run(['open', '-n', '--env', 'LEDGELING_DIAGNOSTICS=1', '--env', 'PERCH_DIAGNOSTICS=1',
                            str(bundle)], check=True)
            for _ in range(50):
                time.sleep(0.2)
                out = subprocess.run(['pgrep', '-nf', str(binary)], capture_output=True, text=True).stdout.split()
                if out:
                    self.pid = int(out[0])
                    break
            if self.pid is None:
                sys.exit('open started the app but no process was found')
        else:
            self.popen = subprocess.Popen([str(binary)], env={**os.environ, 'LEDGELING_DIAGNOSTICS': '1',
                                                              'PERCH_DIAGNOSTICS': '1'}, stderr=subprocess.DEVNULL)
            self.pid = self.popen.pid

    def poll(self):
        if self.popen:
            return self.popen.poll()
        return None if subprocess.run(['ps', '-p', str(self.pid)], capture_output=True).returncode == 0 else -1

    returncode = property(lambda self: self.popen.returncode if self.popen else -1)

    def terminate(self):
        if self.popen:
            self.popen.terminate()
        else:
            os.kill(self.pid, 15)

    def wait(self, timeout):
        if self.popen:
            self.popen.wait(timeout=timeout)
            return
        end = time.time() + timeout
        while self.poll() is None:
            if time.time() > end:
                raise subprocess.TimeoutExpired('ledgeling', timeout)
            time.sleep(0.2)

    def kill(self):
        if self.popen:
            self.popen.kill()
        else:
            os.kill(self.pid, 9)


process = Launched()
rows = []
names = {}
fp_by_name = {}
categories = None
try:
    for second in range(duration):
        time.sleep(1)
        if process.poll() is not None:
            raise RuntimeError(f'Ledgeling exited early with {process.returncode}')
        cpu, rss, procs, native_cpu = sample(process.pid)
        row = {'second': second + 1, 'cpu': cpu, 'cpu_native': native_cpu, 'rss_kib': rss,
               'processes': len(procs)}
        names.update(dict(procs))
        last = second + 1 == duration
        if (second + 1) % FOOTPRINT_EVERY == 0 or last:
            fps, entries = footprints(procs)
            row['footprint_bytes'] = sum(fps.values())
            row['footprint_by_process'] = {f'{names[p]}[{p}]': b for p, b in fps.items()}
            if last and process.pid in entries:
                categories = {k: v for k, v in entries[process.pid].get('categories', {}).items()}
        rows.append(row)
        if (second + 1) % 30 == 0:
            fp = next((r['footprint_bytes'] for r in reversed(rows) if 'footprint_bytes' in r), None)
            fp_text = f'  footprint {fp / MIB:.1f} MiB' if fp is not None else ''
            print(f'{second + 1}s  CPU {cpu:.1f}% (app {native_cpu:.1f}%)  RSS {rss / 1024:.1f} MiB'
                  f'{fp_text}  ({len(procs)} processes: {", ".join(sorted(set(n for _, n in procs)))})',
                  flush=True)
finally:
    process.terminate()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
    scope = 'native process only' if native_only else 'whole process tree, excluding WindowServer'
    cpus = [r['cpu'] for r in rows]
    native_cpus = [r.get('cpu_native', 0.0) for r in rows]
    fps = [r['footprint_bytes'] for r in rows if 'footprint_bytes' in r]
    breakdown = {}
    for r in rows:
        for key, value in r.get('footprint_by_process', {}).items():
            breakdown.setdefault(key, []).append(value)
    breakdown_mib = {k: {'mean': statistics.mean(v) / MIB, 'peak': max(v) / MIB}
                     for k, v in sorted(breakdown.items())}
    report = {
        'duration_seconds': len(rows),
        'scope': scope,
        'cpu_mean': statistics.mean(cpus) if cpus else None,
        'cpu_median': statistics.median(cpus) if cpus else None,
        'cpu_p95': sorted(cpus)[int(len(cpus) * 0.95)] if cpus else None,
        'rss_peak_mib': max((r['rss_kib'] for r in rows), default=0) / 1024,
        'cpu_native_mean': statistics.mean(native_cpus) if native_cpus else None,
        'footprint_peak_mib': max(fps) / MIB if fps else None,
        'footprint_mean_mib': statistics.mean(fps) / MIB if fps else None,
        'footprint_by_process_mib': breakdown_mib,
        'app_footprint_categories_bytes': categories,
        'samples': rows,
    }
    name = 'benchmark-native.json' if native_only else 'benchmark.json'
    (root / 'docs' / name).write_text(json.dumps(report, indent=2) + '\n')
    if fps:
        print(f'\nFootprint (sampled every {FOOTPRINT_EVERY}s): peak {max(fps) / MIB:.1f} MiB, '
              f'mean {statistics.mean(fps) / MIB:.1f} MiB; RSS peak {report["rss_peak_mib"]:.1f} MiB')
        for key, v in breakdown_mib.items():
            print(f'  {key:<40} mean {v["mean"]:7.1f}  peak {v["peak"]:7.1f} MiB')
    print(f'CPU mean {report["cpu_mean"]:.2f}% (app only {report["cpu_native_mean"]:.2f}%)' if cpus else '')
    if categories:
        print('App footprint categories (dirty / clean / swapped MiB):')
        top = sorted(categories.items(), key=lambda kv: -(kv[1].get('dirty', 0) + kv[1].get('clean', 0)))[:10]
        for key, v in top:
            print(f'  {key:<32} {v.get("dirty", 0) / MIB:7.1f} {v.get("clean", 0) / MIB:7.1f} '
                  f'{v.get("swapped", 0) / MIB:7.1f}')
    print(f'\nWrote docs/{name}')
