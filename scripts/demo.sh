#!/usr/bin/env bash
# Fire a demo prompt of each kind at the running (or auto-launched) cenno.
# Usage: scripts/demo.sh [text|voice|choice|scale|confirm|mood|table|matrix|sliders|images|image-error|image-remote|all]
set -euo pipefail
cd "$(dirname "$0")/.."
BIN="${CENNO_BIN:-src-tauri/target/release/cenno}"
[ -x "$BIN" ] || { echo "build first: npx tauri build --no-bundle"; exit 1; }

ask() { # $1 = ask_user arguments JSON
  (printf '%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"demo","version":"0"}}}' \
    '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
    "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"ask_user\",\"arguments\":$1}}"; \
   sleep "${CENNO_DEMO_WAIT:-45}") | "$BIN" --mcp-stdio | tail -1
}

case "${1:-all}" in
  text)    ask '{"title":"Quick note","body_md":"What are you working on **right now**?","timeout_s":40}' ;;
  voice)   ask '{"title":"Say a few words about your day.","input":{"kind":"voice_text"},"timeout_s":60}' ;;
  choice)  ask '{"title":"Where did this hour go?","input":{"kind":"choice"},"choices":["Deep work","Meetings","Email","Wandering"],"timeout_s":40}' ;;
  scale)   ask '{"title":"How focused were you this hour?","input":{"kind":"scale"},"flow":"ema","progress":{"step":1,"total":3},"timeout_s":40}' ;;
  confirm) ask '{"title":"Stand up and stretch.","input":{"kind":"confirm"},"flow":"reminder","timeout_s":40}' ;;
  mood)    ask '{"title":"How are you feeling?","input":{"kind":"choice"},"choices":["great","good","okay","low","rough"],"flow":"mood","timeout_s":40}' ;;
  table)   ask '{"title":"Which plan fits?","a2ui":[{"version":"v0.9","createSurface":{"surfaceId":"main","catalogId":"cenno:catalog/v1"}},{"version":"v0.9","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"Column","children":["t"]},{"id":"t","component":"Table","caption":"Plans","columns":[{"label":"Plan"},{"label":"Seats","align":"end"},{"label":"Cost","align":"end"}],"rows":[["free","1","$0"],["pro","3","$20"],["team","10","$60"]]}]}},{"version":"v0.9","updateDataModel":{"surfaceId":"main","path":"/","value":{}}}],"timeout_s":60}' ;;
  matrix)  ask '{"title":"Rate this session.","a2ui":[{"version":"v0.9","createSurface":{"surfaceId":"main","catalogId":"cenno:catalog/v1"}},{"version":"v0.9","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"Column","children":["m"]},{"id":"m","component":"RatingMatrix","mode":"matrix","rows":[{"value":"pace","label":"Pace"},{"value":"useful","label":"Usefulness"},{"value":"clarity","label":"Clarity"}],"min":1,"max":5,"minLabel":"low","maxLabel":"high","value":{"path":"/items"},"selectAction":{"event":{"name":"submit-matrix","context":{"value":{"path":"/items"},"via":"choice"}}}}]}},{"version":"v0.9","updateDataModel":{"surfaceId":"main","path":"/","value":{"items":{}}}}],"timeout_s":60}' ;;
  sliders) ask '{"title":"How was each part?","a2ui":[{"version":"v0.9","createSurface":{"surfaceId":"main","catalogId":"cenno:catalog/v1"}},{"version":"v0.9","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"Column","children":["s"]},{"id":"s","component":"RatingMatrix","mode":"sliders","rows":[{"value":"pace","label":"Pace"},{"value":"useful","label":"Usefulness"},{"value":"clarity","label":"Clarity"}],"min":0,"max":10,"minLabel":"not at all","maxLabel":"completely","value":{"path":"/items"},"selectAction":{"event":{"name":"submit-sliders","context":{"value":{"path":"/items"},"via":"choice"}}}}]}},{"version":"v0.9","updateDataModel":{"surfaceId":"main","path":"/","value":{"items":{}}}}],"timeout_s":60}' ;;
  images)  ask '{"title":"Pick a direction","a2ui":[{"version":"v0.9","createSurface":{"surfaceId":"main","catalogId":"cenno:catalog/v1"}},{"version":"v0.9","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"Column","children":["p","ic"]},{"id":"p","component":"Text","text":"Keep which direction?","variant":"caption"},{"id":"ic","component":"ImageChoice","variant":"mutuallyExclusive","options":[{"value":"warm","label":"warm","image":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAIAAAAlC+aJAAAAT0lEQVR42u3PQQkAAAgEsItoBvt30Ai+hcEKLNP1WgQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQELgtkXRGWZb480AAAAABJRU5ErkJggg=="},{"value":"cool","label":"cool","image":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAIAAAAlC+aJAAAAT0lEQVR42u3PQQkAAAgEsEtiJesbxwi+hcEKLNXzWgQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQELgtH1VEtURDC4QAAAABJRU5ErkJggg=="},{"value":"quiet","label":"quiet","image":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAIAAAAlC+aJAAAAUElEQVR42u3PQQkAAAgEsMvn3w72T2AE38JgBZbqeS0CAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICApcFjc1w8Su0zKMAAAAASUVORK5CYII="}],"value":{"path":"/pick"},"selectAction":{"event":{"name":"submit-image","context":{"value":{"path":"/pick"},"via":"choice"}}}}]}},{"version":"v0.9","updateDataModel":{"surfaceId":"main","path":"/","value":{"pick":[]}}}],"timeout_s":60}' ;;
  # One missing file and one remote URL beside a tile that loads: proves the
  # per-tile reason line AND that a broken tile stays selectable.
  image-error) ask '{"title":"Two of these cannot render","a2ui":[{"version":"v0.9","createSurface":{"surfaceId":"main","catalogId":"cenno:catalog/v1"}},{"version":"v0.9","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"Column","children":["ic"]},{"id":"ic","component":"ImageChoice","variant":"mutuallyExclusive","options":[{"value":"missing","label":"missing file","image":"/tmp/cenno-no-such-image.png"},{"value":"undecodable","label":"not a real png","image":"data:image/png;base64,AAAAAAAA"},{"value":"good","label":"loads fine","image":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAIAAAAlC+aJAAAAT0lEQVR42u3PQQkAAAgEsItoBvt30Ai+hcEKLNP1WgQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQELgtkXRGWZb480AAAAABJRU5ErkJggg=="}],"value":{"path":"/pick"},"selectAction":{"event":{"name":"submit-image","context":{"value":{"path":"/pick"},"via":"choice"}}}}]}},{"version":"v0.9","updateDataModel":{"surfaceId":"main","path":"/","value":{"pick":[]}}}],"timeout_s":60}' ;;
  # A remote URL never reaches a panel: the guard rejects the payload
  # before a prompt is registered, so this one prints the agent-facing error.
  image-remote) ask '{"title":"This payload is rejected","a2ui":[{"version":"v0.9","createSurface":{"surfaceId":"main","catalogId":"cenno:catalog/v1"}},{"version":"v0.9","updateComponents":{"surfaceId":"main","components":[{"id":"root","component":"Image","url":"https://example.com/a.png"}]}}],"timeout_s":60}' ;;
  all)     for k in mood text voice choice scale confirm table matrix sliders images image-error image-remote; do "$0" "$k"; done ;;
  *) echo "usage: $0 [text|voice|choice|scale|confirm|mood|table|matrix|sliders|images|image-error|image-remote|all]"; exit 1 ;;
esac
