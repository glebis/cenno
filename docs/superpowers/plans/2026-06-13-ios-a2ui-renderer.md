# iOS companion A2UI v0.9 renderer + landscape — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the iPhone companion to A2UI v0.9 rendering parity with the tauri macOS app — every prompt (simple or raw `a2ui` passthrough) renders through one A2UI runtime with Markdown + rich layouts — and make it work in landscape.

**Architecture:** Vendor `BBC6BAE9/a2ui-swift` (MIT, v0.9) as a pinned in-repo Swift package. A Swift port of `desugar.ts` turns simple prompts into v0.9 messages; passthrough prompts use their `a2ui` field verbatim. A `CennoRemap` pass renames the six cenno-specific *leaf* components (`Text`, `TextField`, `ChoicePicker`, `Slider`, `Scale`, `Dots`) to `Cenno*` custom typeNames so a `CennoComponentCatalog` renders them natively (Markdown, voice, chips/words, labels), while a2ui-swift's **basic catalog** handles the structural components (`Row`, `Column`, `Button`, `Image`) — its `Button` already dispatches actions and its containers render custom children. `A2UISurfaceView`'s `onAction` closure is bridged to `CloudKitRelay.submit`.

**Tech Stack:** Swift 5.9 / SwiftUI, XcodeGen (`project.yml`), `a2ui-swift` (`A2UISwiftUI`/`A2UISwiftCore`), XCTest on the iOS Simulator (iPhone 17, Xcode 26.6).

---

## Reference facts (verified against source — do not re-derive)

**a2ui-swift public API (`BBC6BAE9/a2ui-swift`, branch `main`):**
- Package products: `A2UISwiftCore`, `A2UISwiftUI` (depend on `A2UISwiftUI`). swift-tools 5.9; platforms iOS 17 / macOS 14 / watchOS 10. External dep: `swift-foundation-icu` (rev `swift-6.3.1-RELEASE`).
- `public enum A2uiMessage: Codable, Sendable { case createSurface(CreateSurfacePayload); case updateComponents(UpdateComponentsPayload); case updateDataModel(UpdateDataModelPayload); case deleteSurface(DeleteSurfacePayload) }` — decodes from JSON requiring `"version":"v0.9"`.
- `public struct RawComponent { var id; var component; var weight; var accessibility; var properties: [String: AnyCodable]; init(id:component:weight:nil:accessibility:nil:properties:[:]) }` — `init(from:)` peels fixed keys and stores **all other JSON keys in `properties`**.
- `SurfaceViewModel(catalog: Catalog)`; `func processMessages(_ messages: [A2uiMessage]) -> [Error]`; `var componentTree: ComponentNode?`; `func makeDataContext(path: String = "/") -> DataContext`; `var surface: SurfaceModel`. Global `basicCatalog` provides standard components.
- `A2UISurfaceView(viewModel: SurfaceViewModel, catalog: Catalog, scrolls: Bool = true, onAction: (@Sendable (ResolvedAction) -> Void)? = nil)`. Custom catalog attached via `.a2uiCatalog(MyCatalog())`.
- `public protocol CustomComponentCatalog { associatedtype Output: View; @ViewBuilder @MainActor func build(typeName: String, node: ComponentNode, surface: SurfaceModel) -> Output }`. Returning `EmptyView()` = "not handled".
- `ComponentNode` (`@Observable` class): `var id: String`, `var type: ComponentType` (match `if case .custom(let name) = node.type`), `var instance: RawComponent`, `var children: [ComponentNode]`, `var dataContextPath: String`, `func typedProperties<T: Decodable>(_:) throws -> T` (re-encodes `instance.properties` → decodes `T`).
- `public struct ResolvedAction: Sendable { let name: String; let sourceComponentId: String; let context: [String: AnyCodable] }`.
- Action dispatch from a custom component: `surface.dispatchAction(name:sourceComponentId:context: [String: AnyCodable])` **and** `@Environment(\.a2uiActionHandler) var handler` → `handler?(ResolvedAction(...))`. Mirror the built-in Button.
- `public enum Action: Codable { case event(name: String, context: [String: DynamicValue]?); case functionCall(FunctionCall) }` — decodes `{ "event": { "name": ..., "context": { ... } } }`.
- Data binding: `DataContext.stringBinding(for: DynamicString?) -> Binding<String>`, `doubleBinding(for:fallback:)`, `resolveDynamicValue(_:) -> AnyCodable?`, `set(_ path:value:)`.
- `AnyCodable` cases include `.string`, `.number(Double)`, `.bool`, `.array([AnyCodable])`, `.dictionary([String:AnyCodable])`, `.null`; it is `Codable`.

**cenno wire contract (from `src/a2ui/desugar.ts`, `src/PromptPanel.tsx:122-160`, `src/a2ui/desugar.test.ts`):**
- Surface id `"main"`, catalog id `"cenno:catalog/v1"`. Three messages: createSurface, updateComponents (flat list), updateDataModel at path `/`.
- Component tree: `root` Column → `col` Column → `[title, body?, <input ids>, dots?]`. `title` = Text variant `h2`; `body` = Text (omitted when `body_md == ""`).
- Per input kind (ids and props):
  - **text / voice / voice_text / unknown / missing** → `input` TextField `{ value:{path:/draft}, submitAction, voice?:true }` + `send` Button `{ variant:quiet, child:sendLabel, action:submit }` + `sendLabel` Text "Send". dataModel `{draft:""}`. `voice:true` only for `voice`/`voice_text`. `via` = `voice_text` for voice kinds else `text`.
  - **choice** → `choices` ChoicePicker `{ options:[{label,value}], value:{path:/choice}, selectAction:submit-choice via choice, variant:"words" iff flow=="mood" }`. dataModel `{choice:[]}`. No send.
  - **scale** → `scale` Scale `{ min:1, max:7, minLabel:"not at all", maxLabel:"completely", value:{path:/scale}, selectAction:submit-scale via choice }`. No send.
  - **confirm** → `actions` Row `{children:[yes,no]}` + `yes` Button `{variant:primary, child:yesLabel, action:submit-yes value:"yes" via choice}` + `no` Button `{variant:borderless, child:noLabel, action:submit-no value:"no" via choice}` + `yesLabel`/`noLabel` Text. No input.
  - **none** → no input components.
- Action shape: `{ event: { name, context: { value, via } } }` where `value` is `{path:...}` or a literal, `via` ∈ {text, choice, voice_text}. Every completing action name starts with `submit`.
- `progress` present → append `dots` Dots `{step,total}` as last child of `col`.
- Answer extraction: only `name` starting `submit`; `via` = "choice"|"voice_text" else "text"; `value` array → element 0; null/undefined → ""; else `String(value)`.

---

## File structure

```
companion/
  Vendor/a2ui-swift/                      (NEW) pinned fork of BBC6BAE9/a2ui-swift
  Sources/Shared/
    JSONValue.swift                       (NEW) minimal Codable JSON tree, Equatable
    PromptRecord.swift                    (MODIFY) decode a2ui / progress / urgency
    A2UIDesugar.swift                     (NEW) Swift port of desugar.ts → [JSONValue] messages
    CennoComponentRemap.swift             (NEW) rename leaf components → Cenno* typeNames
    A2UIAnswerBridge.swift                (NEW) ResolvedAction → PromptAnswer (pure)
    A2UIMessageBuilder.swift              (NEW) [JSONValue] → [A2uiMessage] (decode bridge)
  Sources/iPhone/
    CennoComponentCatalog.swift           (NEW) CustomComponentCatalog: 6 leaf views
    A2UIPromptView.swift                  (NEW) replaces PhonePromptDetailView body
    PhonePromptDetailView.swift           (MODIFY) delegate to A2UIPromptView
    Info.plist                            (MODIFY) landscape orientations
  Tests/CennoSharedTests/                 (NEW) XCTest target
    JSONValueTests.swift
    PromptPayloadDecodeTests.swift
    A2UIDesugarTests.swift                (mirrors src/a2ui/desugar.test.ts)
    CennoComponentRemapTests.swift
    A2UIAnswerBridgeTests.swift
  project.yml                             (MODIFY) vendor package, CennoSharedTests target, scheme test
```

---

## Task 1: Vendor a2ui-swift and wire the build

**Files:**
- Create: `companion/Vendor/a2ui-swift/` (cloned fork)
- Modify: `companion/project.yml`

- [ ] **Step 1: Clone the fork pinned to a commit**

