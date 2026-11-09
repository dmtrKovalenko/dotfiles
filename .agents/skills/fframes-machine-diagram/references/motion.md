# The motion grammar

Every number here is in the template (`template/lib.rs`) and was checked on consecutive
frames. The right-hand column says what in the reference video it reproduces. Change a number
only after a strip of consecutive frames shows why.

## Springs

All motion uses `fframes::animation::SpringRuntime` (`SpringRuntime::new(mass, stiffness,
damping).solve(&seconds)` gives 0 → 1 with overshoot). There is no hand-written easing except
`smooth` (a smoothstep) for opacity ramps.

| spring | parameters | used for |
| --- | --- | --- |
| travel | `throw(seconds, 0.78)` | every card move. `throw` builds a spring that first reaches its target after `seconds`; ratio 0.78 gives a fast launch, a visible deceleration and about 2% overshoot |
| pop | `SpringRuntime::new(1.0, 260.0, 11.0)` | bars, dots, ring pops, the terminal filling. Damping ratio 0.34: two visible swings |
| clamp | `SpringRuntime::new(1.0, 420.0, 30.0)` | the press plates. Ratio 0.73: it closes in about 0.15 s and does not pass through the card |

`throw` is the important one:

```rust
fn throw(seconds: f32, damping_ratio: f32) -> SpringRuntime {
    let damped = (1.0 - damping_ratio * damping_ratio).sqrt();
    let frequency = (std::f32::consts::PI - (damped / damping_ratio).atan()) / (damped * seconds);
    SpringRuntime::new(1.0, frequency * frequency, 2.0 * damping_ratio * frequency)
}
```

It lets the schedule say "this card arrives 0.55 s after it leaves" while the motion is a real
spring. A route continues straight past its end (`Route::at` with `u > 1`, capped at 160 px of
route length), so the overshoot is real movement.

What did not work: a ratio of 0.42-0.6 on every move (version 2). Cards wobbled at every stop,
long routes drifted for two seconds, and the user called the animation bad. The reference
cards barely overshoot; their life comes from the launch speed and the smear.

## Cards

| property | value | reference |
| --- | --- | --- |
| shape | rounded rectangle, height 0.64 of the width, radius 4. Never a circle | cards are small rounded rectangles |
| smear | 10 ghosts at the positions of 6, 12, … 60 ms ago, opacity `0.16 * (1 - i/11)`, drawn under the card | a soft streak behind a moving card, gone at rest |
| badge | green dot radius 4 with a 1.5 px background-colored stroke at the top right corner, on tool calls | the small green dot on a branch card |
| appear | opacity 0 → 1 in 0.1 s at departure | cards slide out of a machine, they do not pop |
| disappear | opacity 1 → 0 in 0.12 s at `dies`, inside the machine that takes it | |
| lean | 38° clockwise, rising over the 0.22 s before it leaves a slot and falling over 0.3 s after | a card tilts out of its slot and straightens on the rail |
| color change | at a time, through `fills`; a call turns green or red 0.12 s before it leaves the last slot | the card carries its result |
| tag | 18 px weight 600 text in the card's color, 20 px above it, between two times | "conflict" and "fixes" ride with their cards |

The ghosts follow the card's real earlier positions, so the smear bends around corners.

## Gates

A gate is two halves on a machine wall, on either side of the rail.

- Closed: the halves are 11 px from the rail (a 4 px slit for the rail between two 9 x 20 px
  pills, in the reference a single pill with a seam).
- Open: 24 px from the rail.
- Timing: `window(t, crossing - 0.16, crossing + 0.1)`, where `window` rises in 0.15 s and
  falls in 0.25 s. The crossing time is computed in `fn leg` (`reach(&spring, fraction)` walks
  the spring until the card is at the wall).
- Gates are drawn above cards, so a card passes between the halves.
- The gates of a machine that is working glow amber (`lit`), as the integrator's do.

## The press

For a machine that works on one card for a while.

| part | geometry (relative to the machine center, machine 260 x 220) |
| --- | --- |
| mounts | 84 x 10 px bars at ±100 px, color of the machine edge, each with six 8 x 3 px lights 12 px apart |
| plates | 72 x 9 px, dark, resting at ±88 px, clamped at ±21 px |
| stems | 5 px lines from mount to plate, visible only when the plate has moved |
| glow | 54 x 40 px amber rectangle behind the card, opacity 0.22 while clamped |
| sparks | three 12 px lines on each side, 9 px apart vertically, from 34 px to 56 px from the center, fading over 0.4 s, starting 0.08 s after the clamp |

Sequence: the card arrives (`clamps`) → plates close with the clamp spring → sparks → lights
turn on one by one at the stage times → at `completes` the plates open with the same spring and
the lights go out 0.5 s later. `closed = sprung(clamp, t - clamps) - sprung(clamp, t - completes)`.

## Slots

For staged work, and for parallel work in lanes.

1. The card lands in the slot (travel spring).
2. An amber arc grows clockwise from the top over the whole stay (`progress * 360`), stroke
   3.5 px, round caps, radius 24.
3. At the end it is a full ring and changes to green, or red for a failed last stage, and its
   radius pops by 14% with the pop spring.
4. From 0.1 s to 0.45 s after the end the arc unwinds from its start, while the card leans and
   leaves.

An arc of 0° is drawn as 0.5° (`arc` clamps), because a zero-length arc is invalid.

## Stack and event rail

- History bars: width `target * pop.solve(t - at)`, so a new bar overshoots its length once.
  A bar is not drawn at all before its time.
- Event dots: radius times the pop spring. Reasoning 6 px grey, call 11 px ring (amber while it
  runs, then the result color, with a 6 px filled center that pops in), message 10 px ink.
- The terminal: a 14 px circle that fills green with the pop spring at the end, and one ring
  that grows from 14 to 64 px while fading over 0.8 s.

## Status lines

`fn note(notes, t, x, y, size, anchor)`: on a change the old text moves up 16 px and fades in
0.1 s, the new text starts 14 px low and at opacity 0, becomes visible between 0.08 s and
0.24 s and settles by 0.26 s. The two are never both readable. The reference blurs the text
during the swap; fframes text has no cheap blur, the slide stands in for it.

A status line has a color per entry: grey for a stage in progress, ink for an event that just
happened, amber for "still waiting on something", green or red for a result.

## Things that light up

- The label of the return rail turns amber while an output is on it.
- The row of the band below turns amber while its event is happening (the queued message).
- The context bar grows with the number of recorded items.
- The request counter above the model rises 8 px and fades in over 0.2 s at each cycle.
- The caret in the "you" box blinks at 2 Hz for the whole video, so a hold is never frozen.

## Order of drawing

Background → static backdrop (rails, boxes, static labels) → history bars → press → status
lines → event dots → slots → cards → gates → the band below. Cards are above machines and
below gates.
