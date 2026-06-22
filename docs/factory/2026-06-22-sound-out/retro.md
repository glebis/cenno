# Retro — sound-out v1

1. **What slowed shipping?** Reading App.tsx/userConfig via shell `sed` instead of the Read tool — the editor needs a tracked Read first, so several edits bounced and had to be re-read. Use Read for anything I'll edit.
2. **What caught a real bug?** The code map caught two: `AskRequest.urgency` already existed (avoided a duplicate priority field) and it serializes **lowercase** on the wire (would have silently never matched a "High" string-compare). The gating test's case-folding made it moot.
3. **Which artifact was never used?** `messaging-angles.md` from the JTBD bundle — irrelevant for a personal internal feature; the `gtm-brief.md` was rightly skipped too.
4. **What gets deleted before the next feature?** The JTBD bundle's marketing artifacts for internal tools — generate `jtbd.json` + a short scope note only; skip messaging/GTM unless it's a real product.
