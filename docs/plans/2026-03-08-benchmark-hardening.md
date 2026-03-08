# Benchmark Hardening Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make line-rendering benchmarks easier to interpret and better at isolating regressions before changing more rendering code.

**Architecture:** Keep the existing end-to-end Criterion harness, but add a named-baseline workflow, increase the macro benchmark workload so it is less noisy, and add microbench coverage for density fallback and LOD stitching/query hot paths. Use deterministic guidance in `README.md` so developers compare against intentional baselines instead of whatever stale Criterion `base` directory happens to exist.

**Tech Stack:** Rust 2021, Criterion, `RecordingContext`, `blinc_charts`

---

### Task 1: Document intentional baseline workflow

**Files:**
- Modify: `README.md`

**Steps:**
1. Document a named-baseline capture command for the current branch head.
2. Document a named-baseline comparison command for later runs.
3. Clarify that absolute budgets are the acceptance gate and percentage change is secondary context.

### Task 2: Stabilize existing macro benchmarks

**Files:**
- Modify: `benches/line_render.rs`

**Steps:**
1. Increase per-iteration work in the hover and pan benches so they are less sensitive to scheduling noise.
2. Keep the benchmark names aligned with the new workloads.
3. Preserve the current steady-state warm-cache intent.

### Task 3: Add microbench coverage for key hot paths

**Files:**
- Modify: `benches/line_render.rs`

**Steps:**
1. Add a density-overview benchmark that forces the fallback renderer path.
2. Add LOD microbenches for `query_into()` and `stitch_visible_edges()`.
3. Add a narrow-window line benchmark for the raw-visible fallback path.

### Task 4: Verify and review

**Files:**
- Verify only

**Steps:**
1. Run `cargo test --lib`.
2. Run `cargo bench --bench line_render -- --noplot`.
3. Request review on the diff and address any findings before completion.
