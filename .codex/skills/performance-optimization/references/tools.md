# Platform tools

Choose tools already used by the repository when they provide the needed evidence. Inspect installed versions and help before relying on optional flags. These examples are starting points; use the actual executable and representative input. Bound recording durations and avoid loading the same machine with parallel profiler jobs.

## Harness selection

| Workload | Suitable starting point | Important limit |
|---|---|---|
| Rust function | Existing Criterion, Divan, or project benchmark | Confirm optimized profile, output consumption, and timing boundaries |
| Whole CLI command | Hyperfine or project process-level harness | Includes process startup; not a request latency distribution |
| Go | `go test -bench` with `-benchmem`, then benchstat | Keep toolchain, benchmark inputs, and run configuration comparable |
| C/C++ | Existing Google Benchmark or project harness | Inspect optimized code and compiler barriers |
| Python | pyperf and an appropriate CPU/allocation profiler | Python-only allocation tracking can miss native allocations |
| Service / interactive app | Existing operation-level timing and trace/load harness | Include usable completion and queueing when relevant |

Use these tools' native raw-data export and comparison support rather than writing an ad hoc statistical engine. Process-level measurements and microbenchmarks answer different questions. Hyperfine's ordinary command comparison is useful for screening; check run ordering and confirm small effects with balanced serial A/B blocks.

## Linux

Inspect support and permissions first:

```sh
perf --version
perf list
cat /proc/sys/kernel/perf_event_paranoid
```

Collect a small set of counters for a representative, bounded run; reduce the event set if scheduling or multiplexing prevents reliable collection:

```sh
perf stat -r 10 -e cycles,instructions,branches,branch-misses -- ./bench-before --fixture representative
perf stat -r 10 -e cycles,instructions,branches,branch-misses -- ./bench-after --fixture representative
```

These two commands illustrate collection, not a balanced final timing experiment. Do not assume hardware events work in a VM, that a particular paranoia value guarantees access, or that generic cache events mean the same thing on every CPU. Identify actual event semantics. Request supported execution permission when needed; do not automatically lower system-wide restrictions.

Find sampled hotspots and inspect the relevant function:

```sh
perf record -o before.perf.data -g --call-graph dwarf -- ./bench-before --fixture representative
perf report -i before.perf.data --stdio
perf annotate -i before.perf.data --stdio --symbol hot_function
```

Use matching after-run commands and distinct files. DWARF unwinding can have significant overhead; frame-pointer collection is an alternative for a suitable build. Sampling can attribute an event near, rather than exactly at, the causal instruction. Inspect callers and instruction sequences before attributing a stall to one line.

For memory, choose heaptrack or a runtime allocator profiler for allocation stacks. GNU `/usr/bin/time -v` is useful for process resource totals and peak RSS, not an allocation profile. For an existing process, use scoped `/proc` memory information as available and distinguish live snapshots from lifetime peaks. Valgrind-based tools can diagnose memory behavior or simulated instruction counts; their timing and simulated counters are not native hardware measurements.

Use `perf c2c`, lock profiling, or scheduler/off-CPU tracing only when contention or waiting is implicated and supported on the host.

## macOS / Apple silicon

Start by checking available Instruments templates:

```sh
xcrun xctrace list templates
```

Use Time Profiler for sampled CPU stacks, Allocations for allocation/lifetime analysis, and CPU Counters or Processor Trace where the installed Xcode and hardware support them. Select actual available templates rather than assuming every Mac supports every instrument. Follow the installed `xctrace help record` syntax for bounded command-line captures or use Instruments.

`/usr/bin/time -l ./bench-before ...` can supply CPU time and peak resource information for a process. Preserve its raw output and document the reported units. It does not replace allocation profiling or measured hardware cycles. `sample`, `vmmap`, and `leaks` may help answer a specific stack or memory question; none alone establishes a latency improvement.

Account for Apple silicon's different core classes and power/thermal state when comparing runs. Do not label an x86-only counter or instruction diagnosis as an Apple silicon finding. If counters are unavailable, use supported CPU/time/stack evidence and label cycles unavailable.

## Assembly for the measured hot path

Start with the benchmark binary itself. GNU or LLVM objdump can disassemble ELF binaries; on macOS, use available LLVM tools or `otool -tvV` for Mach-O. Keep debug symbols or dSYM files matched to the binary. Save only the relevant function and its callers/inlined regions, rather than dumping megabytes into the conversation.

For Rust, `cargo-show-asm` provides the `cargo asm` command. Check its installed help to select the package, target, profile, features, and symbol. It may rebuild the crate: generated output is comparable only if those settings match the measured version. Confirm final linked disassembly when LTO, code layout, or inlining matters. Use `cargo rustc --release -- --emit=asm` only with the appropriate target selection and the same code-generation settings; intermediate assembly is not always the linked binary's assembly.

For C/C++, use final-binary disassembly plus optimization remarks when useful. A separately compiled `-S` snippet can explain a compiler transformation but is not proof of how the real call site was compiled.

Check:

- Correct architecture, symbol, target features, and baseline/candidate identity.
- Calls, allocations, copies, bounds checks, branches, loads/stores, spills, synchronization, vector instructions, and loop dependencies relevant to the hypothesis.
- Code-size or instruction-cache tradeoffs, scalar fallbacks, and vector tails.
- Whether the changed instruction sequence is actually executed in the measured workload.

LLVM MCA and similar models estimate behavior for a modeled processor. Label their output as an estimate; validate changes on the real target. Do not upload private code or disassembly to external analysis sites as an implicit part of local inspection.
