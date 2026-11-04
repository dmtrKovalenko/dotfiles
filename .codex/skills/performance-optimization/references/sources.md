# References and selection

Reviewed on 2026-11-04. This is an original, focused skill written for the user's five requirements: correct benchmarks, valid results, concrete before/after changes, memory/CPU/latency profiling, and assembly inspection. The following public skills informed the comparison; they are references, not installed runtime dependencies. No third-party executable helpers are bundled.

## Existing skills compared

| Reference | Useful coverage | Reason to adapt rather than install unchanged |
|---|---|---|
| [Mnwa/performance-engineering](https://github.com/Mnwa/performance-engineering/blob/main/skills/performance-engineering/SKILL.md) | Closest overall fit: benchmark validity, memory lifetime, Rust/C++, generated code, and measured comparison | Broad package with many design/SIMD references; only limited public adoption was visible, so selection is based on inspected content rather than popularity |
| [Intel linux-perf](https://github.com/intel/intel-performance-skills/blob/main/skills/linux-perf/SKILL.md) and [performance-patterns](https://github.com/intel/intel-performance-skills/blob/main/skills/performance-patterns/SKILL.md) | Vendor-authored Linux hardware-counter and assembly workflows | x86/Linux focus; diagnostic heuristics need qualification and do not provide the whole Mac/latency/benchmark workflow |
| [jc1122/perf-benchmark](https://github.com/jc1122/perf-benchmark-skill/blob/main/SKILL.md) | Explicit comparison gates, environment matching, and missing/noisy-data handling | Linux/Python/C tooling and package-specific rubric; limited adoption; less suitable as a general Mac/Rust skill |
| [awesome-agv perf-optimization](https://github.com/irahardianto/awesome-agv/blob/main/.agents/skills/perf-optimization/SKILL.md) | Profiling-guided iteration and a broad optimization checklist | Integrates with other skills and prescribes commit/test conventions outside this user's requested scope |

The closest reference among those inspected is **Mnwa/performance-engineering**. Intel provides a useful complementary source for hardware tooling. This is a task-fit judgment, not a claim that either package has been independently proven superior to all available skills.

## Primary tool documentation

- [Criterion comparisons](https://bheisler.github.io/criterion.rs/book/user_guide/comparing_functions.html): comparing implementations and input cases within a benchmark harness.
- [Hyperfine](https://github.com/sharkdp/hyperfine): process-level benchmarking, warmups, and data export.
- [Linux perf documentation](https://perfwiki.github.io/main/tutorial/) and [perf stat manual](https://man7.org/linux/man-pages/man1/perf-stat.1.html): hardware-event collection and interpretation of repeated measurements.
- [Apple: Optimize CPU performance with Instruments](https://developer.apple.com/videos/play/wwdc2025/308/): CPU Counters, Processor Trace, and profiling on supported Apple hardware.
- [cargo-show-asm](https://github.com/pacak/cargo-show-asm): Rust assembly/IR inspection and target selection.

Verify version-sensitive flags against the installed tools. Example commands in this skill describe collection methods, not a claim that all profilers were installed or exercised during skill creation.
