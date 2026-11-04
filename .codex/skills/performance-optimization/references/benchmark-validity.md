# Benchmark validity and comparison

## Workload and correctness

Define the useful operation before choosing the timer. A parser benchmark should validate parsed values and errors; a renderer should finish the relevant frame; an asynchronous producer should not stop timing immediately after enqueueing. Make output consumption observable without adding unrelated logging to the timed path.

Keep fixtures equivalent across variants. Record seeds, data hashes or fixture versions, input sizes, and distributions. Cover representative data plus cases that could invalidate the proposed optimization: empty and tiny inputs, large inputs, skew, duplicates, boundary lengths, and errors as relevant. A microbenchmark demonstrates its own workload, not the entire application.

Run the benchmark once with explicit correctness checks and inspect its operation count. For optimized native code, inspect enough assembly to check that the intended work survives compilation. An optimization barrier prevents some compiler transformations; it does not prove realistic inputs or correct outputs.

If setup is intentionally excluded, provide the same setup policy for both versions. Changing a reusable buffer's lifetime from one operation to the entire process changes the experiment. Measure its steady-state benefit and retained-memory/startup cost separately.

## Comparable artifacts

Record this manifest alongside the raw data:

| Field | What to capture |
|---|---|
| Source | Baseline/candidate revision, dirty patch, benchmark revision |
| Artifact | Absolute executable path, hash or build ID, linked dependency versions |
| Build | Full command, compiler/runtime, release profile, optimization, LTO/PGO, target features, allocator |
| Workload | Command, input identity, work count, concurrency, warm/cold policy, seed |
| Host | OS/kernel, CPU/architecture, core count/class, RAM, VM/container and effective quotas |
| Conditions | Load, thermal/power state, affinity if used, collection time, warmup and repetition policy |
| Collector | Tool/version, events, sampling rate, process scope, duration, units |

Use distinct artifact paths so a rebuild cannot accidentally replace the baseline. Detect stale outputs and wrong binaries. If the compiler or build flags are the intended change, label it as such and isolate it from source changes. Symbol-enabled or frame-pointer profiling builds must be identified if they differ from the deployed artifact; confirm the final result with the intended release configuration.

For a remote benchmark, execute the timer and work on the remote machine. SSH round-trip time belongs in the measurement only when remote interaction is the user-facing operation. Compare A and B on each host separately; a Mac result and a Linux VM result are not a before/after pair.

## Timing protocol

Use a monotonic timer or the established harness. For very short operations, use calibrated batches and account for harness overhead; retain independent process/run samples. Inner-loop iterations are not automatically independent statistical observations.

Choose warmup to match the question. Cold launch, cache population, JIT compilation, and steady state are distinct cases. Do not flush global caches or change host-wide CPU settings merely to create a cleaner local result. Keep other benchmark jobs and builds off the same host during measurement.

A practical comparison uses serial A/B blocks with randomized or balanced order. Each side uses equivalent initial state. Save trial order and all valid samples. If a run is disrupted, document the reason and rerun the affected block under the same rule; do not remove samples simply because they are slow. Keep timeout and failure observations visible rather than turning them into fast successes.

Start with a short pilot and increase duration/repetitions based on variation and the smallest effect worth detecting. Prefer the project's established statistical tools. Use a supported confidence interval for the chosen statistic and sampling design; for paired data, preserve pairs or time blocks in resampling. State the interval level and method. An interval on mean batch duration is not an interval on request p99. Avoid displaying more precision than the experiment supports.

If there is strong drift, multimodality, or large variation, diagnose scheduling, cache state, power, GC, I/O, and background load before concluding. More samples do not correct a biased workload or a mismatched environment. After trying several candidates, rerun the selected candidate against the baseline using fresh data to reduce selection bias.

## Latency and load

For a request-serving system, record offered and achieved load, concurrency, request count, arrival model, duration, errors, timeouts, and client-side bottlenecks. Show p50/p95/p99 only from actual operation-level observations with adequate tail coverage. State uncertainty or omit unsupported tail claims.

A closed-loop generator slows its arrivals when the service slows. This can hide queueing and tail latency under a fixed offered load (coordinated omission). Use an arrival model that matches the intended claim, and include queueing time where the user experiences it. Do not average per-worker or per-run percentiles; aggregate compatible raw samples or histograms, or report each run separately.

For an interactive app, measure input-to-usable-result latency and distinguish CPU work from waiting or rendering. For a function-only task, report function latency and do not invent service percentile coverage.

## CPU and memory interpretation

- Hardware cycles, retired instructions, CPU seconds, and wall seconds are separate measurements. Normalize counters by completed work when input sizes or throughput differ.
- IPC is instructions/cycles from comparable scopes and intervals. A low IPC value does not uniquely identify a dependency stall; memory, scheduling, and microarchitecture need consideration.
- Check supported events, permissions, counter running time, scaling/multiplexing, and VM visibility. Keep CPU classes and event semantics consistent. Unsupported counters are missing evidence, not zero.
- Distinguish allocated bytes over time, allocation count, live heap, post-burst retention, resident memory, and virtual address space. Record units and process/child scope.
- Pools and caches may reduce churn while increasing retained memory. Measure both under repeated use. A single peak-RSS result does not prove a leak; use a representative lifecycle or soak for that claim.
- Heavy profilers change timings. Compare equivalent profiles for diagnosis, and use separate uninstrumented runs for the speed claim.

## Presenting the effect

For lower-is-better metrics such as duration or bytes:

`reduction_percent = 100 * (baseline - candidate) / baseline`

For higher-is-better rates:

`increase_percent = 100 * (candidate - baseline) / baseline`

For time-based speedup:

`speedup_factor = baseline_duration / candidate_duration`

Require a positive baseline denominator, keep identical units and work definitions, and name the direction. A 2x speedup means half the duration, not a 200% time reduction. Show absolute values before percentages. Report each important workload rather than hiding regressions in an aggregate score. The minimum useful effect and allowed regressions come from the task, not a hard-coded universal threshold.