```bash
cd /Users/glebkalinin/ai_projects/cenno/companion
mkdir -p Vendor
git clone https://github.com/BBC6BAE9/a2ui-swift.git Vendor/a2ui-swift
cd Vendor/a2ui-swift
git rev-parse HEAD > /dev/null   # record the commit below
# Pin: detach at current main HEAD so upstream can't move under us.
PIN=$(git rev-parse HEAD); echo "pinned a2ui-swift @ $PIN"
# Drop the nested .git so it vendors as plain source under cenno's repo.
trash .git
cd /Users/glebkalinin/ai_projects/cenno
```

Record the pinned commit in the commit message at Step 5.

- [ ] **Step 2: Add the local package + test target to `project.yml`**

Add a top-level `packages:` block and wire the package product into `CennoShared`, plus a new `CennoSharedTests` unit-test target. Insert after the `options:`/`settings:` blocks and edit the `CennoShared` target + `CennoiPhone` scheme:

```yaml
packages:
  A2UISwift:
    path: Vendor/a2ui-swift

# ── in target CennoShared: add the package product dependency ──
  CennoShared:
    type: framework
    platform: iOS
    sources:
      - path: Sources/Shared
    settings:
      PRODUCT_BUNDLE_IDENTIFIER: app.cenno.companion.shared
      GENERATE_INFOPLIST_FILE: YES
    dependencies:
      - sdk: CloudKit.framework
      - package: A2UISwift
        product: A2UISwiftUI

# ── new test target (place among targets:) ──
  CennoSharedTests:
    type: bundle.unit-test
    platform: iOS
    sources:
      - path: Tests/CennoSharedTests
    dependencies:
      - target: CennoShared
      - package: A2UISwift
        product: A2UISwiftUI
    settings:
      GENERATE_INFOPLIST_FILE: YES
      PRODUCT_BUNDLE_IDENTIFIER: app.cenno.companion.sharedtests
```

In the existing `schemes: CennoiPhone:` block, add a `test` targets list:

```yaml
  CennoiPhone:
    build:
      targets:
        CennoiPhone: all
        CennoShared: all
        CennoSharedTests: all
    test:
      config: Debug
      targets:
        - CennoSharedTests
    run:
      config: Debug
```

- [ ] **Step 3: Create the test source dir with a smoke test**

Create `companion/Tests/CennoSharedTests/SmokeTest.swift`:

```swift
import XCTest
@testable import CennoShared
import A2UISwiftUI

final class SmokeTest: XCTestCase {
    func testA2UISwiftLinks() {
        // Proves the vendored package compiles + links into the test target.
        let vm = SurfaceViewModel(catalog: basicCatalog)
        XCTAssertNil(vm.componentTree)
    }
}
```

- [ ] **Step 4: Regenerate the project and build the test target**

```bash
cd /Users/glebkalinin/ai_projects/cenno/companion
xcodegen generate
xcodebuild build-for-testing \
  -project CennoCompanion.xcodeproj -scheme CennoiPhone \
  -destination 'platform=iOS Simulator,name=iPhone 17'
```
Expected: `** BUILD SUCCEEDED **`. If `basicCatalog`/`SurfaceViewModel` are unresolved, fix the `product:` name against `Vendor/a2ui-swift/Package.swift` before continuing.

- [ ] **Step 5: Commit**

```bash
cd /Users/glebkalinin/ai_projects/cenno
git add companion/Vendor/a2ui-swift companion/project.yml companion/CennoCompanion.xcodeproj companion/Tests/CennoSharedTests/SmokeTest.swift
git commit -m "build(companion): vendor a2ui-swift @ <PIN> + CennoSharedTests target"
```

---

## Task 2: `JSONValue` — a minimal Codable JSON tree

**Files:**
- Create: `companion/Sources/Shared/JSONValue.swift`
- Test: `companion/Tests/CennoSharedTests/JSONValueTests.swift`

- [ ] **Step 1: Write the failing test**

```swift
import XCTest
@testable import CennoShared

final class JSONValueTests: XCTestCase {
    func testRoundTripsNestedObject() throws {
        let json = #"{"a":1,"b":["x",true,null],"c":{"d":2.5}}"#
        let value = try JSONDecoder().decode(JSONValue.self, from: Data(json.utf8))
        XCTAssertEqual(value["a"], .number(1))
        XCTAssertEqual(value["b"]?[1], .bool(true))
        XCTAssertEqual(value["b"]?[2], .null)
        XCTAssertEqual(value["c"]?["d"], .number(2.5))
    }

    func testEncodesBackToStableJSON() throws {
        let value: JSONValue = .object(["k": .string("v")])
        let data = try JSONEncoder().encode(value)
        let back = try JSONDecoder().decode(JSONValue.self, from: data)
        XCTAssertEqual(value, back)
    }

    func testSubscriptsReturnNilOffType() {
        let value: JSONValue = .string("hi")
        XCTAssertNil(value["a"])
        XCTAssertNil(value[0])
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `xcodebuild test -project companion/CennoCompanion.xcodeproj -scheme CennoiPhone -destination 'platform=iOS Simulator,name=iPhone 17' -only-testing:CennoSharedTests/JSONValueTests`
Expected: FAIL — `JSONValue` undefined.

- [ ] **Step 3: Implement**

Create `companion/Sources/Shared/JSONValue.swift`:

```swift
import Foundation

/// A minimal, Equatable JSON tree. Used to (a) decode the untyped `a2ui`
/// passthrough payload, (b) hold desugar output for assertion-friendly tests,
/// and (c) carry resolved action context across the answer bridge. Encodes to
/// the same wire shape a2ui-swift's `A2uiMessage` Codable consumes.
public enum JSONValue: Codable, Equatable, Sendable {
    case string(String)
    case number(Double)
    case bool(Bool)
    case array([JSONValue])
    case object([String: JSONValue])
    case null

    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() { self = .null; return }
        if let b = try? c.decode(Bool.self) { self = .bool(b); return }
        if let n = try? c.decode(Double.self) { self = .number(n); return }
        if let s = try? c.decode(String.self) { self = .string(s); return }
        if let a = try? c.decode([JSONValue].self) { self = .array(a); return }
        if let o = try? c.decode([String: JSONValue].self) { self = .object(o); return }
        throw DecodingError.dataCorruptedError(in: c, debugDescription: "unrecognised JSON")
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .string(let s): try c.encode(s)
        case .number(let n): try c.encode(n)
        case .bool(let b): try c.encode(b)
        case .array(let a): try c.encode(a)
        case .object(let o): try c.encode(o)
        case .null: try c.encodeNil()
        }
    }

    public subscript(key: String) -> JSONValue? {
        if case .object(let o) = self { return o[key] }
        return nil
    }
    public subscript(index: Int) -> JSONValue? {
        if case .array(let a) = self, a.indices.contains(index) { return a[index] }
        return nil
    }
    public var stringValue: String? { if case .string(let s) = self { return s }; return nil }
    public var arrayValue: [JSONValue]? { if case .array(let a) = self { return a }; return nil }
}
```

Note on `Bool` vs `Double`: decode `Bool` before `Double` so `true`/`false` don't become `1`/`0`. (JSON has no integer type; `1` decodes as `.number(1)`.)

- [ ] **Step 4: Run to verify it passes**

Run: same `-only-testing:CennoSharedTests/JSONValueTests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add companion/Sources/Shared/JSONValue.swift companion/Tests/CennoSharedTests/JSONValueTests.swift
git commit -m "feat(companion): JSONValue Codable tree"
```

---

## Task 3: Decode `a2ui` / `progress` / `urgency` in `PromptPayload`

**Files:**
- Modify: `companion/Sources/Shared/PromptRecord.swift:20-31`
- Test: `companion/Tests/CennoSharedTests/PromptPayloadDecodeTests.swift`

- [ ] **Step 1: Write the failing test**

```swift
import XCTest
@testable import CennoShared

final class PromptPayloadDecodeTests: XCTestCase {
    func testDecodesA2uiProgressUrgency() throws {
        let json = """
        {"title":"T","body_md":"**b**","input":{"kind":"scale"},
         "urgency":"high","progress":{"step":2,"total":5},
         "a2ui":[{"version":"v0.9","createSurface":{"surfaceId":"main","catalogId":"cenno:catalog/v1"}}]}
        """
        let p = try JSONDecoder().decode(PromptPayload.self, from: Data(json.utf8))
        XCTAssertEqual(p.title, "T")
        XCTAssertEqual(p.urgency, "high")
        XCTAssertEqual(p.progress?.step, 2)
        XCTAssertEqual(p.progress?.total, 5)
        XCTAssertEqual(p.a2ui?[0]?["createSurface"]?["surfaceId"], .string("main"))
    }

