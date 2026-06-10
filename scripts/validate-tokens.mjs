#!/usr/bin/env node
/**
 * validate-tokens.mjs — W3C DTCG validation for tokens/tokens.json
 *
 * Checks (per the Design Tokens Community Group draft,
 * https://design-tokens.github.io/community-group/format/):
 *   1. every token (node with $value) has a $type — own or inherited from
 *      an ancestor group
 *   2. $type is one of the spec's type enum
 *   3. no node mixes $value with child tokens/groups
 *   4. names contain no `.`, `{`, `}` and don't start with `$`
 *   5. $extensions is an object whose keys are reverse-DNS identifiers
 *   6. value shapes per type:
 *        color      — hex string #RGB/#RRGGBB/#RRGGBBAA (or alias)
 *        dimension  — "<number>px|rem|em" string (or alias).
 *                     NOTE deliberate deviations from the newest draft:
 *                     string form instead of {value, unit} (style-dictionary
 *                     4.x cannot build the object form), and `em` allowed for
 *                     letter-spacing (tracking must scale with font size).
 *        fontFamily — string or array of strings
 *        fontWeight — number 1..1000 or spec keyword
 *        number     — finite number
 *   7. alias values "{path.to.token}" resolve to an existing token
 *
 * Run: npm run validate:tokens  (also runs automatically before `npm run tokens`)
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { resolve, dirname } from "node:path";

const file = resolve(dirname(fileURLToPath(import.meta.url)), "../tokens/tokens.json");

const TYPES = new Set([
  "color", "dimension", "fontFamily", "fontWeight", "duration", "cubicBezier",
  "number", "strokeStyle", "border", "transition", "shadow", "gradient", "typography",
]);
const FONT_WEIGHT_KEYWORDS = new Set([
  "thin", "hairline", "extra-light", "ultra-light", "light", "normal", "regular",
  "book", "medium", "semi-bold", "demi-bold", "bold", "extra-bold", "ultra-bold",
  "black", "heavy", "extra-black", "ultra-black",
]);
const HEX_RE = /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;
const DIMENSION_RE = /^-?(?:\d+\.?\d*|\.\d+)(px|rem|em)$/;
const ALIAS_RE = /^\{[^{}]+\}$/;
const REVERSE_DNS_RE = /^[a-zA-Z0-9-]+(\.[a-zA-Z0-9-]+)+$/;

const errors = [];
const aliases = []; // [path, target]
const tokenPaths = new Set();
let tokenCount = 0;

let root;
try {
  root = JSON.parse(readFileSync(file, "utf8"));
} catch (e) {
  console.error(`tokens.json is not valid JSON: ${e.message}`);
  process.exit(1);
}

function isAlias(v) {
  return typeof v === "string" && ALIAS_RE.test(v);
}

function checkValueShape(path, type, value) {
  if (isAlias(value)) {
    aliases.push([path, value.slice(1, -1)]);
    return;
  }
  switch (type) {
    case "color":
      if (typeof value !== "string" || !HEX_RE.test(value))
        errors.push(`${path}: color $value must be a hex string (#RGB/#RRGGBB/#RRGGBBAA), got ${JSON.stringify(value)}`);
      break;
    case "dimension":
      if (typeof value !== "string" || !DIMENSION_RE.test(value))
        errors.push(`${path}: dimension $value must be "<number>px|rem|em", got ${JSON.stringify(value)}`);
      break;
    case "fontFamily":
      if (typeof value !== "string" && !(Array.isArray(value) && value.length > 0 && value.every((v) => typeof v === "string")))
        errors.push(`${path}: fontFamily $value must be a string or non-empty array of strings`);
      break;
    case "fontWeight":
      if (typeof value === "number") {
        if (!(value >= 1 && value <= 1000)) errors.push(`${path}: fontWeight number must be 1..1000, got ${value}`);
      } else if (typeof value === "string") {
        if (!FONT_WEIGHT_KEYWORDS.has(value)) errors.push(`${path}: unknown fontWeight keyword ${JSON.stringify(value)}`);
      } else errors.push(`${path}: fontWeight $value must be a number or keyword string`);
      break;
    case "number":
      if (typeof value !== "number" || !Number.isFinite(value))
        errors.push(`${path}: number $value must be a finite number, got ${JSON.stringify(value)}`);
      break;
    default:
      // composite types (border, shadow, typography, ...) — not used here;
      // only assert it's an object/array as a light sanity check
      if (value === null || (typeof value !== "object" && typeof value !== "string"))
        errors.push(`${path}: ${type} $value has unexpected shape ${JSON.stringify(value)}`);
  }
}

function checkExtensions(path, ext) {
  if (typeof ext !== "object" || ext === null || Array.isArray(ext)) {
    errors.push(`${path}: $extensions must be an object`);
    return;
  }
  for (const key of Object.keys(ext))
    if (!REVERSE_DNS_RE.test(key))
      errors.push(`${path}: $extensions key "${key}" is not reverse-DNS (e.g. "app.cenno.mark")`);
}

function walk(node, path, inheritedType) {
  if (typeof node !== "object" || node === null || Array.isArray(node)) {
    errors.push(`${path || "(root)"}: must be an object`);
    return;
  }
  const ownType = node.$type;
  if (ownType !== undefined && (typeof ownType !== "string" || !TYPES.has(ownType)))
    errors.push(`${path || "(root)"}: $type ${JSON.stringify(ownType)} is not a DTCG type`);
  const effectiveType = typeof ownType === "string" && TYPES.has(ownType) ? ownType : inheritedType;

  if (node.$description !== undefined && typeof node.$description !== "string")
    errors.push(`${path || "(root)"}: $description must be a string`);
  if (node.$extensions !== undefined) checkExtensions(path || "(root)", node.$extensions);

  const childNames = Object.keys(node).filter((k) => !k.startsWith("$"));
  const isToken = "$value" in node;

  if (isToken) {
    tokenCount += 1;
    tokenPaths.add(path);
    if (childNames.length > 0)
      errors.push(`${path}: token has both $value and children (${childNames.join(", ")})`);
    if (!effectiveType)
      errors.push(`${path}: token has no $type (own or inherited from an ancestor group)`);
    else checkValueShape(path, effectiveType, node.$value);
    return;
  }

  for (const name of childNames) {
    if (name.includes(".") || name.includes("{") || name.includes("}"))
      errors.push(`${path ? path + "." : ""}${name}: name must not contain ".", "{" or "}"`);
    walk(node[name], path ? `${path}.${name}` : name, effectiveType);
  }
}

walk(root, "", undefined);

for (const [path, target] of aliases)
  if (!tokenPaths.has(target)) errors.push(`${path}: alias {${target}} does not resolve to a token`);

if (errors.length > 0) {
  console.error(`tokens.json: ${errors.length} problem(s)\n` + errors.map((e) => `  ✗ ${e}`).join("\n"));
  process.exit(1);
}
console.log(`tokens.json: valid DTCG — ${tokenCount} tokens, 0 problems`);
