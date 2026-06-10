#!/usr/bin/env python3
"""Batch-generate cenno design frames via gpt-image-2 skill CLI."""
import subprocess, sys, os
from pathlib import Path

CLI = os.environ.get("GPT_IMAGE_CLI", "gpt_image_2.py")
OUT = Path(__file__).parent

STYLE = (
    "Flat minimal macOS UI design mockup in the visual style of the Reporter app by Nicholas Felton: "
    "full-bleed solid color background filling the entire image edge to edge, one question per screen, "
    "large quiet grotesque sans-serif typography (like SF Pro), white and semi-transparent white text, "
    "no gradients, no drop shadows, no cards, no window chrome, no menu bar, no decoration, no illustration, "
    "no photographic elements, generous negative space. A precise flat UI screenshot."
)

FRAMES = [
    # (name, size, seed, prompt-body)
    ("panel-mood-checkin", "840x480", 201,
     "Solid warm coral (#FF6250) background. Tiny semi-transparent white caption 'cenno' in the upper left area, "
     "and a small semi-transparent white multiplication-sign close glyph in the upper right area — "
     "both fully visible and inset about 15% from the top edge, never touching the image border. "
     "Centered large white medium-weight question: 'How are you feeling?'. "
     "Below it a single horizontal row of five evenly spaced large tappable white words in the same prominent size as comfortable button labels: "
     "'Awful', 'Bad', 'Okay', 'Good', 'Great' — the answer words are primary content, generously spaced. "
     "Everything, including the tiny caption and close glyph, stays at least 18% away from the top and bottom edges."),

    ("panel-free-text", "840x480", 202,
     "Solid cobalt blue (#1E4FD8) background. Tiny semi-transparent white caption 'cenno' in the upper left area "
     "and a small close glyph in the upper right area, both fully visible and inset about 15% from the top edge, never touching the image border. "
     "Large white question slightly above center: 'What are you working on?'. "
     "Below it a long thin white horizontal underline (a text input line) with dim placeholder text 'type or speak' resting on it; "
     "at the right end of the underline a small thin-outlined white circle containing a minimal white microphone glyph. "
     "A small white text button 'Send' in the lower right area. Everything, including the tiny caption, close glyph and Send button, stays at least 18% away from the top and bottom edges."),

    ("panel-choice", "840x480", 103,
     "Solid cobalt blue (#1E4FD8) background. Tiny semi-transparent white caption 'cenno' top left, small close glyph top right. "
     "Large white question slightly above center: 'Where did this hour go?'. "
     "Below it a row of four generously sized pill-shaped choice chips with thin white outlines, transparent fill, comfortable vertical padding and white labels: "
     "'Deep work', 'Meetings', 'Email', 'Wandering'. Everything, including the tiny caption and close glyph, stays at least 18% away from the top and bottom edges."),

    ("panel-reminder", "840x480", 104,
     "Solid slate gray (#4A5568) background. Tiny semi-transparent white caption 'cenno' top left, small close glyph top right. "
     "Large white statement slightly above center: 'Stand up and stretch.'. "
     "Below it one horizontal row of three actions: a solid white pill button with slate-colored label 'Done', "
     "then two plain semi-transparent white text buttons 'Snooze' and 'Dismiss'. Everything, including the tiny caption and close glyph, stays at least 18% away from the top and bottom edges."),

    ("panel-expired", "840x480", 105,
     "Solid near-black ink (#14171A) background. Vast empty dark space. "
     "Centered quiet white text at 60% opacity, clearly legible: 'This moment passed.'. "
     "Below it a smaller but still clearly readable text button at 50% white opacity: 'Dismiss'. Nothing else on the screen."),

    ("fullscreen-ema-1-scale", "1440x900", 106,
     "Solid deep teal (#0E7C6B) background. Tiny dim uppercase caption top center: 'CHECK-IN — 1 OF 3'. "
     "Very large white question centered: 'How focused were you this hour?'. "
     "Below it a horizontal rating scale of seven evenly spaced thin-outlined white circles containing the numbers "
     "'1', '2', '3', '4', '5', '6', '7'. Tiny dim label 'not at all' under the leftmost circle and 'completely' under the rightmost. "
     "Exactly centered horizontally at the bottom: three small pagination dots, first dot solid white, the other two semi-transparent."),

    ("fullscreen-ema-2-choice", "1440x900", 107,
     "Solid deep teal (#0E7C6B) background. Tiny dim uppercase caption top center: 'CHECK-IN — 2 OF 3'. "
     "Very large white question centered: 'What pulled at your attention?'. "
     "Below it one row of four pill-shaped chips with thin white outlines, transparent fill and white labels: "
     "'People', 'Notifications', 'My own thoughts', 'Nothing' — generously sized chips with comfortable vertical padding. "
     "Exactly centered horizontally at the bottom: three small pagination dots, second dot solid white, first and third semi-transparent."),

    ("fullscreen-ema-3-voice", "1440x900", 108,
     "Solid deep teal (#0E7C6B) background. Tiny dim uppercase caption top center: 'CHECK-IN — 3 OF 3'. "
     "Very large white question centered: 'Say a few words about right now.'. "
     "Below it a long thin white horizontal underline (text input line) with dim placeholder 'type or speak'; "
     "at its right end a small thin-outlined white circle containing a minimal white microphone glyph. "
     "Bottom left corner tiny dim text 'ambient 38 dB' preceded by a tiny solid white dot. "
     "Exactly centered horizontally at the bottom of the screen: three small pagination dots, third dot solid white, first and second semi-transparent, "
     "in the identical bottom-center position as the other check-in screens."),

    ("fullscreen-ema-done", "1440x900", 109,
     "Solid deep teal (#0E7C6B) background. A single huge white word centered: 'Noted.'. "
     "Below it a small dim line: 'this window closes itself'. Nothing else, vast calm empty teal space."),

    ("tray-inbox", "720x960", 110,
     "Portrait orientation. Solid near-black ink (#14171A) background. A header row with small white wordmark 'cenno' on the left and tiny dim text link 'history' on the right, positioned about 15% down from the top edge. "
     "Tiny dim uppercase section label 'WAITING', then two minimal list rows separated by hairline semi-transparent divider lines: "
     "first row white text 'How are you feeling?' with right-aligned dim '2m'; "
     "second row 'Where did this hour go?' with right-aligned dim '18m'. "
     "Further down a tiny dim uppercase section label 'MISSED' with one dimmer row 'How focused were you this hour?' with right-aligned '1h'. "
     "No icons, no buttons, just quiet text rows. All content stays at least 12% away from the top and bottom edges."),

    ("tray-history", "720x960", 111,
     "Portrait orientation. Solid near-black ink (#14171A) background. A header row with small white wordmark 'cenno' on the left and tiny dim text link 'inbox' on the right, positioned about 15% down from the top edge. "
     "Tiny dim uppercase section label 'TODAY', then four minimal list rows separated by hairline semi-transparent divider lines, "
     "each with a white answer on the left and a dim right-aligned time: "
     "'Good' with '9:40', 'Deep work' with '11:05', '23 spoken words' with '13:10', 'Done' with '15:00'. "
     "No icons, just quiet text rows. All content stays at least 12% away from the top and bottom edges."),

    ("tokens-sheet", "1440x900", 112,
     "Design-token reference sheet on a solid warm off-white (#FAF8F5) background with near-black (#14171A) text, "
     "flat and typographic like a Swiss specimen page. Small title top left: 'cenno tokens'. "
     "Top area: a row of five large solid square color swatches labeled underneath in small text: "
     "'coral #FF6250', 'cobalt #1E4FD8', 'teal #0E7C6B', 'slate #4A5568', 'ink #14171A' — render these hex codes letter-perfect, exactly as written; "
     "next to them three smaller swatches labeled 'text #FFFFFF', 'text-dim 60%', 'line 40%'. "
     "Middle area: a type scale specimen, four lines of the word 'Question' at decreasing sizes labeled '44', '22', '17', '13'. "
     "Bottom area: a spacing scale of five small black bars increasing in length labeled '8 16 24 40 64', "
     "a rounded square labeled 'radius 10' and a pill shape labeled 'pill 999'."),
]