    func testMissingOptionalFieldsDecodeNil() throws {
        let json = #"{"title":"T","body_md":"","input":{"kind":"text"}}"#
        let p = try JSONDecoder().decode(PromptPayload.self, from: Data(json.utf8))
        XCTAssertNil(p.a2ui)
        XCTAssertNil(p.progress)
        XCTAssertNil(p.urgency)
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `xcodebuild test ... -only-testing:CennoSharedTests/PromptPayloadDecodeTests`
Expected: FAIL — `PromptPayload` has no `a2ui`/`progress`/`urgency`.

- [ ] **Step 3: Implement**

In `companion/Sources/Shared/PromptRecord.swift`, replace the `PromptPayload` struct (lines 20-31) with:

```swift
public struct PromptPayload: Codable, Sendable {
    public let title: String
    public let bodyMd: String?
    public let input: InputSpec?
    public let choices: [String]?
    public let flow: String?
    public let timeoutS: Int?
    public let urgency: String?
    public let progress: Progress?
    public let a2ui: JSONValue?

    enum CodingKeys: String, CodingKey {
        case title, bodyMd = "body_md", input, choices, flow
        case timeoutS = "timeout_s", urgency, progress, a2ui
    }
}

public struct Progress: Codable, Sendable {
    public let step: Int
    public let total: Int
}
```

- [ ] **Step 4: Run to verify it passes**

Run: same `-only-testing:CennoSharedTests/PromptPayloadDecodeTests`
Expected: PASS. (`a2ui` decodes as `JSONValue.array`; `p.a2ui?[0]?["createSurface"]` uses the subscripts from Task 2.)

- [ ] **Step 5: Commit**

```bash
git add companion/Sources/Shared/PromptRecord.swift companion/Tests/CennoSharedTests/PromptPayloadDecodeTests.swift
git commit -m "feat(companion): decode a2ui/progress/urgency in PromptPayload"
```

---

## Task 4: `A2UIDesugar` — Swift port of `desugar.ts`

**Files:**
- Create: `companion/Sources/Shared/A2UIDesugar.swift`
- Test: `companion/Tests/CennoSharedTests/A2UIDesugarTests.swift` (mirrors `src/a2ui/desugar.test.ts`)

Produces `[JSONValue]` (the three-message envelope). Component names match the TS output exactly (`Text`/`Button`/`TextField`/`ChoicePicker`/`Slider`/`Scale`/`Dots`/`Row`/`Column`); the rename to `Cenno*` happens later in Task 5.

**Custom-widget scope (intentional divergence from `desugar.ts`):** the TS `desugar(req, widgets)` expands custom `~/.cenno` widget templates, and `desugar.test.ts` has two widget tests. The iPhone has no `~/.cenno` config, so the Swift port omits the `widgets` parameter entirely — a custom `input.kind` with no built-in match falls through to the text default. The two TS widget tests are **deliberately not ported**; `testUnknownKindFallsBackToText` is the parity guarantee for the no-config path. (If agents need a custom widget rendered on iPhone, they must send it via the `a2ui` passthrough field, which already works.)

- [ ] **Step 1: Write the failing tests (mirror the TS table)**

```swift
import XCTest
@testable import CennoShared

final class A2UIDesugarTests: XCTestCase {
    private func payload(title: String = "How focused are you?",
                         bodyMd: String = "Be honest.",
                         kind: String? = "text",
                         choices: [String]? = nil,
                         flow: String? = nil,
                         progress: Progress? = nil) -> PromptPayload {
        let json = try! JSONEncoder().encode(DesugarFixture(
            title: title, body_md: bodyMd,
            input: kind.map { DesugarFixture.Input(kind: $0) },
            choices: choices, flow: flow, progress: progress))
        return try! JSONDecoder().decode(PromptPayload.self, from: json)
    }
    private struct DesugarFixture: Encodable {
        struct Input: Encodable { let kind: String }
        let title: String; let body_md: String; let input: Input?
        let choices: [String]?; let flow: String?; let progress: Progress?
    }

    /// Unpack the envelope into (create, components-by-id, col, dataModel).
    private func parts(_ p: PromptPayload)
        -> (msgs: [JSONValue], create: JSONValue?, byId: [String: JSONValue], col: JSONValue?, dataModel: JSONValue?) {
        let msgs = A2UIDesugar.messages(for: p)
        let create = msgs[0]["createSurface"]
        let comps = msgs[1]["updateComponents"]?["components"]?.arrayValue ?? []
        var byId: [String: JSONValue] = [:]
        for c in comps { if let id = c["id"]?.stringValue { byId[id] = c } }
        return (msgs, create, byId, byId["col"], msgs[2]["updateDataModel"]?["value"])
    }

    func testEnvelope() {
        let (msgs, create, _, _, _) = parts(payload())
        XCTAssertEqual(msgs.count, 3)
        XCTAssertEqual(create, .object(["surfaceId": .string("main"),
                                        "catalogId": .string("cenno:catalog/v1")]))
    }

    func testDeterministic() {
        let p = payload(kind: "choice", choices: ["a", "b"])
        XCTAssertEqual(A2UIDesugar.messages(for: p), A2UIDesugar.messages(for: p))
    }

    func testRootColTitleBody() {
        let (_, _, byId, col, _) = parts(payload())
        XCTAssertEqual(byId["root"]?["children"], .array([.string("col")]))
        XCTAssertEqual(col?["children"]?[0], .string("title"))
        XCTAssertEqual(col?["children"]?[1], .string("body"))
        XCTAssertEqual(byId["title"]?["variant"], .string("h2"))
        XCTAssertEqual(byId["title"]?["text"], .string("How focused are you?"))
        XCTAssertEqual(byId["body"]?["text"], .string("Be honest."))
    }

    func testEmptyBodyOmitsBody() {
        let (_, _, byId, col, _) = parts(payload(bodyMd: ""))
        XCTAssertNil(byId["body"])
        XCTAssertFalse(col!["children"]!.arrayValue!.contains(.string("body")))
    }

    func testTextFieldAndQuietSend() {
        let (_, _, byId, col, dm) = parts(payload(kind: "text"))
        XCTAssertEqual(col?["children"], .array(["title","body","input","send"].map(JSONValue.string)))
        let submit = JSONValue.object(["event": .object([
            "name": .string("submit"),
            "context": .object(["value": .object(["path": .string("/draft")]), "via": .string("text")])])])
        XCTAssertEqual(byId["input"]?["component"], .string("TextField"))
        XCTAssertEqual(byId["input"]?["value"], .object(["path": .string("/draft")]))
        XCTAssertEqual(byId["input"]?["submitAction"], submit)
        XCTAssertNil(byId["input"]?["voice"])
        XCTAssertEqual(byId["send"]?["variant"], .string("quiet"))
        XCTAssertEqual(byId["send"]?["action"], submit)
        XCTAssertEqual(byId["sendLabel"]?["text"], .string("Send"))
        XCTAssertEqual(dm?["draft"], .string(""))
    }

    func testVoiceKindsSetVoiceTrueAndVia() {
        for kind in ["voice", "voice_text"] {
            let (_, _, byId, _, _) = parts(payload(kind: kind))
            XCTAssertEqual(byId["input"]?["voice"], .bool(true))
            XCTAssertEqual(byId["input"]?["submitAction"]?["event"]?["context"]?["via"], .string("voice_text"))
        }
    }

    func testChoice() {
        let (_, _, byId, col, dm) = parts(payload(kind: "choice", choices: ["Calm", "Tense"]))
        XCTAssertEqual(col?["children"], .array(["title","body","choices"].map(JSONValue.string)))
        XCTAssertEqual(byId["choices"]?["options"], .array([
            .object(["label": .string("Calm"), "value": .string("Calm")]),
            .object(["label": .string("Tense"), "value": .string("Tense")])]))
        XCTAssertEqual(byId["choices"]?["selectAction"]?["event"]?["name"], .string("submit-choice"))
        XCTAssertNil(byId["send"])
        XCTAssertEqual(dm?["choice"], .array([]))
    }

    func testMoodFlowWordsVariant() {
        let (_, _, byId, _, _) = parts(payload(kind: "choice", choices: ["Bad","Good"], flow: "mood"))
        XCTAssertEqual(byId["choices"]?["variant"], .string("words"))
    }

    func testNonMoodNoWordsVariant() {
        let (_, _, byId, _, _) = parts(payload(kind: "choice", choices: ["Calm"], flow: "question"))
        XCTAssertNil(byId["choices"]?["variant"])
    }

    func testScale() {
        let (_, _, byId, col, _) = parts(payload(kind: "scale"))
        XCTAssertEqual(col?["children"], .array(["title","body","scale"].map(JSONValue.string)))
        XCTAssertEqual(byId["scale"]?["min"], .number(1))
        XCTAssertEqual(byId["scale"]?["max"], .number(7))
        XCTAssertEqual(byId["scale"]?["minLabel"], .string("not at all"))
        XCTAssertEqual(byId["scale"]?["maxLabel"], .string("completely"))
        XCTAssertEqual(byId["scale"]?["selectAction"]?["event"]?["name"], .string("submit-scale"))
        XCTAssertNil(byId["send"])
    }

    func testConfirm() {
        let (_, _, byId, col, _) = parts(payload(kind: "confirm"))
        XCTAssertEqual(col?["children"], .array(["title","body","actions"].map(JSONValue.string)))
        XCTAssertEqual(byId["actions"]?["children"], .array([.string("yes"), .string("no")]))
        XCTAssertEqual(byId["yes"]?["variant"], .string("primary"))
        XCTAssertEqual(byId["yes"]?["action"]?["event"]?["context"]?["value"], .string("yes"))
        XCTAssertEqual(byId["no"]?["variant"], .string("borderless"))
        XCTAssertEqual(byId["yesLabel"]?["text"], .string("Yes"))
        XCTAssertEqual(byId["noLabel"]?["text"], .string("No"))
        XCTAssertNil(byId["input"])
    }

    func testNone() {
        let (_, _, byId, col, _) = parts(payload(kind: "none"))
        XCTAssertEqual(col?["children"], .array([.string("title"), .string("body")]))
        for id in ["input","send","choices","scale","actions","yes","no"] { XCTAssertNil(byId[id]) }
    }

    func testProgressDotsAppended() {
        let (_, _, byId, col, _) = parts(payload(kind: "scale", progress: Progress(step: 2, total: 5)))
        XCTAssertEqual(col?["children"], .array(["title","body","scale","dots"].map(JSONValue.string)))
        XCTAssertEqual(byId["dots"]?["step"], .number(2))
        XCTAssertEqual(byId["dots"]?["total"], .number(5))
    }

    func testUnknownKindFallsBackToText() {
        let (_, _, byId, col, _) = parts(payload(kind: "hologram"))
        XCTAssertEqual(col?["children"], .array(["title","body","input","send"].map(JSONValue.string)))
        XCTAssertEqual(byId["input"]?["component"], .string("TextField"))
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `xcodebuild test ... -only-testing:CennoSharedTests/A2UIDesugarTests`
Expected: FAIL — `A2UIDesugar` undefined.

- [ ] **Step 3: Implement the port**

Create `companion/Sources/Shared/A2UIDesugar.swift`:

```swift
import Foundation

/// Pure port of src/a2ui/desugar.ts: PromptPayload → A2UI v0.9 message
/// envelope ([createSurface, updateComponents, updateDataModel]) as JSONValue.
/// Component names match the TS catalog; CennoComponentRemap renames the
/// cenno-specific leaves to Cenno* typeNames before rendering.
public enum A2UIDesugar {
    public static let surfaceID = "main"
    public static let catalogID = "cenno:catalog/v1"

    public static func messages(for req: PromptPayload) -> [JSONValue] {
        let input = desugarInput(req)
        let hasBody = (req.bodyMd ?? "") != ""

        var childIds: [JSONValue] = [.string("title")]
        if hasBody { childIds.append(.string("body")) }
        childIds.append(contentsOf: input.childIds.map(JSONValue.string))
        if req.progress != nil { childIds.append(.string("dots")) }

        var components: [JSONValue] = [
            obj(["id": .string("root"), "component": .string("Column"),
                 "children": .array([.string("col")])]),
            obj(["id": .string("col"), "component": .string("Column"),
                 "children": .array(childIds)]),
            obj(["id": .string("title"), "component": .string("Text"),
                 "variant": .string("h2"), "text": .string(req.title)]),
        ]
        if hasBody {
            components.append(obj(["id": .string("body"), "component": .string("Text"),
                                   "text": .string(req.bodyMd ?? "")]))
        }
        components.append(contentsOf: input.components)
        if let p = req.progress {
            components.append(obj(["id": .string("dots"), "component": .string("Dots"),
                                   "step": .number(Double(p.step)), "total": .number(Double(p.total))]))
        }

        return [
            obj(["version": .string("v0.9"),
                 "createSurface": obj(["surfaceId": .string(surfaceID),
                                       "catalogId": .string(catalogID)])]),
            obj(["version": .string("v0.9"),
                 "updateComponents": obj(["surfaceId": .string(surfaceID),
                                          "components": .array(components)])]),
            obj(["version": .string("v0.9"),
                 "updateDataModel": obj(["surfaceId": .string(surfaceID),
                                         "path": .string("/"), "value": input.dataModel])]),
        ]
    }

    // MARK: input-kind specific

    private struct Input { let childIds: [String]; let components: [JSONValue]; let dataModel: JSONValue }

    private static func desugarInput(_ req: PromptPayload) -> Input {
        switch req.input?.kind {
        case "choice":
            let options = (req.choices ?? []).map { c in
                JSONValue.object(["label": .string(c), "value": .string(c)]) }
            var picker: [String: JSONValue] = [
                "id": .string("choices"), "component": .string("ChoicePicker"),
                "options": .array(options), "value": binding("/choice"),
                "selectAction": action("submit-choice", binding("/choice"), "choice")]
            if req.flow == "mood" { picker["variant"] = .string("words") }
            return Input(childIds: ["choices"], components: [.object(picker)],
                         dataModel: .object(["choice": .array([])]))
        case "scale":
            return Input(childIds: ["scale"], components: [obj([
                "id": .string("scale"), "component": .string("Scale"),
                "min": .number(1), "max": .number(7),
                "minLabel": .string("not at all"), "maxLabel": .string("completely"),
                "value": binding("/scale"),
                "selectAction": action("submit-scale", binding("/scale"), "choice")])],
                dataModel: .object([:]))
        case "confirm":
            return Input(childIds: ["actions"],
                components: [obj(["id": .string("actions"), "component": .string("Row"),
                                  "children": .array([.string("yes"), .string("no")])])]
                    + button("yes", "Yes", "primary", action("submit-yes", .string("yes"), "choice"))
                    + button("no", "No", "borderless", action("submit-no", .string("no"), "choice")),
                dataModel: .object([:]))
        case "none":
            return Input(childIds: [], components: [], dataModel: .object([:]))
        default:
            let kind = req.input?.kind
            let voice = (kind == "voice" || kind == "voice_text")
            let submit = action("submit", binding("/draft"), voice ? "voice_text" : "text")
            var field: [String: JSONValue] = [
                "id": .string("input"), "component": .string("TextField"),
                "label": .string("Your reply"), "value": binding("/draft"),
                "submitAction": submit]
            if voice { field["voice"] = .bool(true) }
            return Input(childIds: ["input", "send"],
                         components: [.object(field)] + button("send", "Send", "quiet", submit),
                         dataModel: .object(["draft": .string("")]))
        }
    }

    // MARK: helpers

    private static func obj(_ d: [String: JSONValue]) -> JSONValue { .object(d) }
    private static func binding(_ path: String) -> JSONValue { .object(["path": .string(path)]) }
    private static func action(_ name: String, _ value: JSONValue, _ via: String) -> JSONValue {
        .object(["event": .object(["name": .string(name),
                                   "context": .object(["value": value, "via": .string(via)])])])
    }
    private static func button(_ id: String, _ label: String,
                               _ variant: String, _ action: JSONValue) -> [JSONValue] {
        [obj(["id": .string(id), "component": .string("Button"), "variant": .string(variant),
              "child": .string("\(id)Label"), "action": action]),
         obj(["id": .string("\(id)Label"), "component": .string("Text"), "text": .string(label)])]
    }
}
```

- [ ] **Step 4: Run to verify they pass**

Run: same `-only-testing:CennoSharedTests/A2UIDesugarTests`
Expected: PASS (all cases).

- [ ] **Step 5: Commit**

```bash
git add companion/Sources/Shared/A2UIDesugar.swift companion/Tests/CennoSharedTests/A2UIDesugarTests.swift
git commit -m "feat(companion): Swift port of A2UI desugar (mirrors desugar.test.ts)"
```

---

## Task 5: `CennoComponentRemap` — leaf components → `Cenno*` typeNames

**Files:**
- Create: `companion/Sources/Shared/CennoComponentRemap.swift`
- Test: `companion/Tests/CennoSharedTests/CennoComponentRemapTests.swift`

Renames the six cenno-specific leaf components in every `updateComponents` message so they become a2ui-swift `.custom(...)` types routed to `CennoComponentCatalog`. Structural components (`Row`, `Column`, `Button`, `Image`) keep their basic names. Applied to BOTH desugar output and `a2ui` passthrough.

- [ ] **Step 1: Write the failing test**

```swift
import XCTest
@testable import CennoShared

final class CennoComponentRemapTests: XCTestCase {
    func testRenamesLeafComponentsOnly() {
        let msgs: [JSONValue] = [.object(["updateComponents": .object(["components": .array([
            .object(["id": .string("title"), "component": .string("Text")]),
            .object(["id": .string("col"), "component": .string("Column")]),
            .object(["id": .string("yes"), "component": .string("Button")]),
            .object(["id": .string("choices"), "component": .string("ChoicePicker")]),
            .object(["id": .string("scale"), "component": .string("Scale")]),
            .object(["id": .string("dots"), "component": .string("Dots")]),
        ])])])]
        let out = CennoComponentRemap.apply(msgs)
        let comps = out[0]["updateComponents"]?["components"]?.arrayValue ?? []
        func comp(_ id: String) -> String? {
            comps.first { $0["id"]?.stringValue == id }?["component"]?.stringValue }
        XCTAssertEqual(comp("title"), "CennoText")
        XCTAssertEqual(comp("choices"), "CennoChoicePicker")
        XCTAssertEqual(comp("scale"), "CennoScale")
        XCTAssertEqual(comp("dots"), "CennoDots")
        XCTAssertEqual(comp("col"), "Column")    // structural: unchanged
        XCTAssertEqual(comp("yes"), "Button")    // structural: unchanged
    }

    func testPassesNonComponentMessagesThrough() {
        let msgs: [JSONValue] = [.object(["createSurface": .object(["surfaceId": .string("main")])])]
        XCTAssertEqual(CennoComponentRemap.apply(msgs), msgs)
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `xcodebuild test ... -only-testing:CennoSharedTests/CennoComponentRemapTests`
Expected: FAIL — `CennoComponentRemap` undefined.

- [ ] **Step 3: Implement**

Create `companion/Sources/Shared/CennoComponentRemap.swift`:

```swift
import Foundation

/// Renames cenno-specific LEAF components to Cenno* typeNames so a2ui-swift
/// routes them to CennoComponentCatalog. Structural components (Row/Column/
/// Button/Image) stay basic — a2ui-swift's Button dispatches actions and its
/// containers render custom children natively.
public enum CennoComponentRemap {
    static let leaves: Set<String> = ["Text", "TextField", "ChoicePicker", "Slider", "Scale", "Dots"]

    public static func apply(_ messages: [JSONValue]) -> [JSONValue] {
        messages.map { msg in
            guard case .object(var top) = msg,
                  let uc = top["updateComponents"], case .object(var ucObj) = uc,
                  let comps = ucObj["components"]?.arrayValue else { return msg }
            ucObj["components"] = .array(comps.map(renameComponent))
            top["updateComponents"] = .object(ucObj)
            return .object(top)
        }
    }

    private static func renameComponent(_ comp: JSONValue) -> JSONValue {
        guard case .object(var o) = comp,
              let name = o["component"]?.stringValue, leaves.contains(name) else { return comp }
        o["component"] = .string("Cenno" + name)
        return .object(o)
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: same `-only-testing:CennoSharedTests/CennoComponentRemapTests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add companion/Sources/Shared/CennoComponentRemap.swift companion/Tests/CennoSharedTests/CennoComponentRemapTests.swift
git commit -m "feat(companion): remap cenno leaf components to Cenno* typeNames"
```

---

## Task 6: `A2UIAnswerBridge` — resolved action → `PromptAnswer`

**Files:**
- Create: `companion/Sources/Shared/A2UIAnswerBridge.swift`
- Test: `companion/Tests/CennoSharedTests/A2UIAnswerBridgeTests.swift`

Pure port of the answer-extraction logic in `src/PromptPanel.tsx:122-160`. Operates on `[String: JSONValue]` (the resolved action context); the view converts a2ui-swift's `ResolvedAction.context` (`[String: AnyCodable]`) into `[String: JSONValue]` by encoding/decoding (both are Codable).

- [ ] **Step 1: Write the failing test**

```swift
import XCTest
@testable import CennoShared

final class A2UIAnswerBridgeTests: XCTestCase {
    func testIgnoresNonSubmitActions() {
        XCTAssertNil(A2UIAnswerBridge.answer(name: "focus", context: [:], elapsedS: 1, device: "iphone"))
    }
    func testTextAnswerViaText() {
        let a = A2UIAnswerBridge.answer(name: "submit",
            context: ["value": .string("hello"), "via": .string("text")],
            elapsedS: 2.5, device: "iphone")
        XCTAssertEqual(a?.answer, "hello"); XCTAssertEqual(a?.via, "text")
        XCTAssertEqual(a?.elapsedS, 2.5); XCTAssertEqual(a?.device, "iphone")
    }
    func testChoiceUnwrapsArray() {
        let a = A2UIAnswerBridge.answer(name: "submit-choice",
            context: ["value": .array([.string("Calm")]), "via": .string("choice")],
            elapsedS: 0, device: "iphone")
        XCTAssertEqual(a?.answer, "Calm"); XCTAssertEqual(a?.via, "choice")
    }
    func testScaleStringifiesNumber() {
        let a = A2UIAnswerBridge.answer(name: "submit-scale",
            context: ["value": .number(4), "via": .string("choice")], elapsedS: 0, device: "iphone")
        XCTAssertEqual(a?.answer, "4")
    }
    func testNullValueBecomesEmptyAck() {
        let a = A2UIAnswerBridge.answer(name: "submit",
            context: ["value": .null, "via": .string("text")], elapsedS: 0, device: "iphone")
        XCTAssertEqual(a?.answer, "")
    }
    func testUnknownViaDefaultsToText() {
        let a = A2UIAnswerBridge.answer(name: "submit-yes",
            context: ["value": .string("yes"), "via": .string("weird")], elapsedS: 0, device: "iphone")
        XCTAssertEqual(a?.via, "text")
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `xcodebuild test ... -only-testing:CennoSharedTests/A2UIAnswerBridgeTests`
Expected: FAIL — `A2UIAnswerBridge` undefined.

- [ ] **Step 3: Implement**

Create `companion/Sources/Shared/A2UIAnswerBridge.swift`:

```swift
import Foundation

/// Pure port of the submit-action → answer logic in src/PromptPanel.tsx.
public enum A2UIAnswerBridge {
    public static func answer(name: String, context: [String: JSONValue],
                              elapsedS: Double, device: String) -> PromptAnswer? {
        guard name.hasPrefix("submit") else { return nil }
        let viaRaw = context["via"]?.stringValue
        let via = (viaRaw == "choice" || viaRaw == "voice_text") ? viaRaw! : "text"

        // /choice arrives as a 1-element array; unwrap. null/missing → "" (ack).
        let rawValue: JSONValue?
        if let arr = context["value"]?.arrayValue { rawValue = arr.first } else { rawValue = context["value"] }
        let answer = stringify(rawValue)
        return PromptAnswer(answer: answer, via: via, elapsedS: elapsedS, device: device)
    }

    private static func stringify(_ v: JSONValue?) -> String {
        switch v {
        case .some(.string(let s)): return s
        case .some(.number(let n)): return n == n.rounded() ? String(Int(n)) : String(n)
        case .some(.bool(let b)): return b ? "true" : "false"
        case .none, .some(.null): return ""
        // A nested object/array value should not occur for /draft, /choice[0],
        // or /scale, but mirror JS String() best-effort rather than dropping it.
        case .some(let other):
            if let data = try? JSONEncoder().encode(other), let s = String(data: data, encoding: .utf8) {
                return s
            }
            return ""
        }
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: same `-only-testing:CennoSharedTests/A2UIAnswerBridgeTests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add companion/Sources/Shared/A2UIAnswerBridge.swift companion/Tests/CennoSharedTests/A2UIAnswerBridgeTests.swift
git commit -m "feat(companion): A2UIAnswerBridge (port of PromptPanel answer extraction)"
```

---

## Task 7: `A2UIMessageBuilder` — `[JSONValue]` → `[A2uiMessage]`

**Files:**
- Create: `companion/Sources/Shared/A2UIMessageBuilder.swift`
- Test: `companion/Tests/CennoSharedTests/A2UIMessageBuilderTests.swift`

Bridges the JSONValue envelope into a2ui-swift's `[A2uiMessage]` by JSON round-trip, and exposes `messages(for:)`/`messages(fromPassthrough:)` that apply the remap.

- [ ] **Step 1: Write the failing test**

```swift
import XCTest
@testable import CennoShared
import A2UISwiftUI

final class A2UIMessageBuilderTests: XCTestCase {
    func testDesugarPathDecodesIntoSurface() throws {
        let json = #"{"title":"Hi","body_md":"","input":{"kind":"text"}}"#
        let p = try JSONDecoder().decode(PromptPayload.self, from: Data(json.utf8))
        let messages = try A2UIMessageBuilder.messages(for: p)
        let vm = SurfaceViewModel(catalog: basicCatalog)
        let errors = vm.processMessages(messages)
        XCTAssertTrue(errors.isEmpty, "\(errors)")
        XCTAssertNotNil(vm.componentTree)
        // After remap the input is a custom CennoTextField node somewhere in the tree.
        XCTAssertTrue(containsType(vm.componentTree, "CennoText"))
    }

    private func containsType(_ node: ComponentNode?, _ name: String) -> Bool {
        guard let node else { return false }
        if case .custom(let n) = node.type, n == name { return true }
        return node.children.contains { containsType($0, name) }
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `xcodebuild test ... -only-testing:CennoSharedTests/A2UIMessageBuilderTests`
Expected: FAIL — `A2UIMessageBuilder` undefined.

- [ ] **Step 3: Implement**

Create `companion/Sources/Shared/A2UIMessageBuilder.swift`:

```swift
import Foundation
import A2UISwiftUI   // re-exports A2UISwiftCore (A2uiMessage, RawComponent, ...)

/// Converts cenno's JSONValue envelope into a2ui-swift's [A2uiMessage], after
/// applying CennoComponentRemap. Source is either the desugared simple prompt
/// or the raw `a2ui` passthrough field.
public enum A2UIMessageBuilder {
    public enum BuildError: Error { case passthroughNotAnArray }

    public static func messages(for payload: PromptPayload) throws -> [A2uiMessage] {
        if let a2ui = payload.a2ui {
            guard let arr = a2ui.arrayValue else { throw BuildError.passthroughNotAnArray }
            return try decode(CennoComponentRemap.apply(arr))
        }
        return try desugarMessages(for: payload)
    }

    /// The desugared-only envelope, ignoring any `a2ui` passthrough. Used as the
    /// fallback when a passthrough payload fails to build/process (parity with
    /// PromptPanel.tsx, which renders `desugar(prompt)` if the rich surface throws).
    public static func desugarMessages(for payload: PromptPayload) throws -> [A2uiMessage] {
        try decode(CennoComponentRemap.apply(A2UIDesugar.messages(for: payload)))
    }

    private static func decode(_ values: [JSONValue]) throws -> [A2uiMessage] {
        let data = try JSONEncoder().encode(values)
        return try JSONDecoder().decode([A2uiMessage].self, from: data)
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: same `-only-testing:CennoSharedTests/A2UIMessageBuilderTests`
Expected: PASS. If `processMessages` reports a schema error on custom components, confirm a2ui-swift accepts unknown `component` names as `.custom` (it does — `catalogId` is opaque); otherwise the remap target names need a `cenno:`-free identifier (already plain, e.g. `CennoText`).

- [ ] **Step 5: Commit**

```bash
git add companion/Sources/Shared/A2UIMessageBuilder.swift companion/Tests/CennoSharedTests/A2UIMessageBuilderTests.swift
git commit -m "feat(companion): build [A2uiMessage] from desugar/passthrough + remap"
```

---

## Task 8: `CennoComponentCatalog` — native SwiftUI leaf views

**Files:**
- Create: `companion/Sources/iPhone/CennoComponentCatalog.swift`

Integration code (rendering) — verified in the simulator at Task 10. Implements the six custom leaves. Each reads `node.typedProperties(...)`, binds via `surface.makeDataContext(path: node.dataContextPath)`, and inputs fire their `submitAction`/`selectAction` exactly like the built-in Button (`surface.dispatchAction` + `@Environment(\.a2uiActionHandler)`).

- [ ] **Step 1: Implement the catalog**

Create `companion/Sources/iPhone/CennoComponentCatalog.swift`:

```swift
import SwiftUI
import A2UISwiftUI

/// Renders cenno's leaf components (remapped to Cenno* typeNames) natively.
///
/// Output is pinned to `AnyView` rather than `some View`: the protocol declares
/// `associatedtype Output: View`, and an opaque `some View` from a switch can
/// fail to infer a single concrete `Output`. `AnyView` removes that ambiguity
/// (Codex BLOCKER #1). The `default` branch never fires in practice — the remap
/// only ever produces the six Cenno* typeNames — so losing a2ui-swift's
/// "EmptyView ⇒ render children" fallback for unknown types is acceptable here.
struct CennoComponentCatalog: CustomComponentCatalog {
    typealias Output = AnyView
    @MainActor
    func build(typeName: String, node: ComponentNode, surface: SurfaceModel) -> AnyView {
        switch typeName {
        case "CennoText":         return AnyView(CennoTextView(node: node, surface: surface))
        case "CennoTextField":    return AnyView(CennoTextFieldView(node: node, surface: surface))
        case "CennoChoicePicker": return AnyView(CennoChoicePickerView(node: node, surface: surface))
        case "CennoSlider":       return AnyView(CennoSliderView(node: node, surface: surface))
        case "CennoScale":        return AnyView(CennoScaleView(node: node, surface: surface))
        case "CennoDots":         return AnyView(CennoDotsView(node: node, surface: surface))
        default:                  return AnyView(EmptyView())
        }
    }
}

// MARK: - Action firing (mirrors the built-in A2UIButton)

@MainActor
private func fire(_ action: Action?, node: ComponentNode, surface: SurfaceModel,
                  handler: (@Sendable (ResolvedAction) -> Void)?) {
    guard case .event(let name, let ctx)? = action else { return }
    let dc = surface.makeDataContext(path: node.dataContextPath)
    var resolved: [String: AnyCodable] = [:]
    ctx?.forEach { resolved[$0.key] = dc.resolveDynamicValue($0.value) ?? .null }
    surface.dispatchAction(name: name, sourceComponentId: node.id, context: resolved)
    handler?(ResolvedAction(name: name, sourceComponentId: node.id, context: resolved))
}

// MARK: - Text (Markdown + variant sizing)

private struct CennoTextView: View {
    let node: ComponentNode; let surface: SurfaceModel
    private struct Props: Codable { var text: String?; var variant: String? }
    var body: some View {
        let _ = node.instance   // register @Observable tracking
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        // desugar emits `text` as a literal string; no data-binding resolve needed.
        let raw = p.text ?? ""
        Text(markdown(raw)).font(font(for: p.variant)).frame(maxWidth: .infinity, alignment: .leading)
    }
    private func markdown(_ s: String) -> AttributedString {
        (try? AttributedString(markdown: s,
            options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(s)
    }
    private func font(for variant: String?) -> Font {
        switch variant {
        case "h1": return .system(size: 34, weight: .bold)
        case "h2", "h3", "h4", "h5": return .title2.bold()
        case "caption": return .caption
        default: return .body
        }
    }
}

// MARK: - TextField (voice flag + submitAction)

private struct CennoTextFieldView: View {
    let node: ComponentNode; let surface: SurfaceModel
    @Environment(\.a2uiActionHandler) private var handler
    @FocusState private var focused: Bool
    private struct Props: Codable { var label: String?; var value: DynamicString?
                                    var voice: Bool?; var submitAction: Action? }
    var body: some View {
        let _ = node.instance
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        let dc = surface.makeDataContext(path: node.dataContextPath)
        VStack(spacing: 12) {
            TextField(p.label ?? "Your reply", text: dc.stringBinding(for: p.value), axis: .vertical)
                .textFieldStyle(.roundedBorder).lineLimit(3...).focused($focused)
                .onAppear { focused = true }
            Button("Send") { fire(p.submitAction, node: node, surface: surface, handler: handler) }
                .buttonStyle(.borderedProminent).frame(maxWidth: .infinity, alignment: .trailing)
        }
        // `voice: true` relies on the system keyboard dictation mic for MVP
        // (the on-device push-to-talk path is a tauri-only feature today).
    }
}

// MARK: - ChoicePicker (chips + words variant + selectAction)

private struct CennoChoicePickerView: View {
    let node: ComponentNode; let surface: SurfaceModel
    @Environment(\.a2uiActionHandler) private var handler
    private struct Option: Codable { var label: String; var value: String }
    private struct Props: Codable { var options: [Option]?; var value: DynamicStringList?
                                    var variant: String?; var selectAction: Action? }
    var body: some View {
        let _ = node.instance
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        let dc = surface.makeDataContext(path: node.dataContextPath)
        VStack(spacing: 10) {
            ForEach(p.options ?? [], id: \.value) { opt in
                Button(opt.label) {
                    try? dc.set(bindingPath(p.value), value: .array([.string(opt.value)]))
                    fire(p.selectAction, node: node, surface: surface, handler: handler)
                }
                .font(p.variant == "words" ? .title2 : .body)
                .buttonStyle(.bordered).frame(maxWidth: .infinity)
            }
        }
    }
    private func bindingPath(_ v: DynamicStringList?) -> String {
        if case .dataBinding(let path)? = v { return path }; return "/choice"
    }
}

// MARK: - Slider (min/max labels + selectAction on commit)

private struct CennoSliderView: View {
    let node: ComponentNode; let surface: SurfaceModel
    @Environment(\.a2uiActionHandler) private var handler
    private struct Props: Codable { var min: Double?; var max: Double?; var value: DynamicNumber?
                                    var minLabel: String?; var maxLabel: String?; var selectAction: Action? }
    var body: some View {
        let _ = node.instance
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        let dc = surface.makeDataContext(path: node.dataContextPath)
        let lo = p.min ?? 0, hi = p.max ?? 10
        VStack {
            Slider(value: dc.doubleBinding(for: p.value ?? .literal(lo), fallback: lo), in: lo...hi) { editing in
                if !editing { fire(p.selectAction, node: node, surface: surface, handler: handler) }
            }
            HStack { Text(p.minLabel ?? "").font(.caption); Spacer(); Text(p.maxLabel ?? "").font(.caption) }
        }
    }
}

// MARK: - Scale (discrete numeral row + selectAction)

private struct CennoScaleView: View {
    let node: ComponentNode; let surface: SurfaceModel
    @Environment(\.a2uiActionHandler) private var handler
    private struct Props: Codable { var min: Double?; var max: Double?; var value: DynamicNumber?
                                    var minLabel: String?; var maxLabel: String?; var selectAction: Action? }
    var body: some View {
        let _ = node.instance
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        let dc = surface.makeDataContext(path: node.dataContextPath)
        let lo = Int(p.min ?? 1), hi = Int(p.max ?? 7)
        VStack(spacing: 8) {
            HStack { ForEach(lo...hi, id: \.self) { n in
                Button("\(n)") {
                    try? dc.set(bindingPath(p.value), value: .number(Double(n)))
                    fire(p.selectAction, node: node, surface: surface, handler: handler)
                }.buttonStyle(.bordered).frame(maxWidth: .infinity)
            } }
            HStack { Text(p.minLabel ?? "").font(.caption); Spacer(); Text(p.maxLabel ?? "").font(.caption) }
        }
    }
    private func bindingPath(_ v: DynamicNumber?) -> String {
        if case .dataBinding(let path)? = v { return path }; return "/scale"
    }
}

// MARK: - Dots (step pagination)

private struct CennoDotsView: View {
    let node: ComponentNode; let surface: SurfaceModel
    private struct Props: Codable { var step: Double?; var total: Double? }
    var body: some View {
        let _ = node.instance
        let p = (try? node.typedProperties(Props.self)) ?? Props()
        let total = Int(p.total ?? 1), step = Int(p.step ?? 1)
        HStack(spacing: 6) {
            ForEach(1...max(total, 1), id: \.self) { i in
                Circle().fill(i == step ? Color.primary : Color.secondary.opacity(0.3))
                    .frame(width: 7, height: 7)
            }
        }
    }
}
```

- [ ] **Step 2: Build to verify it compiles**

Run: `cd companion && xcodegen generate && xcodebuild build -project CennoCompanion.xcodeproj -scheme CennoiPhone -destination 'platform=iOS Simulator,name=iPhone 17'`
Expected: `** BUILD SUCCEEDED **`. Fix any API mismatch against `Vendor/a2ui-swift` source. Most likely points (confirm against the vendored source if a name differs): the `CustomComponentCatalog` Output/`@ViewBuilder` shape (the plan pins `typealias Output = AnyView`); `DataContext.set` argument label; `doubleBinding` signature; `AnyCodable` case names; whether `Action` decodes from the `submitAction`/`selectAction`/`action` props as written.

- [ ] **Step 3: Verify architecture risk #1 early (basic Button renders remapped child labels)**

Before wiring the full screen, prove that a2ui-swift's **basic** `Button` and containers render the remapped `CennoText` children. Add a temporary preview at the bottom of `CennoComponentCatalog.swift`:

```swift
#if DEBUG
import CennoShared
#Preview("Risk1: Button+Row labels") {
    let json = #"{"title":"Proceed?","body_md":"","input":{"kind":"confirm"}}"#
    let payload = try! JSONDecoder().decode(PromptPayload.self, from: Data(json.utf8))
    let messages = try! A2UIMessageBuilder.messages(for: payload)
    let vm = SurfaceViewModel(catalog: basicCatalog)
    _ = vm.processMessages(messages)
    return A2UISurfaceView(viewModel: vm, catalog: CennoComponentCatalog())
}
#endif
```

Run this preview in Xcode. **Expected:** the title "Proceed?" renders (CennoText inside a basic Column) and the **Yes / No buttons show their labels** (CennoText inside a basic Button inside a basic Row).

**Contingency (if labels are blank):** a2ui-swift's basic Button does not route its `child` through the custom catalog. Then add `"Button"` to `CennoComponentRemap.leaves` and implement a `CennoButton` leaf in the catalog that renders its child label by reading the `child` id from `node.children` (or `node.instance.properties["child"]`) and rendering that child node's text, and fires `action` via the shared `fire(...)` helper. Re-run this preview before proceeding. Delete the temporary preview once verified (it is re-added properly in Task 10).

- [ ] **Step 4: Commit**

```bash
git add companion/Sources/iPhone/CennoComponentCatalog.swift companion/CennoCompanion.xcodeproj
git commit -m "feat(companion): CennoComponentCatalog native leaf views"
```

---

## Task 9: `A2UIPromptView` + landscape

**Files:**
- Create: `companion/Sources/iPhone/A2UIPromptView.swift`
- Modify: `companion/Sources/iPhone/PhonePromptDetailView.swift`
- Modify: `companion/Sources/iPhone/Info.plist`

- [ ] **Step 1: Implement `A2UIPromptView`**

Create `companion/Sources/iPhone/A2UIPromptView.swift`:

```swift
import SwiftUI
import A2UISwiftUI
import CennoShared

/// Renders a PromptRecord through the A2UI runtime and bridges submit actions
/// back to CloudKitRelay. Replaces the hand-rolled input switch.
struct A2UIPromptView: View {
    let prompt: PromptRecord
    @EnvironmentObject var relay: CloudKitRelay
    @Environment(\.dismiss) private var dismiss
    @State private var vm: SurfaceViewModel?
    @State private var buildError: String?
    private let shownAt = Date()

    var body: some View {
        Group {
            if let vm {
                // `catalog:` IS the custom component catalog (generic
                // `where Catalog: CustomComponentCatalog`); the core component
                // registry lives on the SurfaceViewModel via `init(catalog:)`.
                A2UISurfaceView(viewModel: vm, catalog: CennoComponentCatalog(), scrolls: true) { action in
                    handle(action)
                }
            } else if let buildError {
                ContentUnavailableView("Couldn't render", systemImage: "exclamationmark.triangle",
                                       description: Text(buildError))
            } else {
                ProgressView()
            }
        }
        .padding()
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Skip") { Task { await relay.markTimedOut(promptID: prompt.id); dismiss() } }
            }
        }
        .onAppear(perform: buildSurface)
    }

    private func buildSurface() {
        guard vm == nil else { return }
        // Try the primary path (passthrough if present, else desugar). If a
        // passthrough payload fails to build or process, fall back to the
        // desugared prompt — parity with PromptPanel.tsx's error boundary.
        if let model = makeSurface(try? A2UIMessageBuilder.messages(for: prompt.payload)) {
            vm = model; return
        }
        if prompt.payload.a2ui != nil,
           let model = makeSurface(try? A2UIMessageBuilder.desugarMessages(for: prompt.payload)) {
            vm = model; return
        }
        buildError = "This prompt couldn't be rendered."
    }

    /// Process messages into a SurfaceViewModel, or nil if they're absent,
    /// errored, or produced no component tree.
    private func makeSurface(_ messages: [A2uiMessage]?) -> SurfaceViewModel? {
        guard let messages else { return nil }
        let model = SurfaceViewModel(catalog: basicCatalog)
        guard model.processMessages(messages).isEmpty, model.componentTree != nil else { return nil }
        return model
    }

    private func handle(_ action: ResolvedAction) {
        // Convert a2ui-swift's [String: AnyCodable] context → [String: JSONValue].
        let context: [String: JSONValue] = (try? JSONDecoder().decode(
            [String: JSONValue].self, from: JSONEncoder().encode(action.context))) ?? [:]
        let elapsed = Date().timeIntervalSince(shownAt)
        guard let answer = A2UIAnswerBridge.answer(name: action.name, context: context,
                                                   elapsedS: elapsed, device: "iphone") else { return }
        Task { await relay.submit(answer: answer, for: prompt.id); dismiss() }
    }
}
```

- [ ] **Step 2: Delegate `PhonePromptDetailView` to it**

Replace the body of `companion/Sources/iPhone/PhonePromptDetailView.swift` (the whole file) with a thin wrapper, removing the old native input controls:

```swift
import SwiftUI
import CennoShared

/// The detail screen now renders every prompt through the A2UI runtime.
struct PhonePromptDetailView: View {
    let prompt: PromptRecord
    var body: some View { A2UIPromptView(prompt: prompt) }
}
```

- [ ] **Step 3: Add landscape orientations to `Info.plist`**

In `companion/Sources/iPhone/Info.plist`, add (inside the top-level `<dict>`):

```xml
<key>UISupportedInterfaceOrientations</key>
<array>
    <string>UIInterfaceOrientationPortrait</string>
    <string>UIInterfaceOrientationLandscapeLeft</string>
    <string>UIInterfaceOrientationLandscapeRight</string>
</array>
```

If `UISupportedInterfaceOrientations` already exists, add the two landscape values to the existing array instead of duplicating the key.

- [ ] **Step 4: Build**

Run: `cd companion && xcodegen generate && xcodebuild build -project CennoCompanion.xcodeproj -scheme CennoiPhone -destination 'platform=iOS Simulator,name=iPhone 17'`
Expected: `** BUILD SUCCEEDED **`.

- [ ] **Step 5: Commit**

```bash
git add companion/Sources/iPhone/A2UIPromptView.swift companion/Sources/iPhone/PhonePromptDetailView.swift companion/Sources/iPhone/Info.plist companion/CennoCompanion.xcodeproj
git commit -m "feat(companion): A2UIPromptView render path + landscape support"
```

---

## Task 10: Full test pass + simulator verification

**Files:** none (verification only)

- [ ] **Step 1: Run the entire unit-test suite**

Run:
```bash
cd companion
xcodebuild test -project CennoCompanion.xcodeproj -scheme CennoiPhone \
  -destination 'platform=iOS Simulator,name=iPhone 17'
```
Expected: all `CennoSharedTests` pass (`** TEST SUCCEEDED **`).

- [ ] **Step 2: Launch the app in the simulator**

```bash
xcrun simctl boot "iPhone 17" 2>/dev/null || true
open -a Simulator
xcodebuild build -project CennoCompanion.xcodeproj -scheme CennoiPhone \
  -destination 'platform=iOS Simulator,name=iPhone 17' \
  -derivedDataPath build
xcrun simctl install booted "$(find build/Build/Products -name 'CennoiPhone.app' -maxdepth 2 | head -1)"
xcrun simctl launch booted app.cenno.companion
```
Expected: the app launches to the "Nothing pending" queue (CloudKit may be empty in the simulator).

- [ ] **Step 3: Render each prompt kind via a SwiftUI preview harness**

Because driving live CloudKit prompts into the simulator is out of scope here, add a preview that renders each input kind directly. Create `companion/Sources/iPhone/A2UIPromptView+Preview.swift`:

```swift
#if DEBUG
import SwiftUI
import CennoShared

#Preview("Kinds") {
    let kinds = ["text", "voice_text", "choice", "scale", "confirm", "none"]
    return TabView {
        ForEach(kinds, id: \.self) { kind in
            NavigationStack { A2UIPromptView(prompt: previewPrompt(kind: kind)) }
                .tabItem { Text(kind) }
        }
    }.environmentObject(CloudKitRelay())
}

private func previewPrompt(kind: String) -> PromptRecord {
    let json = """
    {"title":"How **focused** are you?","body_md":"_Be honest._ See [docs](https://x).",
     "input":{"kind":"\(kind)"},"choices":["Calm","Tense","Wired"]}
    """
    let payload = try! JSONDecoder().decode(PromptPayload.self, from: Data(json.utf8))
    return PromptRecord(id: "preview-\(kind)", payload: payload, deviceHint: .iphone,
                        state: .pending, answer: nil, createdAt: Date(), expiresAt: Date().addingTimeInterval(300))
}
#endif
```

If `PromptRecord` has no public memberwise initializer, add one in `PromptRecord.swift` guarded for reuse (the preview needs it):

```swift
extension PromptRecord {
    public init(id: String, payload: PromptPayload, deviceHint: DeviceHint, state: State,
                answer: PromptAnswer?, createdAt: Date, expiresAt: Date) {
        self.id = id; self.payload = payload; self.deviceHint = deviceHint; self.state = state
        self.answer = answer; self.createdAt = createdAt; self.expiresAt = expiresAt
    }
}
```

- [ ] **Step 4: Verify rendering in Xcode previews (portrait + landscape)**

Open `A2UIPromptView+Preview.swift` in Xcode, run the preview, and confirm for each tab:
- **text/voice_text:** Markdown renders (`focused` bold, `Be honest.` italic, `docs` link); TextField + Send present; Send returns an answer.
- **choice:** three chips; tapping one submits.
- **scale:** numerals 1–7 with "not at all"/"completely"; tap submits.
- **confirm:** Yes/No row; tap submits "yes"/"no".
- **none:** title + body only.
- Rotate the preview/simulator to landscape: content scrolls, inputs remain reachable, the Row (choices/confirm) uses horizontal width.

- [ ] **Step 5: Commit**

```bash
git add companion/Sources/iPhone/A2UIPromptView+Preview.swift companion/Sources/iPhone/PhonePromptDetailView.swift companion/CennoCompanion.xcodeproj
git commit -m "test(companion): A2UI render preview harness for all prompt kinds"
```

---

## Self-review notes

- **Spec coverage:** single A2UI path (Tasks 7–9) ✓; vendored pinned fork (Task 1) ✓; desugar ported to Swift (Task 4) ✓; no CloudKit schema change — only decode added (Task 3) ✓; cenno catalog via CustomComponentCatalog (Task 8) ✓; Markdown (CennoTextView) ✓; landscape (Task 9) ✓; Watch untouched ✓.
- **Known MVP limitations (documented):** custom `~/.cenno` widget kinds fall back to text on iPhone (no phone-side config); `voice:true` uses keyboard dictation, not push-to-talk; `Image`/`DateTimeInput` use a2ui-swift's basic views (no cenno `fit`/`submitAction` extension on iPhone).
- **Integration risks to watch during execution:** (1) basic `Button` rendering a remapped `CennoText` child label — verify Yes/No/Send labels appear; if not, also remap `Button`→keep basic but render label via basic Text (don't remap Text inside buttons) or implement a `CennoButton`. (2) exact a2ui-swift API names (`DataContext.set`, `doubleBinding`, `AnyCodable` cases) — confirm against the vendored source at first build.
