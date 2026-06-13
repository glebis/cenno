import XCTest
@testable import CennoShared

final class A2UIDesugarTests: XCTestCase {
    private func payload(title: String = "How focused are you?",
                         bodyMd: String = "Be honest.",
                         kind: String? = "text",
                         choices: [String]? = nil,
                         flow: String? = nil,
                         progress: PromptProgress? = nil) -> PromptPayload {
        let json = try! JSONEncoder().encode(DesugarFixture(
            title: title, body_md: bodyMd,
            input: kind.map { DesugarFixture.Input(kind: $0) },
            choices: choices, flow: flow, progress: progress))
        return try! JSONDecoder().decode(PromptPayload.self, from: json)
    }
    private struct DesugarFixture: Encodable {
        struct Input: Encodable { let kind: String }
        let title: String; let body_md: String; let input: Input?
        let choices: [String]?; let flow: String?; let progress: PromptProgress?
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
        let (_, _, byId, col, _) = parts(payload(kind: "scale", progress: PromptProgress(step: 2, total: 5)))
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
