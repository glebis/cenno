/**
 * Shrink-to-fit for a single-line credential field.
 *
 * A key is one token: wrapped onto a second line it reads as two values, and a
 * partly-off-screen one cannot be glance-checked. CSS cannot size type to fit,
 * so the field steps its font down until the run fits, or bottoms out and
 * scrolls.
 *
 * Kept pure and separate from the component so the arithmetic can be tested
 * without a layout engine (jsdom reports every width as 0).
 */

/** Below this the value stops being readable; scrolling is the better failure. */
export const MIN_FIT_FONT_PX = 11;

/**
 * The font size that should make `needed` px of text fit in `available` px.
 *
 * Returns `base` unchanged when it already fits (the common case: a short
 * token, a widened panel) so no inline style is written at all.
 *
 * The estimate is linear — glyph advance does not scale perfectly with font
 * size, which is why the caller re-measures and nudges — but it lands within a
 * pixel or two for the system stack, close enough that one or two nudges
 * finish the job.
 */
export function fitFontSize(
  base: number,
  available: number,
  needed: number,
  floor: number = MIN_FIT_FONT_PX,
): number {
  // No layout information (first paint, a detached node, jsdom): leave it.
  if (!(base > 0) || !(available > 0) || !(needed > 0)) return base;
  if (needed <= available) return base;
  const scaled = Math.floor((base * available) / needed);
  return Math.max(floor, Math.min(base, scaled));
}
