#!/usr/bin/env python3
"""Compile-time benchmark harness.

usage: bench/run.py <label> <rustup-toolchain> [--rustflags=...] [--reps=N]

Times clean `cargo check`, clean debug build, clean release build and an incremental
debug rebuild (after touching the main source file) for ripgrep and kitchen-sink.
Writes results/<label>.csv with the median and all samples per scenario.
"""
import argparse, os, shutil, statistics, subprocess, sys, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CRATES = {
    "ripgrep": (ROOT / "bench/work/ripgrep", "crates/core/main.rs"),
    "kitchen-sink": (ROOT / "bench/kitchen-sink", "src/main.rs"),
}
SCENARIOS = ["check", "debug", "release", "incr-debug"]

def cargo(tc, args, cwd, env):
    t0 = time.perf_counter()
    r = subprocess.run(["cargo", f"+{tc}", *args], cwd=cwd, env=env,
                       stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    dt = time.perf_counter() - t0
    if r.returncode:
        sys.exit(f"FAILED cargo {' '.join(args)} in {cwd}:\n{r.stderr[-3000:]}")
    return dt

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("label"); ap.add_argument("toolchain")
    ap.add_argument("--rustflags", default="")
    ap.add_argument("--reps", type=int, default=3)
    ap.add_argument("--crates", default=",".join(CRATES))
    ap.add_argument("--scenarios", default=",".join(SCENARIOS))
    a = ap.parse_args()
    tgt = ROOT / f"bench/target-{a.label}"
    base_env = dict(os.environ, RUSTFLAGS=a.rustflags, CARGO_INCREMENTAL="0")
    (ROOT / "results").mkdir(exist_ok=True)
    out = open(ROOT / f"results/{a.label}.csv", "w")
    out.write("crate,scenario,median_s,samples\n")
    for crate in a.crates.split(","):
        cwd, touch = CRATES[crate]
        subprocess.run(["cargo", f"+{a.toolchain}", "fetch", "-q"], cwd=cwd, check=True)
        for scen in a.scenarios.split(","):
            samples = []
            for _ in range(a.reps):
                td = ["--target-dir", str(tgt)]
                if scen == "incr-debug":
                    env = dict(base_env, CARGO_INCREMENTAL="1")
                    if not (tgt / "debug").exists():
                        cargo(a.toolchain, ["build", *td], cwd, env)
                    (cwd / touch).touch()
                    samples.append(cargo(a.toolchain, ["build", *td], cwd, env))
                else:
                    shutil.rmtree(tgt, ignore_errors=True)
                    args = {"check": ["check"], "debug": ["build"], "release": ["build", "--release"]}[scen]
                    samples.append(cargo(a.toolchain, [*args, *td], cwd, base_env))
            m = statistics.median(samples)
            line = f"{crate},{scen},{m:.3f},{' '.join(f'{s:.3f}' for s in samples)}"
            print(line, flush=True); out.write(line + "\n"); out.flush()
        shutil.rmtree(tgt, ignore_errors=True)

if __name__ == "__main__":
    main()
