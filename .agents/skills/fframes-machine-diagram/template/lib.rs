//! A looping diagram of the Codex sampling loop (`run_turn` in codex-rs
//! `core/src/session/turn.rs`): one user turn that takes five sampling requests.
//!
//! The turn is a table ([`TURN`]). [`CodexLoopVideo::new`] turns it into cards that travel
//! along [`Route`]s, gates that open for them, slots that process them and labels that
//! follow the same clock.
use fframes::{
    AudioMap, Color, Duration, FFramesContext, Frame, Svgr, Video, animation::SpringRuntime,
    include_media_dir, svgr,
};

include_media_dir!(pub struct CodexLoopMedia, "examples/codex-loop/media");

pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;

const MONO: &str = "IBM Plex Mono";
/// Advance of every glyph of the mono font, in em.
const ADVANCE: f32 = 0.6;

const PAPER: &str = "#ffffff";
const PANEL: &str = "#fafafa";
const FAINT: &str = "#ececef";
const RAIL: &str = "#d8d8de";
const EDGE: &str = "#a8a8b2";
const DIM: &str = "#74747e";
const STEEL: &str = "#3f3f46";
const INK: &str = "#18181b";
const AMBER: &str = "#e8930c";
const GREEN: &str = "#17a34a";
const RED: &str = "#e5484d";

/// Seconds between the starts of two sampling requests.
const PERIOD: f32 = 5.8;
const FIRST_REQUEST: f32 = 1.3;
const INPUT_SENT: f32 = 0.3;
const INPUT_RECORDED: f32 = 0.95;

// Seconds inside a sampling request.
const PROMPT_SENT: f32 = 0.55;
const CREATED: f32 = 0.62;
const REASONING: f32 = 0.72;
const REASONING_DONE: f32 = 1.2;
const ITEM_STARTS: f32 = 1.3;
const ITEM_DONE: f32 = 1.8;
/// A second tool call of the same response follows this much later.
const NEXT_CALL: f32 = 0.25;
const COMPLETED: f32 = 2.3;
const CALL_LANDS: f32 = 0.6;
const HOP: f32 = 0.3;
const TOOL_DONE: f32 = 4.3;
const OUTPUT_TRAVELS: f32 = 0.9;
const TURN_COMPLETE: f32 = 2.5;

// Layout.
const RAIL_Y: f32 = 300.0;
const HISTORY: (f32, f32) = (440.0, 300.0);
const MODEL: (f32, f32) = (930.0, 300.0);
const BRANCH_X: f32 = 1180.0;
const TOOLS: (f32, f32) = (1510.0, 525.0);
const SLOTS_X: [f32; 3] = [1390.0, 1510.0, 1630.0];
const LANES_Y: [f32; 2] = [490.0, 560.0];
const STAGE_NAMES: [&str; 3] = ["approve", "sandbox", "run"];
const RETURN_X: f32 = 1830.0;
const RETURN_Y: f32 = 720.0;
const DOTS_X: f32 = 1250.0;
const DOTS_STEP: f32 = 34.0;
const END_X: f32 = 1820.0;
const QUEUE_X: f32 = 286.0;

struct Call {
    name: &'static str,
    /// Seconds the call waits in the approval slot.
    approval: f32,
    ok: bool,
    /// What the output card says on its way back.
    result: &'static str,
    /// What the UI hears when it ends.
    event: &'static str,
}

/// The tool calls of each request that has any. The request after the last one answers.
const TURN: [&[Call]; 4] = [
    &[Call {
        name: "shell · cargo test",
        approval: 0.25,
        ok: false,
        result: "exit 101",
        event: "cargo test failed",
    }],
    &[
        Call {
            name: "shell · rg parse_range",
            approval: 0.25,
            ok: true,
            result: "3 matches",
            event: "rg ended",
        },
        Call {
            name: "shell · sed -n 40,80p",
            approval: 0.25,
            ok: true,
            result: "41 lines",
            event: "sed ended",
        },
    ],
    &[Call {
        name: "apply_patch · src/range.rs",
        approval: 0.9,
        ok: true,
        result: "patched",
        event: "patch applied",
    }],
    &[Call {
        name: "shell · cargo test",
        approval: 0.25,
        ok: true,
        result: "exit 0",
        event: "tests pass",
    }],
];
const REQUESTS: usize = TURN.len() + 1;
/// The request during which you type one more message.
const INTERRUPTED: usize = 2;

const DONE_AT: f32 = FIRST_REQUEST + TURN.len() as f32 * PERIOD + TURN_COMPLETE;
/// Everything the turn drew leaves from here on, so the last frame matches the first.
const RESET: f32 = DONE_AT + 2.0;
const LOOP: f32 = RESET + 0.9;

fn request_start(request: usize) -> f32 {
    FIRST_REQUEST + request as f32 * PERIOD
}

