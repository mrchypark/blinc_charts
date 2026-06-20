#!/usr/bin/env python3
"""Run the example-only mini-model eval for the desktop chart gallery."""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import textwrap
import time
import urllib.error
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DESKTOP_MANIFEST = ROOT / "examples/blinc_charts_desktop/Cargo.toml"
DEFAULT_MODEL = "gpt-5.4-mini"
INVALID_FUNNEL_STYLE_FIELDS = ("scroll_zoom_factor", "pinch_zoom_min")
FUNNEL_STATIC_VARIANT_CODE = "FunnelChartModel::new(stages)?"
FUNNEL_BUDGET_VARIANT_CODE = "stages.truncate(4)"
FUNNEL_STATIC_EVIDENCE = """
let stages = vec![("Visitors".into(), 12000.0), ("Paid".into(), 1480.0)];
let model = FunnelChartModel::new(stages)?;
funnel_chart(FunnelChartHandle::new(model))
Funnel is static/model-driven: FunnelChartStyle has no scroll_zoom_factor or pinch_zoom_min.
""".strip()
MINIMAL_SETUP = """
Minimal setup examples:
let x: Vec<f32> = (0..160).map(|i| i as f32).collect();
let y: Vec<f32> = x.iter().map(|v| (v * 0.12).sin()).collect();
let series = TimeSeriesF32::new(x, y)?;
let mut model = LineChartModel::new(series);
model.style.scroll_zoom_factor = 0.02;
let mut bindings = ChartInputBindings::default();
bindings.scroll_zoom = true;
let chart = line_chart_with_bindings(LineChartHandle::new(model), bindings);
Ok(div().w(640.0).h(320.0).child(chart))

let x2: Vec<f32> = (0..160).map(|i| i as f32).collect();
let a = TimeSeriesF32::new(x2.clone(), x2.iter().map(|v| v.sin()).collect())?;
let b = TimeSeriesF32::new(x2, (0..160).map(|i| (i as f32).cos()).collect())?;
let mut stacked = StackedAreaChartModel::new(vec![a, b])?;
stacked.style.mode = StackedAreaMode::Stacked;

Notes:
- LineChartModel, AreaChartModel, and ScatterChartModel take TimeSeriesF32, not Point vectors.
- ScatterChartModel::new(series) returns a model directly; do not add ?.
- BarChartModel, MultiLineChartModel, and StackedAreaChartModel take Vec<TimeSeriesF32>.
- For StackedArea, call StackedAreaChartModel::new(vec![series_a, series_b]) even for simple examples.
- Constructors ending in ? in the examples return Result; keep the ? before mutating model.style.
- Builders return impl ElementBuilder; wrap them in div().child(builder) when returning Div.
- Use ChartInputBindings::default(); there are no per-family binding types.
- For line budget caps, use model.set_downsample_max_points(128), not model.style.max_points.
- For scatter budget caps, use model.set_max_points(128), not set_downsample_max_points.
- For histogram budget caps, use model.style.bins = 48; HistogramChartModel has no set_max_points.
- StatisticsChartModel::new takes Vec<Vec<f32>> grouped samples, not TimeSeriesF32.
- PolarChartModel::new_radar takes dimensions plus Vec<Vec<f32>> series rows, not a single Vec<f32>.
- NetworkChartModel::new_graph/new_sankey/new_chord all return Result; keep the ? before mutating model.style.
- DensityMap uses density_map_chart(DensityMapChartHandle::new(model)); there is no density_map_chart_with_bindings.
- Funnel is static/model-driven: FunnelChartStyle has no scroll_zoom_factor or pinch_zoom_min.
- For candlestick data, build Candle { x, open, high, low, close } values and call CandleSeries::new(candles)?.
- For candlestick budget caps, use model.style.max_candles, not model.style.max_points.
- For MultiLine detail caps, use model.style.max_points_per_series, not model.style.max_segments.
- For linked charts, create links with chart_link(0.0, 159.0) and pass bindings to linked_*_chart_with_bindings(...).
- Do not call input_bindings on a chart builder.
""".strip()


def run(cmd: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=ROOT, text=True, check=False, **kwargs)


def cargo_env() -> dict[str, str]:
    env = os.environ.copy()
    env.setdefault("CARGO_TARGET_DIR", str(ROOT / "target"))
    return env