mode = sys.argv[1] if len(sys.argv) > 1 else "draft"
only = sys.argv[2].split(",") if len(sys.argv) > 2 else None

for name, size, seed, body in FRAMES:
    if only and name not in only:
        continue
    sub = "drafts" if mode == "draft" else "final"
    out = f"{OUT}/{sub}/{name}.png"
    tw, th = (int(x) for x in size.split("x"))
    api_size = "1024x1536" if th > tw else "1536x1024"
    cmd = [sys.executable, CLI, "-y", "--no-preflight", "--size", api_size,
           "--seed", str(seed), f"{STYLE} {body}", out]
    if mode == "draft":
        cmd.insert(2, "--draft")
    else:
        cmd += ["--quality", "high"]
    print(f"\n=== {name} ({mode}) ===", flush=True)
    r = subprocess.run(cmd)
    if r.returncode != 0:
        print(f"FAILED: {name}", flush=True)
        continue
    if mode != "draft":
        # center-crop to the target aspect, then resize to exact surface size
        aw, ah = (int(x) for x in api_size.split("x"))
        target_aspect = tw / th
        if aw / ah > target_aspect:
            cw, ch = int(ah * target_aspect), ah   # crop width
        else:
            cw, ch = aw, int(aw / target_aspect)   # crop height
        subprocess.run(["sips", "-c", str(ch), str(cw), out], capture_output=True)
        subprocess.run(["sips", "-z", str(th), str(tw), out], capture_output=True)
        print(f"cropped {cw}x{ch} -> {tw}x{th}", flush=True)
print("ALL DONE", flush=True)
