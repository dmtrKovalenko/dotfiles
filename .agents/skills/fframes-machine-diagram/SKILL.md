---
name: fframes-machine-diagram
description: >-
  Build an animated "living system diagram" video with fframes: machines joined by rails, cards
  that travel through gates, slots with progress rings, a press that clamps while work runs,
  status lines that change with the same clock. The look comes from Kit Langton's merge-process
  visualization (x.com/kitlangton/status/2119860603229724930) and was first built as the Codex
  sampling loop diagram (examples/codex-loop in the fframes repository). Use when the user asks for
  an animated diagram, an explainer of how a loop, pipeline, queue, agent, protocol, build system
  or request flow works, "a diagram like the codex loop one", "in the Kit Langton style", or wants
  to change, extend or re-theme that video. Contains the complete working template, the exact
  motion numbers measured from the reference, the content research procedure and the review
  gates that the user's feedback produced.
---

# Machine diagrams with fframes

This skill reproduces one specific kind of video: a looping diagram in which a system is drawn
as machines on rails and its behaviour is shown by cards moving through them. Use it together
with the `fframes-video` skill, which covers installing fframes, the command line and the
general API. This file only says what is specific to this kind of video.

The working result is in `template/` (a complete, compiling video: 1338 lines of `lib.rs`, a
`main.rs` and the two font files). Start every new diagram from that template. Do not write
this kind of video from an empty file: three earlier attempts did, and the user rejected all of
them (see "What the user rejected").

Read `references/motion.md` before touching any animation and `references/reference-analysis.md`
before matching any other reference video.

## What the user rejected, and why

The user asked for a diagram of the Codex sampling loop in the style of the reference tweet.
It took four versions. Keep the reasons in mind, they are the quality bar.

| version | what it was | the user's verdict | the real problem |
| --- | --- | --- | --- |
| 1 | dark theme, three bands of text, small packets sliding with ease-in-out | "the moving animation should be springs with a much more interesting motion", "make it white", "reduce amount of text" | the look was copied from four screenshots, the motion was invented |
| 2 | white, one big loop, five labels, round dots thrown by bouncy springs, one caption in the middle | "need more details and animation is still bad, do a better job analyzing animation from the original video" | springs were added to nothing: no mechanisms, nothing for the motion to say. Detail was removed together with the text |
| 3 (kept) | white, machines with gates, slots, a press, cards with smear, travelling labels, a five-request story with parallel calls and a queued message | accepted, asked for this skill | built after downloading the reference and reading it frame by frame |

Rules that follow from it:

1. **Download the reference and read consecutive frames before designing motion.** Screenshots
   at two-second steps show the look and hide the motion. `references/reference-analysis.md`
   has the commands.
2. **The motion is in the mechanisms, not in the easing.** A card that moves from A to B with a
   spring is still boring. A gate that opens for it, a plate that clamps it, a ring that fills
   around it and a label that rides with it are what make the diagram alive.
3. **"Less text" never means less information.** Remove sentences, keep state. A status line
   that changes ("created" → "reasoning · · ·" → "item.done · start the tool now") is detail,
   not text.
4. **Several things happen at once.** At least once in the loop two cards must be in different
   machines at the same time (the reference always has three or four in flight).
5. **White background** is this user's choice. The reference is dark; the dark palette is in
   the table below for the day somebody asks for it.

## The procedure

Follow the steps in order. Each has a gate; do not go on until it passes.

### 1. Learn the system from its source, not from memory

The diagram must be true. For the Codex video the facts came from the current source file:

```bash
mkdir -p /tmp/src && cd /tmp/src
curl -fsSL -o turn.rs https://raw.githubusercontent.com/openai/codex/main/codex-rs/core/src/session/turn.rs
grep -nE "fn (run_turn|run_sampling_request|try_run_sampling_request|drain_in_flight)|needs_follow_up|ResponseEvent::[A-Za-z]+|get_pending_input|FuturesOrdered|stream_max_retries|auto_compact" turn.rs
```

Then read the functions the grep found. Write down, before any layout:

