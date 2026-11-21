#!/usr/bin/env python3
"""Supervise one owned tiny evaluation, including host, cgroup, VRAM and GTT."""
import argparse
import fcntl
import json
import os
from pathlib import Path
import signal
import subprocess
import time


def memory_available():
    """Return Linux's available-memory estimate in bytes."""
    return int(next(line.split()[1] for line in Path('/proc/meminfo').read_text().splitlines()
                    if line.startswith('MemAvailable:'))) * 1024


def identity(pid):
    """Read a process birth token so reused PIDs are never signalled."""
    return Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()[19]


def run():
    """Admit and supervise a private process group under a finite no-swap cgroup."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', required=True)
    parser.add_argument('--unit', required=True)
    parser.add_argument('--gpu-pci', default='0000:0b:00.0')
    parser.add_argument('--phase-file')
    parser.add_argument('--soft-mib', type=int, default=1536)
    parser.add_argument('--hard-mib', type=int, default=2048)
    parser.add_argument('--seconds', type=float, default=120)
    parser.add_argument('--vram-increment-mib', type=int, default=512)
    parser.add_argument('--gtt-increment-mib', type=int, default=512)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    if not command:
        parser.error('command required')
    relative = Path('/proc/self/cgroup').read_text().strip().split('0::', 1)[1]
    if relative.rsplit('/', 1)[-1] != args.unit + '.scope' or not args.unit.startswith('toniator-gpu-eval-'):
        parser.error('requires an owned toniator-gpu-eval- scope')
    cgroup = Path('/sys/fs/cgroup') / relative.lstrip('/')
    hard = (cgroup / 'memory.max').read_text().strip()
    if hard == 'max' or int(hard) != args.hard_mib * 2**20:
        parser.error('unexpected hard cap')
    if (cgroup / 'memory.swap.max').read_text().strip() != '0':
        parser.error('swap must be disabled')
    if not 0 < args.soft_mib < args.hard_mib:
        parser.error('soft cap must be positive and below hard cap')
    devices = list(Path('/sys/class/drm').glob('card*/device/mem_info_vram_total'))
    if len(devices) != 1:
        parser.error('requires exactly one identified discrete GPU memory source')
    gpu = devices[0].parent
    if gpu.resolve().name != args.gpu_pci or (gpu / 'vendor').read_text().strip() != '0x1002':
        parser.error('physical AMD adapter does not match expected PCI address')
    total = int((gpu / 'mem_info_vram_total').read_text())
    reserve = max(total // 4, 4 * 2**30)
    host_reserve = 10 * 2**30
    vram_budget = args.vram_increment_mib * 2**20
    gtt_budget = args.gtt_increment_mib * 2**20
    if vram_budget <= 0 or gtt_budget <= 0:
        parser.error('positive planned device budgets required')
    if memory_available() < host_reserve + args.hard_mib * 2**20 + gtt_budget:
        parser.error('insufficient host admission headroom')
    start_vram = int((gpu / 'mem_info_vram_used').read_text())
    start_gtt = int((gpu / 'mem_info_gtt_used').read_text())
    if total - start_vram < reserve + vram_budget:
        parser.error('insufficient dedicated VRAM headroom')
    # One common lock covers every invocation of this harness, not benchmark workers.
    lock = open(Path(__file__).resolve().parent / '.workload.lock', 'a')
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    process = None
    started = time.monotonic()
    report = dict(command=command, pid=None, start_ticks=None, cgroup=relative,
                  gpu_pci=args.gpu_pci, initial_memory_events=(cgroup / 'memory.events').read_text(),
                  memory_max=int(hard), memory_swap_max=0, gpu_sysfs=str(gpu),
                  vram_total=total, vram_reserve=reserve, host_reserve=host_reserve,
                  start_vram=start_vram, start_gtt=start_gtt, vram_increment_budget=vram_budget, gtt_increment_budget=gtt_budget, samples=[], stop_reason=None)
    interrupted = []
    seen_processes = {}
    def members():
        """Verify exact membership and birth tokens before returning owned child identities."""
        result = []
        for pid in map(int, (cgroup / 'cgroup.procs').read_text().split()):
            if pid == os.getpid():
                continue
            try:
                actual = Path(f'/proc/{pid}/cgroup').read_text().strip().split('0::', 1)[1]
                if actual != relative:
                    raise RuntimeError('cgroup member identity changed')
                result.append(dict(pid=pid, start_ticks=identity(pid),
                                   exe=str(Path(f'/proc/{pid}/exe').resolve())))
            except (FileNotFoundError, ProcessLookupError):
                pass
        return result

    def stop_owned():
        """Signal only verified exact-cgroup members and wait for complete owned quiescence."""
        for signum in [signal.SIGINT, signal.SIGTERM, signal.SIGKILL]:
            owned = members()
            report['owned_at_stop'] = owned
            if not owned:
                return
            for item in owned:
                try:
                    pid = item['pid']
                    if identity(pid) != item['start_ticks']:
                        raise RuntimeError('PID reused; refusing signal')
                    if Path(f'/proc/{pid}/cgroup').read_text().strip().split('0::', 1)[1] != relative:
                        raise RuntimeError('process left owned cgroup; refusing signal')
                    os.kill(pid, signum)
                except (FileNotFoundError, ProcessLookupError):
                    pass
            deadline = time.monotonic() + 2
            while time.monotonic() < deadline:
                if process is not None:
                    process.poll()
                if not members():
                    return
                time.sleep(0.02)
        if members():
            raise RuntimeError('owned cgroup failed to quiesce')

    try:
        signal.signal(signal.SIGTERM, lambda number, frame: interrupted.append(number))
        signal.signal(signal.SIGINT, lambda number, frame: interrupted.append(number))
        process = subprocess.Popen(command, start_new_session=True)
        report.update(pid=process.pid, start_ticks=identity(process.pid))
        while process.poll() is None:
            current = int((cgroup / 'memory.current').read_text())
            vram = int((gpu / 'mem_info_vram_used').read_text())
            gtt = int((gpu / 'mem_info_gtt_used').read_text())
            rss = 0
            user_cpu = 0.0
            system_cpu = 0.0
            for item in members():
                seen_processes[str(item['pid']) + ':' + item['start_ticks']] = item
                try:
                    status = Path(f"/proc/{item['pid']}/status").read_text().splitlines()
                    stat = Path(f"/proc/{item['pid']}/stat").read_text().rsplit(')', 1)[1].split()
                    user_cpu += int(stat[11]) / os.sysconf('SC_CLK_TCK')
                    system_cpu += int(stat[12]) / os.sysconf('SC_CLK_TCK')
                    rss += sum(int(line.split()[1]) * 1024 for line in status if line.startswith('VmRSS:'))
                except FileNotFoundError:
                    pass
            elapsed = time.monotonic() - started
            available = memory_available()
            phase = 'unlabeled'
            if args.phase_file:
                try:
                    phase = Path(args.phase_file).read_text().strip()
                except FileNotFoundError:
                    phase = 'startup'
            busy_path = gpu / 'gpu_busy_percent'
            gpu_busy = int(busy_path.read_text()) if busy_path.exists() else None
            report['samples'].append(dict(seconds=elapsed, phase=phase, user_cpu_seconds=user_cpu, system_cpu_seconds=system_cpu, gpu_busy_percent=gpu_busy, cgroup=current, rss=rss,
                                          host_available=available, vram=vram, gtt=gtt))
            reason = ('external-stop' if interrupted else
                      'host-reserve' if available < host_reserve else
                      'cgroup-soft' if current > args.soft_mib * 2**20 else
                      'vram-reserve' if total - vram < reserve else
                      'vram-planned-budget' if vram > start_vram + vram_budget else
                      'gtt-planned-budget' if gtt > start_gtt + gtt_budget else
                      'watchdog' if elapsed > args.seconds else None)
            if reason:
                report['stop_reason'] = reason
                stop_owned()
                break
            time.sleep(0.02)
        process.wait(timeout=2)
        if members():
            report['stop_reason'] = report['stop_reason'] or 'orphaned-owned-descendants'
            stop_owned()
    except BaseException as error:
        report['exception'] = repr(error)
        report['stop_reason'] = report['stop_reason'] or 'supervisor-exception'
        stop_owned()
        raise
    finally:
        report.update(returncode=process.poll() if process is not None else None, seconds=time.monotonic() - started,
                      memory_peak=int((cgroup / 'memory.peak').read_text()),
                      memory_events=(cgroup / 'memory.events').read_text(),
                      swap_current=int((cgroup / 'memory.swap.current').read_text()),
                      final_owned_members=members(), owned_process_identities_seen=list(seen_processes.values()))
        Path(args.report).write_text(json.dumps(report, indent=2) + '\n')
    return 124 if report['stop_reason'] else report['returncode']


if __name__ == '__main__':
    raise SystemExit(run())
