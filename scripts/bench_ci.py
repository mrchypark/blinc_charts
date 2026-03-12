#!/usr/bin/env python3

import argparse
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path


def load_json(path: Path):
    with path.open() as f:
        return json.load(f)


def parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--baseline", required=False)
    parser.add_argument("--sample-size", type=int, default=10)
    parser.add_argument("--warm-up-time", type=int, default=1)
    parser.add_argument("--measurement-time", type=int, default=2)
    return parser.parse_args()


VALID_NAME = re.compile(r"^[A-Za-z0-9_-]+$")


def sanitize_name(raw: str, field: str) -> str:
    if not VALID_NAME.fullmatch(raw):
        raise ValueError(f"invalid {field}: {raw!r}")
    return raw


def run_benchmark(item, sample_size: int, warm_up_time: int, measurement_time: int):
    bench_name = sanitize_name(item["bench"], "bench")
    bench_id = sanitize_name(item["id"], "benchmark id")

    target_dir = Path("target/criterion") / bench_id
    if target_dir.exists():
        shutil.rmtree(target_dir)

    cmd = [
        "cargo",
        "bench",
        "--bench",
        bench_name,
        bench_id,
        "--",
        "--noplot",
        "--sample-size",
        str(sample_size),
        "--warm-up-time",
        str(warm_up_time),
        "--measurement-time",
        str(measurement_time),
    ]
    subprocess.run(cmd, check=True)

    estimates_path = Path("target/criterion") / bench_id / "new" / "estimates.json"
    if not estimates_path.exists():
        raise FileNotFoundError(f"missing Criterion estimates for {bench_id}: {estimates_path}")
    estimates = load_json(estimates_path)
    return float(estimates["median"]["point_estimate"])


def regression_min_delta_ns(baseline_ns: float) -> float:
    # Ignore sub-baseline jitter, but still fail millisecond-scale regressions.
    return min(baseline_ns * 0.75, 1_000_000.0)


def compare_to_baseline(item, current_ns: float, baseline_lookup):
    baseline = baseline_lookup.get(item["id"])
    if baseline is None:
        return None
    delta_pct = ((current_ns - baseline) / baseline) * 100.0
    delta_ns = current_ns - baseline
    min_delta_ns = regression_min_delta_ns(baseline)
    return {
        "baseline_ns": baseline,
        "delta_ns": delta_ns,
        "delta_pct": delta_pct,
        "regression_min_delta_ns": min_delta_ns,
        "regression_fail_pct": item["regression_fail_pct"],
        "regression_failed": delta_pct > item["regression_fail_pct"] and delta_ns > min_delta_ns,
    }


def format_ns(ns: float) -> str:
    if ns >= 1_000_000:
        return f"{ns / 1_000_000:.3f} ms"
    if ns >= 1_000:
        return f"{ns / 1_000:.3f} us"
    return f"{ns:.1f} ns"


def main():
    args = parse_args()
    config = load_json(Path(args.config))
    baseline_lookup = {}
    if args.baseline:
        baseline_path = Path(args.baseline)
        if baseline_path.exists():
            baseline_data = load_json(baseline_path)
            baseline_lookup = {
                item["id"]: float(item["median_ns"]) for item in baseline_data.get("results", [])
            }

    results = []
    failures = []

    for item in config["benchmarks"]:
        current_ns = run_benchmark(
            item,
            sample_size=args.sample_size,
            warm_up_time=args.warm_up_time,
            measurement_time=args.measurement_time,
        )
        absolute_failed = current_ns > float(item["absolute_fail_ns"])
        comparison = compare_to_baseline(item, current_ns, baseline_lookup)

        result = {
            "bench": item["bench"],
            "id": item["id"],
            "median_ns": current_ns,
            "absolute_fail_ns": float(item["absolute_fail_ns"]),
            "absolute_failed": absolute_failed,
            "baseline": comparison,
        }
        results.append(result)

        if absolute_failed:
            failures.append(
                f"{item['id']}: absolute budget exceeded "
                f"({format_ns(current_ns)} > {format_ns(float(item['absolute_fail_ns']))})"
            )
        if comparison and comparison["regression_failed"]:
            failures.append(
                f"{item['id']}: regression exceeded "
                f"({comparison['delta_pct']:.1f}% > {comparison['regression_fail_pct']:.1f}%)"
            )

    output = {
        "results": results,
        "failed": bool(failures),
        "failures": failures,
    }
    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(output, indent=2))

    print("Benchmark summary:")
    for result in results:
        line = (
            f"- {result['id']}: {format_ns(result['median_ns'])} "
            f"(absolute fail at {format_ns(result['absolute_fail_ns'])})"
        )
        baseline = result["baseline"]
        if baseline:
            line += f", baseline {format_ns(baseline['baseline_ns'])}, delta {baseline['delta_pct']:.1f}%"
        print(line)

    if failures:
        print("\nBenchmark gate failures:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
