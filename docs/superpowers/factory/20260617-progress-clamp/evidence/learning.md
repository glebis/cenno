# Learning — validating on deserialize with `serde(from)` + schemars

**Gotcha (cost two compile iterations):** to normalize/validate a type on
deserialize via `#[serde(from = "XWire")]`:

1. You **still need `Deserialize` in the derive list** — `from` is a *modifier*
   on the generated impl, not a replacement. Removing it → `Option<X>: Deserialize`
   no longer satisfied.
2. If the type also derives `schemars::JsonSchema`, the **wire struct must derive
   `JsonSchema` too** — schemars honors the serde `from` attribute and generates
   the schema from `XWire`.

**Reusable pattern:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(from = "XWire")]
pub struct X { /* fields */ }

#[derive(Deserialize, schemars::JsonSchema)]
struct XWire { /* same fields */ }

impl From<XWire> for X { fn from(w: XWire) -> Self { /* clamp/validate */ } }
```

**Where:** `src-tauri/src/protocol.rs` (`Progress`). Applies to any future
validated-on-deserialize MCP request type in cenno.
