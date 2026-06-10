# Agent skills for cenno

Drop-in [skills](https://docs.claude.com/en/docs/claude-code/skills) that teach AI agents how to use cenno. Each is a self-contained `SKILL.md` with usage, the canonical CLI path (`/Applications/cenno.app/Contents/MacOS/cenno`), and the MCP config snippet.

| Skill | For |
|---|---|
| [`cenno`](cenno/SKILL.md) | Using cenno to ask the user questions — `ask_user`, input kinds, flows, multi-step questionnaires, custom 1–N scales via `a2ui`, etiquette |
| [`cenno-setup`](cenno-setup/SKILL.md) | Installing cenno and wiring it into a project's `.mcp.json`, then verifying the round-trip |

## Install for Claude Code

Copy a skill into your skills directory (project-level or `~/.claude/skills/`):

```bash
cp -R skills/cenno ~/.claude/skills/cenno
cp -R skills/cenno-setup ~/.claude/skills/cenno-setup
```

They activate automatically when an agent's task matches the skill description.