- the **stations**: where data rests or is processed (Codex: history, model, tools, the UI);
- the **cards**: what moves between stations (user input, prompt, reasoning item, tool call,
  tool output, final message);
- the **order of events inside one cycle**, with the names the code uses (`created`,
  `item.done`, `completed`, `in_flight`, `needs_follow_up`);
- what **ends the loop** (no tool call and no pending input);
- what happens **between cycles only sometimes** (pending input, auto-compact, stream retry);
- anything that runs **in parallel** (tool calls start at `item.done` while the stream goes on).

Gate: every label you plan to put on screen is a name from the source or a plain description
of something you read in it. Example commands and results in the story (`cargo test`,
`exit 101`) are illustrative; say so to the user in the final message.

File names move. `codex-rs/core/src/codex.rs` returned 404 and the loop was in
`session/turn.rs`. When a path is missing, list the directory with the GitHub API or search for
the function name instead of trusting memory.

### 2. Write the story as a table

One loop of the video is one concrete run of the system, chosen so that every mechanism is
used at least once. The Codex story:

| request | what the model asks for | what it shows |
| --- | --- | --- |
| 1 | `shell · cargo test`, fails, `exit 101` | a red output, the loop continues on failure |
| 2 | two shell calls in one response | two lanes at once, calls start during the stream |
| 3 | `apply_patch`, waits 0.9 s in "approve" | approval; while it runs you type a message that queues at the history |
| 4 | `shell · cargo test`, passes, `exit 0` | a green output |
| 5 | a message, no tool call | the loop ends, "turn complete" |

In the template this is `const TURN: [&[Call]; 4]` plus `INTERRUPTED` (the request during which
the extra message arrives). Everything on screen is derived from that table in
`CodexLoopVideo::new`; `render_frame` only samples it. Keep it that way: a change of story must
be a change of the table, never a special case in `render_frame`.

Gate: the story has a failure, a success, a wait, a parallel moment, a queue and an ending.

### 3. Scaffold and install the template

Inside the fframes repository:

```bash
just new my-diagram                                   # creates examples/my-diagram and registers it
S=~/.agents/skills/fframes-machine-diagram/template
cp $S/lib.rs examples/my-diagram/src/lib.rs
cp $S/main.rs examples/my-diagram/src/main.rs
rm examples/my-diagram/media/*.ttf && cp $S/media/*.ttf examples/my-diagram/media/
```

Then rename in both files: `CodexLoopMedia`, `CodexLoopVideo`, `codex_loop`, and the path in
`include_media_dir!(…, "examples/codex-loop/media")`, which is relative to the workspace root.
Outside the repository use `cargo fframes new my-diagram --yes` and the same copies; the media
path is then `"media"`.

Start `cargo build --release -p my-diagram` in the background right away. The first build is
the slow one.

Gate: `cargo run --release -p my-diagram -- inspect` prints "no problems found" for the
unchanged template.

### 4. Lay out the stations

Canvas 1920x1080, 60 fps (60 matters: the smear and the gate timing are sub-frame at 30).

- One main rail runs left to right at `RAIL_Y = 300`. Stations sit on it in the order data
  flows. The loop closes with a return rail along the bottom (`RETURN_Y = 720`) that comes up
  into the first station from below.
- A branch leaves the main rail downwards to the station that does side work (tools), whose
  exit joins the return rail.
- The main rail ends in a terminal on the right (the UI / "done"), with the event dots on it.
- Margins: 60 px left, 60 px right for text, nothing nearer than 60 px to an edge.
- The band below `y = 790` holds "what happens between cycles", as rows of dashed chips. It is
  static except for the row whose event is in the story, which lights while it happens.

Sizes that were tuned on real frames, keep them:

