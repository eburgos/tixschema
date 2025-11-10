# AGENTS.md

This file provides guidance to AI agents working with this codebase.

## Issue Tracking with bd (beads)

**IMPORTANT**: This project uses **bd (beads)** for ALL issue tracking. Do NOT use markdown TODOs, task lists, or other tracking methods.

### Why bd?

- Dependency-aware: Track blockers and relationships between issues
- Git-friendly: Auto-syncs to JSONL for version control
- Agent-optimized: JSON output, ready work detection, discovered-from links
- Prevents duplicate tracking systems and confusion

### Quick Start

**Check for ready work:**
```bash
bd ready --json
```

**Create new issues:**
```bash
bd create "Issue title" -t bug|feature|task -p 0-4 --json
bd create "Issue title" -p 1 --deps discovered-from:bd-123 --json
```

**Claim and update:**
```bash
bd update bd-42 --status in_progress --json
bd update bd-42 --priority 1 --json
```

**Complete work:**
```bash
bd close bd-42 --reason "Completed" --json
```

### Issue Types

- `bug` - Something broken
- `feature` - New functionality
- `task` - Work item (tests, docs, refactoring)
- `epic` - Large feature with subtasks
- `chore` - Maintenance (dependencies, tooling)

### Priorities

- `0` - Critical (security, data loss, broken builds)
- `1` - High (major features, important bugs)
- `2` - Medium (default, nice-to-have)
- `3` - Low (polish, optimization)
- `4` - Backlog (future ideas)

### Workflow for AI Agents

1. **Check ready work**: `bd ready` shows unblocked issues
2. **Claim your task**: `bd update <id> --status in_progress`
3. **Work on it**: Implement, test, document
4. **Discover new work?** Create linked issue:
   - `bd create "Found bug" -p 1 --deps discovered-from:<parent-id>`
5. **Complete**: `bd close <id> --reason "Done"`
6. **Commit together**: Always commit the `.beads/issues.jsonl` file together with the code changes so issue state stays in sync with code state

### Discovering Issues While Working

**CRITICAL: Never hide or suppress discovered issues.**

When you discover problems while working on a task:

1. **Create an issue immediately** - Don't keep it to yourself or try to fix it silently
2. **Link it properly** - Use `--deps discovered-from:<parent-id>` to connect to current work
3. **Assess priority** - Use appropriate priority (0-4) based on severity
4. **Let the system decide** - The issue will be evaluated and prioritized appropriately

**Examples of what to report:**
- Bugs or errors you encounter
- Code smells or technical debt
- Missing tests or documentation
- Security concerns
- Performance issues
- Inconsistencies or edge cases

**Why this matters:**
- Transparency: Team/user can see full scope of work
- Prioritization: Issues can be evaluated and scheduled properly
- Context: `discovered-from` links preserve discovery context
- Accountability: Nothing gets lost or forgotten

```bash
# Example: Found a bug while working on bd-42
bd create "API endpoint returns 500 on empty input" -t bug -p 1 --deps discovered-from:bd-42 --json
```

**Remember**: Discovering issues is valuable work. Report them, don't hide them.

### Auto-Sync

bd automatically syncs with git:
- Exports to `.beads/issues.jsonl` after changes (5s debounce)
- Imports from JSONL when newer (e.g., after `git pull`)
- No manual export/import needed!

### MCP Server (Recommended)

If using Claude or MCP-compatible clients, install the beads MCP server:

```bash
pip install beads-mcp
```

Add to MCP config (e.g., `~/.config/claude/config.json`):
```json
{
  "beads": {
    "command": "beads-mcp",
    "args": []
  }
}
```

Then use `mcp__beads__*` functions instead of CLI commands.

### Important Rules

- ✅ Use bd for ALL task tracking
- ✅ Always use `--json` flag for programmatic use
- ✅ Link discovered work with `discovered-from` dependencies
- ✅ Check `bd ready` before asking "what should I work on?"
- ✅ **Report discovered issues immediately** - Never hide or suppress problems you find
- ❌ Do NOT create markdown TODO lists
- ❌ Do NOT use external issue trackers
- ❌ Do NOT duplicate tracking systems
- ❌ Do NOT keep discovered issues to yourself

For more details, see README.md and QUICKSTART.md.