fn smooth(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// 0 before `from`, 1 after `to`.
fn ramp(t: f32, from: f32, to: f32) -> f32 {
    smooth((t - from) / (to - from))
}

/// 1 between `from` and `to`, with soft edges.
fn window(t: f32, from: f32, to: f32) -> f32 {
    ramp(t, from, from + 0.15) * (1.0 - ramp(t, to, to + 0.25))
}

/// A spring that first reaches its target after `seconds`. A lower `damping_ratio` overshoots more.
fn throw(seconds: f32, damping_ratio: f32) -> SpringRuntime {
    let damped = (1.0 - damping_ratio * damping_ratio).sqrt();
    let frequency = (std::f32::consts::PI - (damped / damping_ratio).atan()) / (damped * seconds);
    SpringRuntime::new(1.0, frequency * frequency, 2.0 * damping_ratio * frequency)
}

/// 0 → 1 of a spring that started `since` seconds ago, 0 before it started.
fn sprung(spring: &SpringRuntime, since: f32) -> f32 {
    if since <= 0.0 {
        0.0
    } else {
        spring.solve(&since)
    }
}

/// 1 when something was hit `since` seconds ago, then it swings back to 0.
fn rings(spring: &SpringRuntime, since: f32) -> f32 {
    if since < 0.0 {
        0.0
    } else {
        1.0 - spring.solve(&since)
    }
}

/// Seconds a spring needs to cover `fraction` of its way.
fn reach(spring: &SpringRuntime, fraction: f32) -> f32 {
    let mut seconds = 0.0;
    while seconds < 4.0 && spring.solve(&seconds) < fraction {
        seconds += 1.0 / 240.0;
    }
    seconds
}

/// A polyline with rounded corners that cards travel along.
struct Route {
    points: Vec<(f32, f32)>,
    /// Length of the route up to each point.
    lengths: Vec<f32>,
}

impl Route {
    fn new(corners: &[(f32, f32)]) -> Self {
        const RADIUS: f32 = 26.0;
        let mut points = vec![corners[0]];
        for window in corners.windows(3) {
            let (previous, corner, next) = (window[0], window[1], window[2]);
            let toward = |target: (f32, f32)| {
                let (dx, dy) = (target.0 - corner.0, target.1 - corner.1);
                let length = dx.hypot(dy);
                let reach = RADIUS.min(length / 2.0);
                (
                    corner.0 + dx / length * reach,
                    corner.1 + dy / length * reach,
                )
            };
            let (from, to) = (toward(previous), toward(next));
            for step in 0..=8 {
                let u = step as f32 / 8.0;
                let v = 1.0 - u;
                points.push((
                    v * v * from.0 + 2.0 * v * u * corner.0 + u * u * to.0,
                    v * v * from.1 + 2.0 * v * u * corner.1 + u * u * to.1,
                ));
            }
        }
        points.push(corners[corners.len() - 1]);

        let mut lengths = vec![0.0];
        for pair in points.windows(2) {
            let length = (pair[1].0 - pair[0].0).hypot(pair[1].1 - pair[0].1);
            lengths.push(lengths[lengths.len() - 1] + length);
        }
        Self { points, lengths }
    }

    fn total(&self) -> f32 {
        self.lengths[self.lengths.len() - 1]
    }

    /// The point at `u` of the route. Above 1 it carries on straight, so a spring can overshoot.
    fn at(&self, u: f32) -> (f32, f32) {
        let last = self.points.len() - 1;
        if u > 1.0 {
            let (from, to) = (self.points[last - 1], self.points[last]);
            let span = (self.lengths[last] - self.lengths[last - 1]).max(0.001);
            let beyond = (u - 1.0) * self.total().min(160.0) / span;
            return (
                to.0 + (to.0 - from.0) * beyond,
                to.1 + (to.1 - from.1) * beyond,
            );
        }
        let target = u.max(0.0) * self.total();
        let next = self
            .lengths
            .partition_point(|length| *length < target)
            .clamp(1, last);
        let (from, to) = (self.points[next - 1], self.points[next]);
        let span = self.lengths[next] - self.lengths[next - 1];
        let u = if span > 0.0 {
            (target - self.lengths[next - 1]) / span
        } else {
            0.0
        };
        (from.0 + (to.0 - from.0) * u, from.1 + (to.1 - from.1) * u)
    }

    /// How far along the route the point closest to `target` is, 0..1.
    fn fraction_near(&self, target: (f32, f32)) -> f32 {
        let distance = |point: &(f32, f32)| (point.0 - target.0).hypot(point.1 - target.1);
        let nearest = (0..self.points.len())
            .min_by(|a, b| distance(&self.points[*a]).total_cmp(&distance(&self.points[*b])))
            .unwrap_or(0);
        self.lengths[nearest] / self.total()
    }

    fn path(&self) -> String {
        let mut d = String::new();
        for (index, (x, y)) in self.points.iter().enumerate() {
            d.push_str(&format!(
                "{}{x:.1} {y:.1}",
                if index == 0 { "M" } else { " L" }
            ));
        }
        d
    }
}

/// One move of a card: a spring throws it down a route at `departs`.
struct Leg {
    route: usize,
    departs: f32,
    spring: SpringRuntime,
}

/// A card that travels through the machines.
struct Card {
    legs: Vec<Leg>,
    /// When it disappears into a machine.
    dies: f32,
    width: f32,
    /// Its color from a second on.
    fills: Vec<(f32, &'static str)>,
    /// A green dot in its corner, as on a tool call.
    badge: bool,
    /// When it leans out of a slot.
    tilts: Vec<f32>,
    /// Text that travels with it between two seconds.
    tag: Option<(&'static str, f32, f32)>,
}

/// A door in the wall of a machine. It opens for every card that crosses it.
struct Gate {
    at: (f32, f32),
    /// On the top or bottom wall.
    flat: bool,
    opens: Vec<f32>,
}

const GATE_HISTORY_IN: usize = 0;
const GATE_HISTORY_OUT: usize = 1;
const GATE_HISTORY_BACK: usize = 2;
const GATE_MODEL_IN: usize = 3;
const GATE_MODEL_OUT: usize = 4;
const GATE_TOOLS_IN: usize = 5;
const GATE_TOOLS_OUT: usize = 6;

/// A tool call being in one slot of the tools machine.
struct Stay {
    lane: usize,
    stage: usize,
    starts: f32,
    ends: f32,
    ok: bool,
}

enum RowKind {
    User,
    Reasoning,
    Call,
    Output(bool),
    Message,
}

/// A response item in the history.
struct Row {
    at: f32,
    kind: RowKind,
}

enum DotKind {
    Reasoning,
    /// A tool call: a ring while it runs, filled when it ends.
    Call {
        ends: f32,
        ok: bool,
    },
    Message,
}

/// An event on the rail to the UI.
struct Dot {
    at: f32,
    kind: DotKind,
}

/// A line of text that changes over time: what it says from a second on, and its color.
type Notes = Vec<(f32, String, &'static str)>;

/// What a request does to the model: when it clamps the prompt, lights and lets go.
struct Sampling {
    starts: f32,
    clamps: f32,
    completes: f32,
    /// When each of the six lights on the press turns on.
    lights: [f32; 6],
}

pub struct CodexLoopVideo<'a> {
    pub media: &'a CodexLoopMedia,
    routes: Vec<Route>,
    gates: Vec<Gate>,
    cards: Vec<Card>,
    stays: Vec<Stay>,
    rows: Vec<Row>,
    dots: Vec<Dot>,
    samplings: Vec<Sampling>,
    model_notes: Notes,
    event_notes: Notes,
    tool_notes: Notes,
    /// When an output is on its way back to the history.
    follow_ups: Vec<(f32, f32)>,
    /// When the message you typed mid-turn waits at the history.
    queued: (f32, f32),
    /// A bouncy spring for things that pop in or get hit.
    pop: SpringRuntime,
    /// The press of the model closing and opening.
    clamp: SpringRuntime,
}

impl<'a> CodexLoopVideo<'a> {
    pub fn new(media: &'a CodexLoopMedia) -> Self {
        let mut video = Self {
            media,
            routes: Vec::new(),
            gates: [
                ((320.0, RAIL_Y), false),
                ((560.0, RAIL_Y), false),
                ((HISTORY.0, 450.0), true),
                ((800.0, RAIL_Y), false),
                ((1060.0, RAIL_Y), false),
                ((1300.0, TOOLS.1), false),
                ((1720.0, TOOLS.1), false),
            ]
            .into_iter()
            .map(|(at, flat)| Gate {
                at,
                flat,
                opens: Vec::new(),
            })
            .collect(),
            cards: Vec::new(),
            stays: Vec::new(),
            rows: vec![Row {
                at: INPUT_RECORDED,
                kind: RowKind::User,
            }],
            dots: Vec::new(),
            samplings: Vec::new(),
            model_notes: vec![(0.0, "waiting for input".to_owned(), DIM)],
            event_notes: vec![(0.0, "nothing yet".to_owned(), DIM)],
            tool_notes: vec![(0.0, "idle".to_owned(), DIM)],
            follow_ups: Vec::new(),
            queued: (0.0, 0.0),
            pop: SpringRuntime::new(1.0, 260.0, 11.0),
            clamp: SpringRuntime::new(1.0, 420.0, 30.0),
        };

        // Your message goes into the history.
        let input = video.route(&[(150.0, RAIL_Y), (350.0, RAIL_Y)]);
        let leg = video.leg(
            input,
            INPUT_SENT,
            INPUT_RECORDED - INPUT_SENT,
            &[GATE_HISTORY_IN],
        );
        video.cards.push(Card {
            legs: vec![leg],
            dies: INPUT_RECORDED,
            width: 28.0,
            fills: vec![(0.0, AMBER)],
            badge: false,
            tilts: Vec::new(),
            tag: None,
        });

        let prompt = video.route(&[(500.0, RAIL_Y), MODEL]);
        let message = video.route(&[MODEL, (END_X, RAIL_Y)]);
        for request in 0..REQUESTS {
            let start = request_start(request);
            let calls = TURN.get(request).copied().unwrap_or(&[]);
            let completes = start + COMPLETED + NEXT_CALL * calls.len().saturating_sub(1) as f32;
            let note = |notes: &mut Notes, at: f32, text: &str, fill| {
                notes.push((start + at, text.to_owned(), fill))
            };

            // The history becomes the prompt and the model holds it while it streams.
            let leg = video.leg(
                prompt,
                start,
                PROMPT_SENT,
                &[GATE_HISTORY_OUT, GATE_MODEL_IN],
            );
            video.cards.push(Card {
                legs: vec![leg],
                dies: completes + 0.15,
                width: 36.0,
                fills: vec![(0.0, INK)],
                badge: false,
                tilts: Vec::new(),
                tag: None,
            });
            video.samplings.push(Sampling {
                starts: start,
                clamps: start + PROMPT_SENT,
                completes,
                lights: [
                    start + CREATED,
                    start + REASONING,
                    start + REASONING_DONE,
                    start + ITEM_STARTS,
                    start + ITEM_DONE,
                    completes,
                ],
            });
            note(&mut video.model_notes, 0.0, "building the prompt", DIM);
            note(&mut video.model_notes, CREATED, "created", DIM);
            note(&mut video.model_notes, REASONING, "reasoning · · ·", DIM);
            note(
                &mut video.model_notes,
                REASONING_DONE,
                "item.done · reasoning",
                INK,
            );

            // Every response thinks first: a reasoning item for the history and the UI.
            let dot_x = DOTS_X + video.dots.len() as f32 * DOTS_STEP;
            let reasoning = video.route(&[MODEL, (dot_x, RAIL_Y)]);
            let leg = video.leg(reasoning, start + REASONING_DONE, 0.45, &[GATE_MODEL_OUT]);
            video.cards.push(Card {
                legs: vec![leg],
                dies: start + REASONING_DONE + 0.5,
                width: 18.0,
                fills: vec![(0.0, EDGE)],
                badge: false,
                tilts: Vec::new(),
                tag: None,
            });
            video.rows.push(Row {
                at: start + REASONING_DONE,
                kind: RowKind::Reasoning,
            });
            video.dots.push(Dot {
                at: start + REASONING_DONE + 0.5,
                kind: DotKind::Reasoning,
            });
            note(
                &mut video.event_notes,
                REASONING_DONE + 0.5,
                "reasoning",
                DIM,
            );

            if calls.is_empty() {
                // No tool call: the answer goes to the UI and the turn ends.
                let leg = video.leg(
                    message,
                    start + ITEM_DONE,
                    TURN_COMPLETE - ITEM_DONE,
                    &[GATE_MODEL_OUT],
                );
                video.cards.push(Card {
                    legs: vec![leg],
                    dies: start + TURN_COMPLETE,
                    width: 28.0,
                    fills: vec![(0.0, INK)],
                    badge: false,
                    tilts: Vec::new(),
                    tag: None,
                });
                video.rows.push(Row {
                    at: start + ITEM_DONE,
                    kind: RowKind::Message,
                });
                video.dots.push(Dot {
                    at: start + ITEM_DONE + 0.4,
                    kind: DotKind::Message,
                });
                note(&mut video.model_notes, ITEM_STARTS, "message · · ·", DIM);
                note(
                    &mut video.model_notes,
                    ITEM_DONE,
                    "item.done · message",
                    INK,
                );
                note(
                    &mut video.model_notes,
                    COMPLETED,
                    "completed · nothing to run",
                    GREEN,
                );
                note(
                    &mut video.event_notes,
                    ITEM_DONE + 0.4,
                    "agent message",
                    INK,
                );
                note(
                    &mut video.event_notes,
                    TURN_COMPLETE,
                    "turn complete",
                    GREEN,
                );
                continue;
            }

            note(
                &mut video.model_notes,
                ITEM_STARTS,
                "function_call · · ·",
                DIM,
            );
            note(
                &mut video.model_notes,
                completes - start,
                "completed · tools still in flight",
                AMBER,
            );
            let mut last_recorded = 0.0_f32;
            for (lane, call) in calls.iter().enumerate() {
                let done = start + ITEM_DONE + NEXT_CALL * lane as f32;
                let lands = done + CALL_LANDS;
                let approved = lands + call.approval;
                let sandboxed = approved + HOP + 0.3;
                let ran = start + TOOL_DONE + NEXT_CALL * lane as f32;
                let recorded = ran + OUTPUT_TRAVELS;
                last_recorded = last_recorded.max(recorded);
                let lane_y = LANES_Y[lane];
                let fill = if call.ok { GREEN } else { RED };

                let enter = video.route(&[
                    MODEL,
                    (BRANCH_X, RAIL_Y),
                    (BRANCH_X, TOOLS.1),
                    (1326.0, TOOLS.1),
                    (SLOTS_X[0], lane_y),
                ]);
                let hops = [
                    video.route(&[(SLOTS_X[0], lane_y), (SLOTS_X[1], lane_y)]),
                    video.route(&[(SLOTS_X[1], lane_y), (SLOTS_X[2], lane_y)]),
                ];
                let back = video.route(&[
                    (SLOTS_X[2], lane_y),
                    (1694.0, TOOLS.1),
                    (RETURN_X, TOOLS.1),
                    (RETURN_X, RETURN_Y),
                    (HISTORY.0, RETURN_Y),
                    (HISTORY.0, 420.0),
                ]);
                let legs = vec![
                    video.leg(enter, done, CALL_LANDS, &[GATE_MODEL_OUT, GATE_TOOLS_IN]),
                    video.leg(hops[0], approved, HOP, &[]),
                    video.leg(hops[1], sandboxed, HOP, &[]),
                    video.leg(
                        back,
                        ran,
                        OUTPUT_TRAVELS,
                        &[GATE_TOOLS_OUT, GATE_HISTORY_BACK],
                    ),
                ];
                video.cards.push(Card {
                    legs,
                    dies: recorded,
                    width: 28.0,
                    fills: vec![(0.0, AMBER), (ran - 0.12, fill)],
                    badge: true,
                    tilts: vec![approved, sandboxed, ran],
                    tag: Some((call.result, ran + 0.15, recorded - 0.1)),
                });
                video.stays.extend([
                    Stay {
                        lane,
                        stage: 0,
                        starts: lands,
                        ends: approved,
                        ok: true,
                    },
                    Stay {
                        lane,
                        stage: 1,
                        starts: approved + HOP,
                        ends: sandboxed,
                        ok: true,
                    },
                    Stay {
                        lane,
                        stage: 2,
                        starts: sandboxed + HOP,
                        ends: ran,
                        ok: call.ok,
                    },
                ]);
                video.follow_ups.push((ran, recorded));

                video.rows.push(Row {
                    at: done,
                    kind: RowKind::Call,
                });
                video.rows.push(Row {
                    at: recorded,
                    kind: RowKind::Output(call.ok),
                });
                video.dots.push(Dot {
                    at: lands,
                    kind: DotKind::Call {
                        ends: ran,
                        ok: call.ok,
                    },
                });
                video.model_notes.push((
                    done,
                    if lane == 0 {
                        "item.done · start the tool now"
                    } else {
                        "item.done · and one more"
                    }
                    .to_owned(),
                    INK,
                ));
                video
                    .event_notes
                    .push((lands, "exec began".to_owned(), AMBER));
                video.event_notes.push((ran, call.event.to_owned(), fill));

                let name = if calls.len() > 1 {
                    "2 calls, side by side"
                } else {
                    call.name
                };
                if lane == 0 {
                    video.tool_notes.push((lands, name.to_owned(), AMBER));
                }
                if call.approval > 0.5 {
                    video.tool_notes.push((
                        lands + 0.05,
                        "apply_patch · waits for you".to_owned(),
                        AMBER,
                    ));
                    video.tool_notes.push((approved, name.to_owned(), AMBER));
                }
            }
            video
                .tool_notes
                .push((last_recorded - OUTPUT_TRAVELS + 0.3, "idle".to_owned(), DIM));
            video.model_notes.push((
                last_recorded,
                "outputs recorded · sample again".to_owned(),
                DIM,
            ));

            // One more message from you: it waits at the history until the request is over.
            if request == INTERRUPTED {
                let (sent, drained) = (start + 1.0, start + PERIOD - 0.45);
                let wait = video.route(&[(150.0, RAIL_Y), (QUEUE_X, RAIL_Y)]);
                let enter = video.route(&[(QUEUE_X, RAIL_Y), (350.0, RAIL_Y)]);
                let legs = vec![
                    video.leg(wait, sent, 0.45, &[]),
                    video.leg(enter, drained, 0.3, &[GATE_HISTORY_IN]),
                ];
                video.cards.push(Card {
                    legs,
                    dies: drained + 0.3,
                    width: 28.0,
                    fills: vec![(0.0, AMBER)],
                    badge: false,
                    tilts: Vec::new(),
                    tag: Some(("queued", sent + 0.45, drained)),
                });
                video.rows.push(Row {
                    at: drained + 0.3,
                    kind: RowKind::User,
                });
                video.queued = (sent, drained + 0.3);
            }
        }

        // Items show up in the order they were recorded.
        video.rows.sort_by(|a, b| a.at.total_cmp(&b.at));
        for notes in [
            &mut video.model_notes,
            &mut video.event_notes,
            &mut video.tool_notes,
        ] {
            notes.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        video
    }

    fn route(&mut self, corners: &[(f32, f32)]) -> usize {
        self.routes.push(Route::new(corners));
        self.routes.len() - 1
    }

    /// A move that takes `seconds`, and the gates it opens on its way.
    fn leg(&mut self, route: usize, departs: f32, seconds: f32, gates: &[usize]) -> Leg {
        let spring = throw(seconds, 0.78);
        for gate in gates {
            let fraction = self.routes[route].fraction_near(self.gates[*gate].at);
            self.gates[*gate]
                .opens
                .push(departs + reach(&spring, fraction));
        }
        Leg {
            route,
            departs,
            spring,
        }
    }

    /// Where a card is, or `None` before its first move.
    fn card_at(&self, card: &Card, t: f32) -> Option<(f32, f32)> {
        let leg = card.legs.iter().rev().find(|leg| leg.departs <= t)?;
        Some(self.routes[leg.route].at(leg.spring.solve(&(t - leg.departs))))
    }
}

impl std::fmt::Debug for CodexLoopVideo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexLoopVideo").finish_non_exhaustive()
    }
}

fn text(
    x: f32,
    y: f32,
    size: f32,
    fill: &'static str,
    content: impl Into<String>,
) -> Svgr<'static> {
    svgr!(<text x={x} y={y} font-size={size} font-weight="400" fill={fill}>{content.into()}</text>)
}

/// "**head** tail": a bold name and what it is, as in "model · one streamed response".
fn label(x: f32, y: f32, head: &'static str, tail: &'static str) -> Svgr<'static> {
    const SIZE: f32 = 22.0;
    let tail_x = x + (head.chars().count() + 1) as f32 * ADVANCE * SIZE;
    svgr!(
        <g font-size={SIZE}>
            <text x={x} y={y} font-weight="600" fill={INK}>{head}</text>
            <text x={tail_x} y={y} font-weight="400" fill={DIM}>{tail}</text>
        </g>
    )
}

