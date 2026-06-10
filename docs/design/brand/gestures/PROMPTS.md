# Gesture pictogram prompts

Generated with GPT Image 2 (`gpt_image_2.py`), quality `high`, size 1024x1024,
then quantized to 8 colors (`magick -colors 8 -strip`) for tiny file size.
Subjects from Bruno Munari, *Speak Italian: The Fine Art of the Gesture*.

## che-vuoi.png (seed 61)

> Minimalist pictogram in the style of Gerd Arntz and Isotype of the Italian pinched-fingers emoji gesture 🤌 exactly as in the emoji: a hand pointing upward with all five fingertips pressed together at a single pinch point at the top, the fingers fanning apart below the pinch so their separate lines are visible converging upward into the point, like a bird's closed beak pointing up or an upside-down cone. The short thumb is on one side pressing against the four longer fingers. The wrist and forearm rise diagonally from the bottom-right corner. Bold flat geometric reduction with a woodcut-print feel, drawn entirely in uniform-width black monoline strokes on a pure white background. No shading, no gradients, no gray, no text, no border. Square icon composition, generous empty margins, as few lines as possible so it reads instantly at 200 pixels.

## niente.png (seed 83, re-rolled)

> Minimalist pictogram in the style of Gerd Arntz and Isotype: the Italian 'niente' (nothing, no good) hand gesture from Bruno Munari's gesture dictionary. The hand is a 3/4 front view caught in mid-wrist-rotation. GEOMETRY: index finger — a long straight horizontal stroke extending far left from the knuckle. Thumb — a long curved stroke rising diagonally up and slightly back (upper-right direction), same length as the index, meeting the palm at the base of the index knuckle — the two fingers create a wide open Y or L with a large triangular gap of white space between them; the white gap is clearly visible and wide. The palm — a compact rounded block. Curled fingers — three small rounded arch shapes (knuckles) stacked on the right side of the palm, each arch fully visible and separated. Forearm — two parallel lines descending from the palm bottom. The thumb base attaches at the left side of the palm, clearly away from the back-of-hand outline. MONOLINE uniform-width black strokes only, pure white background, no fill, no shading, no gray, no text. Square composition, generous white margins.

Post-processing: strokes came out thinner than che-vuoi's, so they were
thickened with `magick -colorspace Gray -morphology Erode Disk:4` before the
8-color quantize.

## Iteration notes

- che-vuoi: 2 refinements. Draft 1 and 2 (generic "fingertips pinched together"
  wording) drifted into praying/namaste hands with straight fingers. Anchoring
  on the 🤌 emoji plus "fan of finger lines converging to a single apex point"
  fixed the pinch.
- niente: 1 refinement. Draft 1 read as a flat ASL-"L" with a paddle-like
  thumb; adding "thumb is slender and finger-like, not a flat paddle" and
  "curled fingers shown as compact rounded knuckles" cleaned it up.
- niente re-roll (2026-06-10): the seed-52 version read as a finger-gun — the
  thumb's edge merged straight into the back-of-hand contour at a tight 90°.
  Fix: spell out the geometry stroke by stroke ("fully separated", "wide
  triangular gap of white space", "thumb must NOT touch or merge with the
  back of the hand outline"). Draft 1 (seed 71) separated the thumb but left
  it a stub inside a fist-like box; draft 2 (seed 83, low) nailed the wide
  open Y with visible knuckle arcs; the high-quality run on the same seed
  kept the geometry and is the shipped version.