def load_cases() -> list[dict]:
    cache = ROOT / "target/blinc_charts_mini_eval/coverage_matrix.json"
    if cache.exists():
        try:
            return normalize_cases(json.loads(cache.read_text())["cases"])
        except json.JSONDecodeError:
            cache.unlink()
    prompts = load_cases_from_prompts()
    if len(prompts) == 117:
        return normalize_cases(prompts)
    try:
        proc = run(
            [
                "cargo",
                "run",
                "--quiet",
                "--manifest-path",
                str(DESKTOP_MANIFEST),
                "--bin",
                "export_coverage_matrix",
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
        )
    except subprocess.TimeoutExpired:
        return normalize_cases(prompts)
    if proc.returncode != 0:
        if prompts:
            return normalize_cases(prompts)
        raise SystemExit(proc.stderr)
    cache.parent.mkdir(parents=True, exist_ok=True)
    cache.write_text(proc.stdout)
    return normalize_cases(json.loads(proc.stdout)["cases"])


def load_cases_from_prompts() -> list[dict]:
    prompts = sorted((ROOT / "target/blinc_charts_mini_eval/prompts").glob("case_*.md"))
    cases = []
    for path in prompts:
        text = path.read_text()
        index = int(path.stem.split("_")[1])
        family = find_line(text, "- chart family: ")
        variant = find_line(text, "- variant: ")
        interaction = find_line(text, "- interaction: ")
        variant_code = find_block(text, "Required variant code or equivalent:", "Required interaction code or equivalent:")
        interaction_code = find_block(text, "Required interaction code or equivalent:", "Example-only evidence:")
        evidence = text.split("Example-only evidence:", 1)[1].strip()
        cases.append(
            {
                "index": index,
                "family": family,
                "variant": variant,
                "variant_code": variant_code,
                "variant_effect": "",
                "interaction": interaction,
                "interaction_code": interaction_code,
                "interaction_effect": "",
                "task": (
                    "Using only the provided blinc_charts examples, write a Rust function "
                    f"that builds chart={family} variant={variant} interaction={interaction} "
                    "and returns a Blinc element."
                ),
                "evidence": evidence,
            }
        )
    return cases


def find_line(text: str, prefix: str) -> str:
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith(prefix):
            return stripped[len(prefix) :].strip()
    return ""


def find_block(text: str, start: str, end: str) -> str:
    return text.split(start, 1)[1].split(end, 1)[0].strip()


def normalize_cases(cases: list[dict]) -> list[dict]:
    return [normalize_case(case) for case in cases]


def normalize_case(case: dict) -> dict:
    if case.get("family") != "Funnel":
        return case

    text = "\n".join(str(case.get(key, "")) for key in ("variant_code", "evidence"))
    if "set_*_max_points" in text or "style.max_*" in text:
        normalized = dict(case)
        normalized.update(
            {
                "variant": "Budget cap",
                "variant_code": FUNNEL_BUDGET_VARIANT_CODE,
                "variant_effect": "Keeps the rendered funnel compact by limiting stage count.",
                "evidence": "\n".join([FUNNEL_STATIC_EVIDENCE, FUNNEL_BUDGET_VARIANT_CODE]),
            }
        )
        return normalized

    if not any(field in text for field in INVALID_FUNNEL_STYLE_FIELDS):
        return case

    normalized = dict(case)
    normalized.update(
        {
            "variant": "Stage values",
            "variant_code": FUNNEL_STATIC_VARIANT_CODE,
            "variant_effect": "Uses FunnelChartModel stage values; Funnel has no scroll or pinch style fields.",
            "evidence": FUNNEL_STATIC_EVIDENCE,
        }
    )
    return normalized


def select_cases(cases: list[dict], args: argparse.Namespace) -> list[dict]:
    if args.case:
        wanted = {int(value) for value in args.case}
        cases = [case for case in cases if case["index"] in wanted]
    if args.limit is not None:
        cases = cases[: args.limit]
    return cases


def prompt_for(case: dict) -> str:
    return textwrap.dedent(
        f"""
        You are generating a small Rust example for blinc_charts.

        Rules:
        - Use only the provided examples and notes below.
        - Do not reference src/*.rs, private implementation details, or test answers.
        - Return only Rust code, no Markdown.
        - The code must define exactly this function:
          pub fn build_chart() -> anyhow::Result<blinc_layout::div::Div>
        - The compile harness already imports:
          use blinc_charts::prelude::*;
          use blinc_core::{{Color, Point}};
          use blinc_layout::prelude::*;

        {MINIMAL_SETUP}

        Chart spec:
        - chart family: {case["family"]}
        - variant: {case["variant"]}
        - interaction: {case["interaction"]}
        - output: blinc_layout::div::Div

        Required variant code or equivalent:
        {case["variant_code"]}

        Required interaction code or equivalent:
        {case["interaction_code"]}

        Example-only evidence:
        {case["evidence"]}
        """
    ).strip()


def call_openai(prompt: str, model: str, api_key: str) -> str:
    payload = {
        "model": model,
        "input": prompt,
        "reasoning": {"effort": "low"},
        "max_output_tokens": 2200,
    }
    req = urllib.request.Request(
        "https://api.openai.com/v1/responses",
        data=json.dumps(payload).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=90) as resp:
            data = json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"OpenAI API HTTP {exc.code}: {body}") from exc

    if data.get("output_text"):
        return data["output_text"]
    chunks: list[str] = []
    for item in data.get("output", []):
        for content in item.get("content", []):
            if "text" in content:
                chunks.append(content["text"])
    return "\n".join(chunks).strip()