/// A line that changes: the new text rises into place while the old one leaves upwards.
fn note(notes: &Notes, t: f32, x: f32, y: f32, size: f32, anchor: &'static str) -> Svgr<'static> {
    let current = notes.iter().rposition(|note| note.0 <= t).unwrap_or(0);
    let since = t - notes[current].0;
    let line = |index: usize, shown: f32, lift: f32| {
        let (_, content, fill) = &notes[index];
        svgr!(<text x={x} y={y + lift} font-size={size} font-weight="400" fill={*fill} text-anchor={anchor}
            opacity={shown}>
            {content.clone()}
        </text>)
    };
    let leaving = if current > 0 && since < 0.18 {
        line(
            current - 1,
            1.0 - ramp(since, 0.0, 0.1),
            -16.0 * ramp(since, 0.0, 0.18),
        )
    } else {
        Svgr::empty()
    };
    svgr!(
        <g>
            {leaving}
            {line(current, ramp(since, 0.08, 0.24), 14.0 * (1.0 - ramp(since, 0.0, 0.26)))}
        </g>
    )
}

fn chip_width(content: &str) -> f32 {
    content.chars().count() as f32 * ADVANCE * 18.0 + 26.0
}

/// A dashed pill with a word in it. `lit` is 0..1 amber.
fn chip(x: f32, y: f32, content: &'static str, lit: f32) -> Svgr<'static> {
    let width = chip_width(content);
    let (stroke, ink) = if lit > 0.5 {
        (AMBER, AMBER)
    } else {
        (EDGE, DIM)
    };
    svgr!(
        <g>
            <rect x={x} y={y - 17.0} width={width} height="34" rx="8" fill={PAPER} />
            <rect x={x} y={y - 17.0} width={width} height="34" rx="8" fill={AMBER} opacity={lit * 0.12} />
            <rect x={x} y={y - 17.0} width={width} height="34" rx="8" fill="none" stroke={stroke} stroke-width="1.5"
                stroke-dasharray="5 4" />
            <text x={x + width / 2.0} y={y + 6.0} font-size="18" font-weight="400" fill={ink} text-anchor="middle">
                {content}
            </text>
        </g>
    )
}

/// A row of the "between requests" section: a name and the steps it takes.
fn steps(
    y: f32,
    head: &'static str,
    tail: &'static str,
    contents: &[&'static str],
    lit: f32,
) -> Svgr<'static> {
    let mut left = 520.0;
    let chips: Vec<_> = contents
        .iter()
        .map(|content| {
            let chip = chip(left, y, content, lit);
            left += chip_width(content) + 40.0;
            chip
        })
        .collect();
    svgr!(
        <g>
            {label(60.0, y + 7.0, head, tail)}
            <line x1="520" y1={y} x2={left - 60.0} y2={y} stroke={EDGE} stroke-width="1.5" stroke-dasharray="5 4" />
            {chips}
        </g>
    )
}

/// A machine: a rounded box, drawn around its center.
fn machine((x, y): (f32, f32), width: f32, height: f32) -> Svgr<'static> {
    svgr!(<rect x={x - width / 2.0} y={y - height / 2.0} width={width} height={height} rx="18" fill={PANEL}
        stroke={EDGE} stroke-width="2" />)
}

/// An arc path. Angles are degrees, clockwise from the top.
fn arc(cx: f32, cy: f32, radius: f32, from: f32, to: f32) -> String {
    let to = to.clamp(from + 0.5, from + 359.9);
    let point = |angle: f32| {
        let (sin, cos) = angle.to_radians().sin_cos();
        (cx + radius * sin, cy - radius * cos)
    };
    let (start, end) = (point(from), point(to));
    let large = if to - from > 180.0 { 1 } else { 0 };
    format!(
        "M{:.2} {:.2} A{radius} {radius} 0 {large} 1 {:.2} {:.2}",
        start.0, start.1, end.0, end.1
    )
}

impl CodexLoopVideo<'_> {
    /// Rails, machines and every label that never changes.
    fn backdrop(&self) -> Svgr<'static> {
        let branch = Route::new(&[(BRANCH_X, RAIL_Y), (BRANCH_X, TOOLS.1), (1300.0, TOOLS.1)]);
        let back = Route::new(&[
            (1720.0, TOOLS.1),
            (RETURN_X, TOOLS.1),
            (RETURN_X, RETURN_Y),
            (HISTORY.0, RETURN_Y),
            (HISTORY.0, 450.0),
        ]);
        let stage_names: Vec<_> = STAGE_NAMES
            .into_iter()
            .zip(SLOTS_X)
            .map(|(name, x)| {
                svgr!(<text x={x} y="640" font-size="18" font-weight="400" fill={DIM} text-anchor="middle">{name}</text>)
            })
            .collect();

        svgr!(
            <g>
                <g fill="none" stroke={RAIL} stroke-width="2.5" stroke-linecap="round">
                    <line x1="150" y1={RAIL_Y} x2={END_X} y2={RAIL_Y} />
                    <path d={branch.path()} />
                    <path d={back.path()} />
                </g>
                <line x1="60" y1="790" x2="1860" y2="790" stroke={FAINT} stroke-width="2" />

                // you
                {label(70.0, 240.0, "you", "")}
                <rect x="70" y="268" width="80" height="64" rx="14" fill={PANEL} stroke={EDGE} stroke-width="2" />
                <path d="M92 288 L106 300 L92 312" fill="none" stroke={INK} stroke-width="3" stroke-linecap="round"
                    stroke-linejoin="round" />
                {text(70.0, 362.0, 18.0, DIM, "Op::UserInput")}

                // history
                {label(320.0, 128.0, "history", "· every item so far")}
                {machine(HISTORY, 240.0, 300.0)}
                {text(680.0, 282.0, 18.0, DIM, "prompt")}
                {text(584.0, 336.0, 18.0, DIM, "instructions, tools")}
                {text(584.0, 360.0, 18.0, DIM, "the whole history")}

                // model
                {label(800.0, 128.0, "model", "· one streamed response")}
                {machine(MODEL, 260.0, 220.0)}

                // tools
                {text(1196.0, 508.0, 18.0, DIM, "tool call")}
                {label(1300.0, 412.0, "tools", "· start while it still streams")}
                {machine(TOOLS, 420.0, 170.0)}
                {stage_names}

                // what happens around a request
                <text x="60" y="832" font-size="20" font-weight="600" fill={DIM} letter-spacing="2.5">
                    "BETWEEN REQUESTS"
                </text>
                {steps(928.0, "auto-compact", "· near the limit", &["tokens ≥ limit", "summarize history", "keep going"], 0.0)}
                {steps(976.0, "stream drop", "· network", &["backoff", "retry the request", "up to stream_max_retries"], 0.0)}
            </g>
        )
    }

    /// The press inside the model: two plates that clamp the prompt while the response streams.
    fn press(&self, t: f32) -> Svgr<'static> {
        let sampling = self
            .samplings
            .iter()
            .rev()
            .find(|sampling| sampling.starts <= t);
        let (closed, lights, hit) = sampling.map_or((0.0, 0, f32::MAX), |sampling| {
            let closed = sprung(&self.clamp, t - sampling.clamps)
                - sprung(&self.clamp, t - sampling.completes);
            let lights = if t < sampling.completes + 0.5 {
                sampling.lights.iter().filter(|light| **light <= t).count()
            } else {
                0
            };
            (closed, lights, t - sampling.clamps - 0.08)
        });
        let (x, y) = MODEL;
        let plates: Vec<_> = [-1.0_f32, 1.0]
            .into_iter()
            .map(|side| {
                let mount = y + side * 100.0;
                let plate = y + side * (88.0 - 67.0 * closed);
                let leds: Vec<_> = (0..6)
                    .map(|led| {
                        svgr!(<rect x={x - 35.0 + led as f32 * 12.0} y={mount - 1.5} width="8" height="3" rx="1.5"
                            fill={if led < lights { AMBER } else { PAPER }} />)
                    })
                    .collect();
                svgr!(
                    <g>
                        <line x1={x} y1={mount} x2={x} y2={plate} stroke={DIM} stroke-width="5" />
                        <rect x={x - 42.0} y={mount - 5.0} width="84" height="10" rx="3" fill={EDGE} />
                        {leds}
                        <rect x={x - 36.0} y={plate - 4.5} width="72" height="9" rx="3" fill={STEEL} />
                    </g>
                )
            })
            .collect();
        // Short lines fly off both sides when the plates hit the card.
        let spread = ramp(hit, 0.0, 0.4);
        let sparks: Vec<_> = if (0.0..0.4).contains(&hit) {
            [-1.0_f32, 1.0]
                .into_iter()
                .flat_map(|side| {
                    [-9.0_f32, 0.0, 9.0].into_iter().map(move |dy| {
                        let from = x + side * (34.0 + 22.0 * spread);
                        svgr!(<line x1={from} y1={y + dy} x2={from + side * 12.0} y2={y + dy} stroke={STEEL}
                            stroke-width="2.5" stroke-linecap="round" opacity={1.0 - spread} />)
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        svgr!(
            <g>
                <rect x={x - 27.0} y={y - 20.0} width="54" height="40" rx="8" fill={AMBER}
                    opacity={0.22 * closed.clamp(0.0, 1.0)} />
                {plates}
                {sparks}
            </g>
        )
    }

    /// A slot of the tools machine: a ring that fills while a call stays in it.
    fn slot(&self, t: f32, lane: usize, stage: usize) -> Svgr<'static> {
        let (x, y) = (SLOTS_X[stage], LANES_Y[lane]);
        let stay = self
            .stays
            .iter()
            .rev()
            .find(|stay| stay.lane == lane && stay.stage == stage && stay.starts <= t);
        let ring = stay.filter(|stay| t < stay.ends + 0.45).map_or(Svgr::empty(), |stay| {
            let progress = ((t - stay.starts) / (stay.ends - stay.starts)).min(1.0);
            let stroke = match progress {
                p if p < 1.0 => AMBER,
                _ if stay.ok => GREEN,
                _ => RED,
            };
            // Full, it pops; then it unwinds while the card leans out.
            let unwound = ramp(t, stay.ends + 0.1, stay.ends + 0.45) * 360.0;
            let radius = 24.0 * (1.0 + 0.14 * rings(&self.pop, t - stay.ends));
            svgr!(<path d={arc(x, y, radius, unwound, progress * 360.0)} fill="none" stroke={stroke} stroke-width="3.5"
                stroke-linecap="round" />)
        });
        svgr!(
            <g>
                <circle cx={x} cy={y} r="24" fill={PAPER} stroke={RAIL} stroke-width="2" />
                {ring}
            </g>
        )
    }

    /// A gate: two halves that slide apart for a card.
    fn gate(&self, gate: &Gate, t: f32, lit: f32) -> Svgr<'static> {
        let open = gate
            .opens
            .iter()
            .map(|opens| window(t, opens - 0.16, opens + 0.1))
            .fold(0.0, f32::max);
        let offset = 11.0 + 13.0 * open;
        let (x, y) = gate.at;
        let halves: Vec<_> = [-1.0_f32, 1.0]
            .into_iter()
            .map(|side| {
                let (cx, cy, w, h) =
                    if gate.flat { (x + side * offset, y, 20.0, 9.0) } else { (x, y + side * offset, 9.0, 20.0) };
                svgr!(
                    <g>
                        <rect x={cx - w / 2.0} y={cy - h / 2.0} width={w} height={h} rx="3.5" fill={PAPER} stroke={DIM}
                            stroke-width="1.5" />
                        <rect x={cx - w / 2.0} y={cy - h / 2.0} width={w} height={h} rx="3.5" fill={AMBER} opacity={lit} />
                    </g>
                )
            })
            .collect();
        svgr!(<g>{halves}</g>)
    }

    /// A card with its smear, its lean and the text it carries.
    fn card(&self, card: &Card, t: f32) -> Svgr<'static> {
        let Some((x, y)) = self.card_at(card, t) else {
            return Svgr::empty();
        };
        let born = card.legs[0].departs;
        let shown = ramp(t, born, born + 0.1) * (1.0 - ramp(t, card.dies, card.dies + 0.12));
        let fill = card
            .fills
            .iter()
            .rev()
            .find(|fill| fill.0 <= t)
            .map_or(INK, |fill| fill.1);
        let (width, height) = (card.width, card.width * 0.64);
        // It leans as it leaves a slot and straightens on the way.
        let lean: f32 = card
            .tilts
            .iter()
            .map(|tilt| 38.0 * ramp(t, tilt - 0.22, *tilt) * (1.0 - ramp(t, *tilt, tilt + 0.3)))
            .sum();
        let smear: Vec<_> = (1..=10)
            .filter_map(|ghost| {
                let (ghost_x, ghost_y) = self.card_at(card, t - ghost as f32 * 0.006)?;
                Some(svgr!(<rect x={ghost_x - width / 2.0} y={ghost_y - height / 2.0} width={width} height={height}
                    rx="4" fill={fill} opacity={0.16 * (1.0 - ghost as f32 / 11.0)} />))
            })
            .collect();
        let badge = if card.badge {
            svgr!(<circle cx={width / 2.0 - 1.0} cy={1.0 - height / 2.0} r="4" fill={GREEN} stroke={PAPER} stroke-width="1.5" />)
        } else {
            Svgr::empty()
        };
        let tag = card.tag.filter(|(_, from, to)| t >= *from && t <= to + 0.2).map_or(Svgr::empty(), |(tag, from, to)| {
            svgr!(<text x={x} y={y - 20.0} font-size="18" font-weight="600" fill={fill} text-anchor="middle"
                opacity={window(t, from, to)}>
                {tag}
            </text>)
        });
        svgr!(
            <g opacity={shown}>
                {smear}
                <g transform={format!("translate({x:.2} {y:.2}) rotate({lean:.2})")}>
                    <rect x={-width / 2.0} y={-height / 2.0} width={width} height={height} rx="4" fill={fill} />
                    {badge}
                </g>
                {tag}
            </g>
        )
    }
}

