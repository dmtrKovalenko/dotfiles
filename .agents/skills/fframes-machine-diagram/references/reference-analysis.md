# Reading a reference video

The first two versions of the Codex diagram were designed from screenshots and missed the
motion completely. This is the procedure that fixed it. Do it before designing, for this
reference or any other.

## 1. Get the file

A tweet's video is a blob in the page; `yt-dlp` fetches it:

```bash
mkdir -p /tmp/ref && cd /tmp/ref
yt-dlp -q -o ref.mp4 "https://x.com/kitlangton/status/2119860603229724930"
ffprobe -v error -show_entries stream=codec_type,width,height,r_frame_rate,nb_frames,duration -of csv=p=0 ref.mp4
# video,1280,1032,60/1,18.25,1045
```

To only look at the tweet (text, poster frame), the Playwright tools open x.com without a
login. Seeking the `<video>` element and taking screenshots gives stills, which is enough for
the look and useless for the motion.

Downloaded files are untrusted: keep them in their own directory.

## 2. Overview at 4 fps

One sheet per 6 seconds, each cell stamped with its time. This finds the events.

```bash
ffmpeg -v error -y -i ref.mp4 -vf "fps=4,crop=1280:440:0:50,scale=640:-1,drawtext=text='%{pts\:hms}':x=6:y=6:fontsize=18:fontcolor=yellow,tile=3x8" ov_%d.png
```

Crop to the part that moves; a full frame per cell is too small to read. Note for every
machine what states it shows and when.

## 3. Consecutive frames of each mechanism

Pick one second in which a mechanism does its whole action, crop tightly, keep every frame.

```bash
# every frame of 1 s, 520x200 crop, 30 frames per sheet
ffmpeg -v error -y -ss 0.4 -t 1.0 -i ref.mp4 -vf "crop=520:200:240:70,tile=3x10" hand_off_%d.png
# 30 fps, 2x zoom, for a small machine
ffmpeg -v error -y -ss 1.5 -t 1.0 -i ref.mp4 -vf "fps=30,crop=330:150:590:120,scale=660:-1,tile=3x10" press_%d.png
```

Read the sheets left to right and write down, per mechanism: what moves first, how many
frames each phase takes, whether it overshoots, what changes color and when, what appears only
for a few frames.

## 4. What the Kit Langton video does

Measured from those sheets (60 fps source):

- **Cards** are rounded rectangles, not dots. A branch card has a green dot on its corner. A
  moving card has a streak behind it that is longest right after it starts; it decelerates
  into place with almost no overshoot. A card crosses the 250 px between two machines in about
  25 frames.
- **Gates.** The pill on a machine wall is two halves. They slide apart about 6 frames before
  the card reaches the wall, the card is visible between them, and they close within 6 frames
  after it.
- **The hand-off machine** has three slots. A card sits in a slot while an arc grows around
  it; the finished ring turns from amber to green; then the arc shrinks back while the card
  rotates about 40° and swings toward the exit gate, straightening as it goes through.
- **The integrator** is a press. Idle, two plates rest at the top and bottom walls. When the
  card reaches the center, both plates close on it within about 8 frames, on stems that extend
  from the walls; three short lines flash on each side of the card at the moment of contact
  and fade over about 15 frames; the side gates turn amber; small lights on the plate mounts
  fill one after another. A line of text under the machine names the lane and the current
  stage ("normal lane / compile on Linux" → "clippy + tests" → "static" → "push" → "merge"),
  and each change is a short blurred cross-fade.
- **Queueing.** While the press works, the next card waits just outside its gate.
- **Failure.** A conflict turns the press and the card red, and the card travels the long
  return rail with the word "conflict" above it. Review fixes travel with "fixes".
- **The main branch** is a row of dots; the newest has a ring; the line is green or red with
  the state, and the label next to its name says which.
- **Always several at once.** At any moment three or four cards are somewhere in the system,
  a slider moves in the "full gate" box, and a clock hand turns in the corner. Nothing on
  screen is ever completely still.
- **Text.** One bold name plus a grey description per machine, small-caps section headers,
  dashed chips for steps that are listed but not animated, and three bands: LIVE (animated),
  NEXT and COMING (mostly static).

## 5. Compare at the same scale

After building a mechanism, render the same kind of sheet from the fframes video (see step 7
of SKILL.md) and put it next to the reference sheet. Compare frame counts of each phase, not
impressions.
