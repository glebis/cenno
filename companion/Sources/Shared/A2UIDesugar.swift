import Foundation

/// Pure port of src/a2ui/desugar.ts: PromptPayload → A2UI v0.9 message
/// envelope ([createSurface, updateComponents, updateDataModel]) as JSONValue.
/// Component names match the TS catalog; CennoComponentRemap renames the
/// cenno-specific leaves to Cenno* typeNames before rendering.
///
/// The iPhone has no `~/.cenno` widget config, so the TS `widgets` parameter is
/// omitted entirely — a custom `input.kind` with no built-in match falls through
/// to the text default (agents needing custom widgets send them via `a2ui`).
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