impl Video for CodexLoopVideo<'_> {
    const FPS: usize = 60;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::WHITE;

    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(LOOP)
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.index as f32 / Self::FPS as f32;
        // What the turn drew so far: leaves before the loop starts over.
        let live = 1.0 - ramp(t, RESET, RESET + 0.5);
        let request = ((t - FIRST_REQUEST) / PERIOD)
            .floor()
            .clamp(0.0, (REQUESTS - 1) as f32) as usize;
        let streaming = self
            .samplings
            .iter()
            .map(|sampling| window(t, sampling.clamps, sampling.completes))
            .fold(0.0, f32::max);
        let done = sprung(&self.pop, t - DONE_AT) * live;
        let follow_up = self
            .follow_ups
            .iter()
            .map(|(from, to)| window(t, *from, *to))
            .fold(0.0, f32::max);
        let queued = window(t, self.queued.0, self.queued.1);

        // history: one bar per response item
        let rows: Vec<_> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.at <= t)
            .map(|(index, row)| {
                let (width, fill) = match row.kind {
                    RowKind::User => (110.0, AMBER),
                    RowKind::Reasoning => (76.0, EDGE),
                    RowKind::Call => (140.0, INK),
                    RowKind::Output(true) => (100.0, GREEN),
                    RowKind::Output(false) => (160.0, RED),
                    RowKind::Message => (190.0, INK),
                };
                let width = (width * sprung(&self.pop, t - row.at) * live).max(0.5);
                svgr!(<rect x="345" y={166.0 + index as f32 * 14.5} width={width} height="7" rx="3.5" fill={fill}
                    opacity={live} />)
            })
            .collect();
        let recorded: f32 = self
            .rows
            .iter()
            .map(|row| ramp(t, row.at, row.at + 0.3))
            .sum::<f32>()
            * live;
        let context = 0.5 + 300.0 * 0.7 * recorded / self.rows.len() as f32;

        // events to the UI
        let dots: Vec<_> = self
            .dots
            .iter()
            .enumerate()
            .filter(|(_, dot)| dot.at <= t)
            .map(|(index, dot)| {
                let x = DOTS_X + index as f32 * DOTS_STEP;
                let grown = (sprung(&self.pop, t - dot.at) * live).max(0.001);
                match dot.kind {
                    DotKind::Reasoning => svgr!(<circle cx={x} cy={RAIL_Y} r={6.0 * grown} fill={EDGE} />),
                    DotKind::Message => svgr!(<circle cx={x} cy={RAIL_Y} r={10.0 * grown} fill={INK} />),
                    DotKind::Call { ends, ok } => {
                        let fill = if ok { GREEN } else { RED };
                        svgr!(
                            <g>
                                <circle cx={x} cy={RAIL_Y} r={11.0 * grown} fill={PAPER}
                                    stroke={if t >= ends { fill } else { AMBER }} stroke-width="2.5" />
                                <circle cx={x} cy={RAIL_Y} r={(6.0 * sprung(&self.pop, t - ends) * grown).max(0.001)}
                                    fill={fill} />
                            </g>
                        )
                    }
                }
            })
            .collect();
        let ripple = ramp(t, DONE_AT, DONE_AT + 0.8);

        let slots: Vec<_> = (0..LANES_Y.len())
            .flat_map(|lane| (0..SLOTS_X.len()).map(move |stage| self.slot(t, lane, stage)))
            .collect();
        let gates: Vec<_> = self
            .gates
            .iter()
            .enumerate()
            .map(|(index, gate)| {
                let lit = if index == GATE_MODEL_IN || index == GATE_MODEL_OUT {
                    streaming
                } else {
                    0.0
                };
                self.gate(gate, t, lit)
            })
            .collect();
        let cards: Vec<_> = self
            .cards
            .iter()
            .filter(|card| t >= card.legs[0].departs && t <= card.dies + 0.15)
            .map(|card| self.card(card, t))
            .collect();

        let request_note = if t < FIRST_REQUEST || live < 0.5 {
            Svgr::empty()
        } else {
            let since = t - request_start(request);
            svgr!(<text x="800" y={156.0 + 8.0 * (1.0 - ramp(since, 0.0, 0.2))} font-size="20" font-weight="600" fill={AMBER}
                opacity={ramp(since, 0.0, 0.2)}>
                {format!("sampling request {}", request + 1)}
            </text>)
        };
        let caret = if (t * 2.0).fract() < 0.5 { 1.0 } else { 0.0 };
        let clock = arc(1836.0, 62.0, 12.0, 0.0, t / LOOP * 360.0);

        svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" viewBox={format!("0 0 {WIDTH} {HEIGHT}")} width={WIDTH} height={HEIGHT}>
                <rect width={WIDTH} height={HEIGHT} fill={PAPER} />
                <g font-family={MONO}>
                    <g font-size="20" letter-spacing="2.5">
                        <text x="60" y="70" font-weight="600" fill={GREEN}>"LIVE"</text>
                        <text x="134" y="70" font-weight="400" fill={DIM}>"· CODEX · ONE TURN OF THE SAMPLING LOOP"</text>
                    </g>
                    <text x="1806" y="69" font-size="18" font-weight="400" fill={DIM} text-anchor="end">
                        "run_turn · core/src/session/turn.rs"
                    </text>
                    <circle cx="1836" cy="62" r="12" fill="none" stroke={RAIL} stroke-width="2" />
                    <path d={clock} fill="none" stroke={DIM} stroke-width="2" stroke-linecap="round" />

                    {self.backdrop()}
                    <rect x="110" y="309" width="14" height="3" fill={INK} opacity={caret} />

                    // history
                    {rows}

                    // model
                    {request_note}
                    {self.press(t)}
                    {note(&self.model_notes, t.min(RESET), MODEL.0, 446.0, 20.0, "middle")}

                    // events
                    <text x="1230" y="262" font-size="22" font-weight="600" fill={INK}>"events"</text>
                    {note(&self.event_notes, if live < 0.5 { 0.0 } else { t }, 1322.0, 262.0, 22.0, "start")}
                    {dots}
                    <circle cx={END_X} cy={RAIL_Y} r={14.0 + 50.0 * ripple} fill="none" stroke={GREEN} stroke-width="2.5"
                        opacity={if t >= DONE_AT { 1.0 - ripple } else { 0.0 }} />
                    <circle cx={END_X} cy={RAIL_Y} r="14" fill={PAPER} stroke={DIM} stroke-width="2" />
                    <circle cx={END_X} cy={RAIL_Y} r={(14.0 * done).max(0.001)} fill={GREEN} />
                    <text x={END_X} y="346" font-size="18" font-weight="400" fill={if done > 0.5 { GREEN } else { DIM }}
                        text-anchor="end">
                        "no tool calls left → turn complete"
                    </text>

                    // tools
                    {slots}
                    {note(&self.tool_notes, if live < 0.5 { 0.0 } else { t }, TOOLS.0, 674.0, 20.0, "middle")}

                    // back into the history
                    <g font-size="20" opacity={0.75 + 0.25 * follow_up}>
                        <text x="480" y="756" font-weight="600" fill={if follow_up > 0.5 { AMBER } else { INK }}>
                            "needs_follow_up"
                        </text>
                        <text x="672" y="756" font-weight="400" fill={if follow_up > 0.5 { AMBER } else { DIM }}>
                            "· the output joins the history, then sample again"
                        </text>
                    </g>

                    {cards}
                    {gates}

                    // between requests
                    {steps(880.0, "pending input", "· typed mid-turn", &["input queue", "into the history", "the next request reads it"],
                        queued)}
                    <rect x="1330" y="924" width="300" height="8" rx="4" fill={FAINT} />
                    <rect x="1330" y="924" width={context} height="8" rx="4" fill={DIM} />
                    <rect x="1600" y="916" width="2" height="24" fill={AMBER} />
                    <text x="1650" y="934" font-size="18" font-weight="400" fill={DIM}>"context"</text>
                </g>
            </svg>
        )
    }
}