| thing | value |
| --- | --- |
| station label | 22 px, name in weight 600 ink, description in 400 dim, one line, above the box |
| section header | 20 px, weight 600, letter-spacing 2.5, upper case |
| status line | 20-22 px, centered under its machine or right of its name |
| small captions, chips, stage names | 18 px (the floor; nothing smaller) |
| machine box | fill `#fafafa`, stroke `#a8a8b2` 2 px, corner radius 18 |
| rail | `#d8d8de`, 2.5 px, round corners of radius 26 |
| card | 28 x 18 px (prompt 36 x 23, reasoning 18 x 11.5), corner radius 4 |
| slot | circle radius 24, stroke `#d8d8de` 2 px; lanes 70 px apart, stages 120 px apart |
| gate | two halves of 9 x 20 px, 11 px from the rail closed, 24 px open |
| history bar | 7 px high, 14.5 px pitch, widths 76-190 px by item kind |

The font is IBM Plex Mono, weights 400 and 600 only. Every glyph advances 0.6 em, so the x of
text that follows other text is computed (`label`), not measured.

Palette (white is the default):

| role | white theme | dark theme (the reference) |
| --- | --- | --- |
| background | `#ffffff` | `#0a0a0b` |
| machine fill | `#fafafa` | `#0e0e10` |
| divider | `#ececef` | `#242428` |
| rail | `#d8d8de` | `#3a3a3f` |
| machine edge, quiet marks | `#a8a8b2` | `#56565c` |
| secondary text | `#74747e` | `#8b8b91` |
| plates, sparks | `#3f3f46` | `#c9c9ce` |
| text, prompt card, message | `#18181b` | `#ededee` |
| active, waiting, in flight | `#e8930c` | `#f2b866` |
| success | `#17a34a` | `#7ed6a0` |
| failure | `#e5484d` | `#f07c6c` |

Color carries state and nothing else: amber is "working or waiting", green "ended well", red
"ended badly", ink "content", grey "structure". Do not add a fourth accent.

Gate: render `frame` at three times and check at full size that no text touches a machine, a
gate or another text. Mono text width is `chars * 0.6 * size`; compute it before placing.

### 5. Build the mechanisms

Each station needs a mechanism that fits what it does. The template has four; reuse them and
only invent a new one when none fits. Exact numbers are in `references/motion.md`.

| mechanism | in the template | use it for |
| --- | --- | --- |
| **gate** (`Gate`, `fn gate`) | every wall a card crosses | any boundary; it opens 0.16 s before the card and closes 0.1 s after |
| **press** (`fn press`, `Sampling`) | the model | one long operation on one card: plates clamp, six lights fill with the stages, sparks at the hit, side gates glow, release at the end |
| **slots** (`Stay`, `fn slot`) | tools, two lanes of three | staged or parallel work: a ring fills while the card stays, pops, unwinds, the card leans out |
| **stack** (`Row`) | the history | accumulation: one bar per item, color by kind, each springs in |
| **event rail** (`Dot`) | to the UI | a log: dots pop on, a call is a ring that fills with its result |
| **queue** (a `Card` with two legs and a `tag`) | the message typed mid-turn | waiting outside a gate until something else ends |
| **status line** (`Notes`, `fn note`) | under the model and tools, next to "events" | the name of the current stage; old text leaves upwards, new text rises |

A card is a list of legs (`Leg`: route, departure, spring). `fn leg` also registers which gates
the leg crosses and computes the moment of crossing from the spring, so gates never need
hand-written times. Colors change over time through `fills`, the lean through `tilts`, the
riding label through `tag`.

Gate: for every station you can say which mechanism it has and what its states are. A station
that is only a box with a label fails.

### 6. Time it

The schedule is a set of constants in seconds, relative to the start of a cycle
(`PROMPT_SENT`, `REASONING_DONE`, `ITEM_DONE`, `COMPLETED`, `TOOL_DONE`, …) and one `PERIOD`
(5.8 s) between cycles. Rules that held:

- A cycle takes 5-6 s. Under 5 s the status lines cannot be read; the first version at 5 s
  with more text was too fast, the second at 6.8 s with one caption felt slow.
- A travel between neighbouring stations takes 0.45-0.6 s, the long return 0.9 s, a hop
  between slots 0.3 s.
- Leave 0.3-0.5 s between the last arrival of a cycle and the start of the next.
- The loop ends with a 2 s hold on the finished state, then everything the run drew leaves in
  0.5 s (`RESET`), and 0.4 s of empty diagram before the loop point. Frame 0 and the last frame
  must be the same picture.
