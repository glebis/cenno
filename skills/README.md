# Agent skill for cenno

A drop-in [skill](https://docs.claude.com/en/docs/claude-code/skills) that teaches AI agents how to use cenno: the `ask_user` tool, input kinds, flows, multi-step questionnaires, custom 1–N scales via `a2ui`, a raw-socket fallback, etiquette — plus a **setup mode** that installs cenno and wires it into a project's `.mcp.json`. Carries the canonical CLI path (`/Applications/cenno.app/Contents/MacOS/cenno`) and MCP config.

## Install

**Via [`npx skills`](https://github.com/vercel-labs/skills):**

```bash
npx skills add https://github.com/glebis/cenno/tree/main/skills/cenno
```

**Via prompt** — ask your agent:

> Install the cenno agent skill from https://github.com/glebis/cenno/tree/main/skills/cenno — copy it into my Claude skills directory.

**Manually** (from a clone):

```bash
cp -R skills/cenno ~/.claude/skills/cenno
```

It activates automatically when an agent's task matches the skill — asking the user a question, running a check-in, or setting cenno up.
