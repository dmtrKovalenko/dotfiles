---
name: performance-optimization
description: Measure and improve software performance with correct benchmarks, CPU and memory profiling, latency analysis, assembly inspection, and reproducible before-and-after comparisons. Use to investigate slow code, reduce allocations or resource use, resolve performance regressions, or verify an optimization claim.
---

# Performance optimization

Deliver a behavior-preserving change with evidence that it improves the intended workload. A plausible mechanism, attractive assembly, or one fast run is not a measured improvement.

## Establish the experiment

Inspect the relevant implementation, tests, benchmarks, build settings, and deployment target. Identify:

- The operation being improved and when its result becomes usable.
- Representative inputs, sizes, distributions, concurrency, and cache state.
- The primary objective: latency, throughput, CPU work, allocations, or memory footprint. Also name the metrics that must not regress.
- Required behavior: outputs, errors, ordering, numerical precision, side effects, and supported hardware.
- An existing performance budget or a stated practical improvement target. Do not invent a universal percentage gate.

Use repository context to resolve these details. Ask only when a missing choice changes the result materially. A request to benchmark or analyze stays within that scope; a request to optimize includes implementing and validating the change.

Read [benchmark-validity.md](references/benchmark-validity.md) before constructing or trusting the comparison. Consult [tools.md](references/tools.md) for the current platform when collecting profiles or inspecting assembly. Use [report.md](references/report.md) for the final evidence. [sources.md](references/sources.md) records the references and why they were selected.

## 1. Prove the benchmark measures the right work

Reuse the project's harness when it is suitable. Add a focused benchmark only when existing coverage cannot reproduce the relevant cost.

- Establish expected outputs using tests, a known-correct implementation, or an independent oracle. Verify the benchmark's output and successful completion for both versions.
- Consume results and use the harness's optimization barriers where needed. Check that constant folding, dead-code elimination, empty work, early errors, or stale results cannot explain a fast run.
- Specify whether setup, parsing, allocation, cleanup, startup, I/O, and synchronization belong inside the timed operation. Keep a representative application-level case that includes the costs its caller actually pays.
- Await asynchronous completion. Record completed useful work, failures, and dropped requests when measuring throughput.
- Exercise meaningful size and distribution buckets, including any important worst case. A single repeated input may accidentally measure a cache hit or a predictable branch.

If the benchmark is wrong, fix it first and collect a fresh baseline. Never compare an old harness against a corrected one and attribute the difference to the implementation.

## 2. Capture a reproducible baseline

Before performance edits, preserve the baseline source state and runnable artifact without overwriting the user's work. Record the commit plus any dirty patch, executable identity, exact build and run commands, compiler/runtime versions, flags, target CPU, dependencies, input identity, and machine conditions.

Use the intended optimized build, with symbols available for profiling. Keep optimization, target features, allocator, and runtime settings equivalent between versions unless one is the explicit experimental variable. Never compare debug against release and call it a code improvement.

Save raw samples and a baseline correctness result. Separate warmup from measurement and cold-start cases from steady-state cases. Run a short pilot to estimate noise and choose a sensible measurement duration and repetition count.

## 3. Measure and profile the bottleneck

For a full optimization task, cover every row below or record the specific reason a metric could not be collected. Narrow the collection only when the user's task explicitly calls for a narrower investigation.

| Dimension | Evidence to collect | Interpretation constraint |
|---|---|---|
| Latency / throughput | Operation timing; distributions and load for requests; completed work per second | Batch averages are not request percentiles; preserve error and timeout counts |
| CPU work | CPU time, sampled stacks, hardware cycles and instructions when supported | CPU time, elapsed time, and cycles are different quantities |
| Memory | Allocation count and bytes per operation, peak resident memory, live/retained memory when relevant | Fewer allocations can coexist with a larger retained heap or RSS |
| Limiting mechanism | Relevant branches, cache behavior, contention, I/O waits, scheduling, or dependency stalls | A hot symbol or one counter is a lead to investigate, not a diagnosis |

