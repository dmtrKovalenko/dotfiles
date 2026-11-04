# Performance evidence report

Scale this report to the task. Include the evidence needed to reproduce the conclusion; do not expand a tiny optimization into an unrelated audit. Use real data or an explicit measurement gap in every populated result row.

## Outcome and code change

State the verdict first: measured improvement, tradeoff, inconclusive, unvalidated, or regression. Identify the measured workload and target machine.

State whether measurements were executed and inspected, or whether this is an assessment of supplied results. Link the raw records actually inspected. Qualify conclusions based only on summaries; do not imply independent verification of unavailable artifacts.

Explain the original cost, the hypothesis, the concrete before/after implementation, and why the data supports or contradicts it. Link the changed code or a focused diff. State what the benchmark does and validates.

## Comparison

Use the relevant rows for each important workload. For the full workflow, include latency, CPU/cycles, and memory evidence; mark missing collection explicitly. Do not fill missing data with illustrative values.

| Metric and workload | Baseline | Candidate | Change | Samples / uncertainty / scope |
|---|---|---|---|---|
| Operation latency | | | | |
| Request p95/p99, if applicable | | | | |
| Completed useful work / second | | | | |
| CPU time / operation | | | | |
| Hardware cycles / operation | | | | |
| Instructions / operation | | | | |
| Allocations and allocated bytes / operation | | | | |
| Peak resident memory | | | | |
| Retained memory after representative use | | | | |

Give units, direction, absolute values, sample counts, and uncertainty method. List important regressions and budget outcomes individually. Keep claimed scope consistent with the data: one host or fixture does not establish universal improvement.

## Assembly and profile evidence

Name the actual hot symbol or call site, build identity, and architecture. Link before/after profiles and focused assembly excerpts. Describe the observed instruction-level change and its relationship to the counters and timing. If inspection was blocked or not meaningful for the workload, state why and what was checked instead.

## Correctness and reproduction

Record the correctness oracle, meaningful tests and results, and coverage of new edge cases. Provide exact build, benchmark, profiling, and analysis commands plus the input identity. Link the environment manifest, raw samples, collector outputs, and relevant source states.

## Limits and disposition

State environmental noise, missing counters, untested targets, profiler overhead, and remaining uncertainty when they affect the conclusion. Identify whether the change was retained or removed and any user decision needed for a tradeoff. State where large artifacts are stored. Never claim an optimization is proven merely because its tests pass or its assembly looks shorter.