def call_codex_exec(prompt: str, model: str, out: Path, index: int, timeout: int) -> str:
    workspace = out / "codex_workspace"
    messages = out / "codex_messages"
    workspace.mkdir(parents=True, exist_ok=True)
    messages.mkdir(parents=True, exist_ok=True)
    output = messages / f"case_{index:03d}.txt"
    cmd = [
        "codex",
        "exec",
        "--ephemeral",
        "--ignore-user-config",
        "--skip-git-repo-check",
        "--sandbox",
        "read-only",
        "--cd",
        str(workspace),
        "--output-last-message",
        str(output),
    ]
    if model:
        cmd.extend(["--model", model])
    cmd.append(prompt)
    proc = subprocess.run(
        cmd,
        cwd=workspace,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
        timeout=timeout,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stdout[-4000:])
    return output.read_text()


def extract_code(text: str) -> str:
    match = re.search(r"```(?:rust)?\s*(.*?)```", text, re.S)
    code = match.group(1) if match else text
    return code.strip()


def has_required_snippet(code: str, snippet: str) -> bool:
    if not snippet or snippet == "build_chart(family)":
        return True
    if ".." in snippet or "/" in snippet or "*" in snippet:
        return True
    haystack = re.sub(r"\s+", "", code)
    checks: list[str] = []
    for part in snippet.split(";"):
        part = part.strip()
        if not part:
            continue
        calls = re.findall(r"([A-Za-z_][A-Za-z0-9_]*)\s*\(", part)
        if "=" in part:
            checks.append(re.sub(r"\s+", "", part))
        elif calls:
            checks.extend(calls)
        else:
            checks.append(re.sub(r"\s+", "", part))
    return all(check in haystack for check in checks)


def write_eval_crate(out: Path, cases: list[dict], generated: dict[int, str]) -> Path:
    crate = out / "crate"
    if (crate / "src").exists():
        shutil.rmtree(crate / "src")
    (crate / "src/cases").mkdir(parents=True)

    (crate / "Cargo.toml").write_text(
        textwrap.dedent(
            f"""
            [package]
            name = "blinc_charts_mini_eval"
            version = "0.0.0"
            edition = "2021"
            publish = false

            [dependencies]
            anyhow = "1.0"
            blinc_charts = {{ path = {json.dumps(str(ROOT))} }}
            blinc_core = {{ git = "https://github.com/mrchypark/Blinc.git", rev = "b9b0c2b01b15cfeaed1de5820e21c76823402fda", package = "blinc_core" }}
            blinc_layout = {{ git = "https://github.com/mrchypark/Blinc.git", rev = "b9b0c2b01b15cfeaed1de5820e21c76823402fda", package = "blinc_layout" }}
            """
        ).strip()
        + "\n"
    )

    lib = [
        "#![allow(unused_imports)]",
        "use blinc_charts::prelude::*;",
        "use blinc_core::{Color, Point};",
        "use blinc_layout::prelude::*;",
        "",
    ]
    for case in cases:
        idx = case["index"]
        code = generated.get(idx)
        if code is None:
            continue
        name = f"case_{idx:03d}"
        (crate / f"src/cases/{name}.rs").write_text(code + "\n")
        lib.extend(
            [
                f"mod {name} {{",
                "    use super::*;",
                f'    include!("cases/{name}.rs");',
                "}",
                "#[test]",
                f"fn {name}_builds() {{",
                f"    let _ui: blinc_layout::div::Div = {name}::build_chart().unwrap();",
                "}",
                "",
            ]
        )
    (crate / "src/lib.rs").write_text("\n".join(lib))
    return crate