Profile baseline and candidate under the same workload. Collect expensive allocation traces, CPU samples, and final timing in separate runs so profiler overhead cannot masquerade as a regression or improvement. Preserve the collector configuration and name the measurement scope, including child processes or service workers where applicable.

On a VM or restricted host, hardware counters may be unavailable. Report that gap and continue with valid measurements; never derive supposed hardware cycles from elapsed time multiplied by advertised clock frequency. Do not silently weaken host security or change shared system settings to get counters.

## 4. Inspect the generated assembly

Use the profile to choose the relevant function, loop, or call site. Inspect both versions' optimized machine code for the actual target architecture, keeping the build settings tied to the benchmark artifacts.

Look for the mechanism the change predicts: removed calls or allocations, eliminated checks, different loads/stores, vectorization, dependency chains, branches, spills, synchronization, or code-size growth. Follow inlined code into its caller. If symbols are absent, try available symbol files or address-based disassembly before declaring inspection blocked.

Save focused before/after excerpts with symbol, binary identity, and commands. For interpreted or JIT workloads, examine the relevant generated code or native extension only when it can be tied to the measured path; state when assembly evidence is unavailable or not meaningful. Do not substitute unrelated interpreter disassembly.

Explain what changed and whether it supports the measured mechanism. Instruction counts, SIMD width, static throughput models, and profiler percentages do not by themselves prove lower latency.

## 5. Make a focused change

State a falsifiable hypothesis: the observed cost, the proposed change, the expected metric movement, and the likely regression risk. Prefer removing unnecessary work and improving algorithms or data representation before adding specialized instructions or concurrency, unless the evidence points elsewhere.

Keep independently testable ideas separate. Preserve the correctness contract, including exceptional inputs and hardware fallbacks. Check the boundary cases introduced by the change, such as vector tails, overflow, aliasing, ownership, or concurrency. Run suitable correctness checks separately from performance collection.

## 6. Remeasure and decide

Run baseline and candidate serially under equivalent conditions. Prefer randomized or balanced A/B blocks to reduce time drift; never benchmark competing versions simultaneously on the same host. Use the same valid harness and workload for both, then repeat the application-level case and relevant profiles.

Report absolute values, units, sample counts, variation, and an appropriate uncertainty estimate. Compare against the declared objective and regression budgets. Recheck the winning candidate in a fresh confirmation run after searching multiple variants. Keep visible any input or target that regresses.

Inspect the raw records and artifact identities behind a result before calling it independently verified. When reviewing supplied summaries without those records, attribute the numbers to the supplied evidence and qualify the conclusion; an assertion that artifacts exist is not verification of them.

Use one of these conclusions:

- **Measured improvement:** correctness holds, the intended metric improves beyond the experiment's uncertainty, the effect matters for the stated objective, and required budgets pass.
- **Tradeoff:** one objective improves while another regresses. Quantify both; do not silently accept a tradeoff outside the user's stated budget.
- **Inconclusive / unvalidated:** noise, missing coverage, or execution limits prevent the claimed conclusion.
- **No improvement / regression:** the measured change does not help. Remove only this experiment's unsuccessful edits, preserving unrelated user work and useful evidence.

Stop when the objective is met, credible scoped candidates are exhausted, or environmental noise/tool access prevents a meaningful decision. Do not keep rerunning until a favorable sample appears.

## Deliver evidence

Explain the original bottleneck, the concrete code change, and the result using the report reference. Include before/after latency, CPU cycles or the stated collection gap, memory measurements, and the assembly finding. Link raw results and exact reproduction commands. Distinguish measured results from estimates and hypotheses.

Keep benchmark code and concise results with the project when appropriate. Put large traces and binaries in a bounded scratch/cache directory and identify it; a performance task should not leave unlimited profiling data behind. Do not present illustrative numbers as measurements or claim broader hardware coverage than was tested.
