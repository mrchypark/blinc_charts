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


def run(cmd: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, cwd=ROOT, text=True, check=False, **kwargs)


def cargo_env() -> dict[str, str]:
    env = os.environ.copy()
    env.setdefault("CARGO_TARGET_DIR", str(ROOT / "target"))
    return env


def load_cases() -> list[dict]:
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
    )
    if proc.returncode != 0:
        raise SystemExit(proc.stderr)
    return json.loads(proc.stdout)["cases"]


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
        elif args.dry_run or not api_key:
            results.append({**case, "status": "prompted", "reason": "dry_run_or_no_api_key"})
            continue
        else:
            started = time.time()
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
    if not generated and not args.dry_run:
        print("OPENAI_API_KEY is not set; prompts were written but no model calls ran.", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