- Total 25-30 s.

Check the arithmetic of every stage (`lands + approval + HOP + sandbox + HOP < ran`); a stage
with a negative length draws nothing and does not fail.

### 7. Review like the reference was reviewed

Stills cannot judge this video. After `inspect` is clean, look at consecutive frames:

```bash
B=target/release/my-diagram   # run from the example's directory, or adjust the path
# 18 consecutive steps of 50 ms around one mechanism, cropped and zoomed
T=$(python3 -c "print(','.join(f'{7.1+i*0.05:.2f}s' for i in range(18)))")
$B strip $T --columns 3 --width 640 --crop 540,150,560,330 -o strip_a.png
# the whole diagram at the moments that matter
$B strip 5.9s,18.5s,23.4s,27.3s --columns 2 --width 960 -o strip.png
```

Make one such strip for each mechanism (press closing, a card through a gate and into a slot,
a slot finishing and the card leaving, the queue entering, the ending) and check against
`references/motion.md`:

- the card has a smear while fast and none at rest;
- the gate is open in the frames where the card is on the wall and closed two frames later;
- the plates stop on the card, they do not pass through it;
- the ring is full before it changes color, and the card leans before it leaves;
- a status line never shows two texts on top of each other at full opacity;
- nothing is drawn before its time (a bar of width 0.5 with full opacity shows as a speck:
  filter by time, do not rely on size).

`inspect --pops` reports one-frame changes. On this video it flags fast cards on the return
rail and status lines that swap; open two of the reported crops to confirm they are those and
not an element that appears without an entrance.

Then `cargo fmt -p my-diagram` and `cargo clippy --release -p my-diagram -- -D warnings`.

### 8. Deliver

```bash
$B render -o my-diagram.mp4
ffprobe -v error -show_entries stream=codec_type,width,height,nb_frames,duration -of csv=p=0 my-diagram.mp4
```

A 30 s loop renders in about 2 s on the Skia Metal backend. In a session that cannot attach
files, the user accepts an upload to catbox (they asked for it; it is public and cannot be
deleted from the session, say so every time):

```bash
curl -sS -F reqtype=fileupload -F fileToUpload=@my-diagram.mp4 https://catbox.moe/user/api.php
```

Also give the preview command with an absolute path:
`cd <repo> && cargo run --release -p my-diagram -- preview 0s`.

In the final message say what was checked and how (stills, strips, `inspect`), that you did
not watch it play, and which parts of the story are illustrative. Do not commit unless asked,
and never add Claude attribution to commits for this user.

## Changing the Codex video

The source is `examples/codex-loop` in the fframes repository (created in a worktree; it is
not committed unless the user did that later, the copy in `template/` is the reference).

| the user wants | change |
| --- | --- |
| another story | `TURN`, `INTERRUPTED`; a third parallel call needs a third entry in `LANES_Y` and a taller tools box |
| slower or faster | `PERIOD` and the constants under "Seconds inside a sampling request"; recheck step 6 |
| another theme | the color constants only; `PAPER` is also used as the fill behind badges and rings |
| less or more text | the `backdrop` function holds every static label; the `Notes` pushed in `new` hold every changing one |
| another station | a `machine(...)` in `backdrop`, its gates in the `gates` array in `new`, routes through it, and a mechanism from step 5 |
| bigger for a phone | scale the layout constants, not the SVG: text must stay at or above 18 px at 1080p |

## Adapting to another system

1. Do step 1 for the new system. Map it: resting places → stations, messages or jobs → cards,
   stages of one operation → the six lights of a press or the columns of a slot machine,
   retries and queues → the band below.
2. Keep the skeleton: source on the left, the thing that accumulates next, the thing that
   computes in the middle, the side work on a branch, the result on the right, the return rail
   below. Most request loops, build pipelines and agents fit it. If the system is a straight
   pipeline, drop the return rail and let cards end in the terminal.
3. Rename the constants and the labels, rewrite `TURN` as the new story, and only then touch
   the mechanisms.
4. Keep every number of `references/motion.md` unless a strip shows a reason to change it.