def compile_crate(crate: Path) -> subprocess.CompletedProcess:
    log = crate.parent / "cargo_check.log"
    if not (crate / "Cargo.lock").exists():
        with log.open("w") as f:
            lock = subprocess.run(
                ["cargo", "generate-lockfile", "--manifest-path", str(crate / "Cargo.toml")],
                cwd=ROOT,
                env=cargo_env(),
                text=True,
                stdout=f,
                stderr=subprocess.STDOUT,
                check=False,
            )
        if lock.returncode != 0:
            lock.stdout = log.read_text()
            return lock
    with log.open("w") as f:
        proc = subprocess.run(
            ["cargo", "check", "--tests", "--locked", "--manifest-path", str(crate / "Cargo.toml")],
            cwd=ROOT,
            env=cargo_env(),
            text=True,
            stdout=f,
            stderr=subprocess.STDOUT,
            check=False,
        )
    proc.stdout = log.read_text()
    return proc


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", default=os.environ.get("OPENAI_MODEL", DEFAULT_MODEL))
    parser.add_argument("--backend", choices=["openai", "codex-exec"], default="openai")
    parser.add_argument("--codex-timeout", type=int, default=360)
    parser.add_argument("--out", type=Path, default=ROOT / "target/blinc_charts_mini_eval")
    parser.add_argument("--limit", type=int)
    parser.add_argument("--case", action="append", help="Coverage case index to run")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--reuse", action="store_true", help="Compile existing generated cases")
    args = parser.parse_args()

    cases = select_cases(load_cases(), args)
    args.out.mkdir(parents=True, exist_ok=True)
    prompts_dir = args.out / "prompts"
    generated_dir = args.out / "generated"
    prompts_dir.mkdir(exist_ok=True)
    generated_dir.mkdir(exist_ok=True)

    api_key = os.environ.get("OPENAI_API_KEY", "")
    generated: dict[int, str] = {}
    results = []

    for case in cases:
        idx = case["index"]
        prompt = prompt_for(case)
        (prompts_dir / f"case_{idx:03d}.md").write_text(prompt + "\n")
        code_path = generated_dir / f"case_{idx:03d}.rs"

        if args.reuse and code_path.exists():
            code = code_path.read_text()
        elif args.dry_run or (args.backend == "openai" and not api_key):
            reason = "dry_run" if args.dry_run else "no_api_key"
            results.append({**case, "status": "prompted", "reason": reason})
            continue
        else:
            started = time.time()
            if args.backend == "codex-exec":
                code = extract_code(call_codex_exec(prompt, args.model, args.out, idx, args.codex_timeout))
            else:
                code = extract_code(call_openai(prompt, args.model, api_key))
            code_path.write_text(code + "\n")
            results.append({**case, "status": "generated", "seconds": round(time.time() - started, 2)})

        generated[idx] = code

    compile_status = "not_run"
    proc_output = ""
    if generated:
        crate = write_eval_crate(args.out, cases, generated)
        proc = compile_crate(crate)
        compile_status = "passed" if proc.returncode == 0 else "failed"
        proc_output = proc.stdout
        for case in cases:
            code = generated.get(case["index"])
            if code is None:
                continue
            results.append(
                {
                    **case,
                    "status": "checked",
                    "variant_snippet_present": has_required_snippet(code, case["variant_code"]),
                    "interaction_snippet_present": has_required_snippet(code, case["interaction_code"]),
                }
            )

    summary = {
        "model": args.model,
        "backend": args.backend,
        "case_count": len(cases),
        "generated_count": len(generated),
        "compile_status": compile_status,
        "output_dir": str(args.out),
    }
    (args.out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    with (args.out / "results.jsonl").open("w") as f:
        for row in results:
            f.write(json.dumps(row, ensure_ascii=False) + "\n")
    if proc_output:
        (args.out / "cargo_check.log").write_text(proc_output)

    print(json.dumps(summary, indent=2))
    if compile_status == "failed":
        print(proc_output[-4000:], file=sys.stderr)
        return 1
    if not generated and not args.dry_run and args.backend == "openai":
        print("OPENAI_API_KEY is not set; prompts were written but no model calls ran.", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
