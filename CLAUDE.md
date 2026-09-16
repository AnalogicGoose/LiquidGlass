# Claude Code instructions for SparkGlass

Read `AGENTS.md` first — it's the canonical instructions for any agent
working in this repo (what the project is, who owns what, the
verify-don't-assume rule, calibration discipline, build/test commands, C
ABI rules). Everything in it applies to Claude too. This file only adds
what's specific to Claude Code.

## Coding authority in this repo (overrides the global default)

The user's global `~/.claude/CLAUDE.md` normally splits work as "Claude
proposes code in chat, the user applies it; Claude edits documentation
directly." **In this repository specifically, that's overridden**: the
user has given standing authorization for Claude to write, build, test,
commit, and push code directly, without proposing diffs in chat first or
asking before each commit. This was established explicitly in-session
("you 100% do the coding... CLAUDE NEVER FOLDS", "you have complete
independence, make commits, push changes and continue") and is meant to
persist for this project going forward, not just for one conversation.

This does not relax anything else: still no force-push without being
asked, still no `--no-verify`/skipped hooks, still confirm before any
destructive or hard-to-reverse action, still no `Co-Authored-By`/session
lines in commits (see `AGENTS.md`). If the user ever says otherwise in a
given session, that instruction wins over this file for that session.

## Keep the handoff doc current

Claude Code sessions don't have reliable visibility into their own
usage-limit percentage. Because of that, update
`docs/SparkGlass_HANDOFF.md` proactively at natural checkpoints — after
finishing a phase, before a risky change, whenever context is about to be
compacted — rather than waiting for a threshold that might not be visible
in time. If you're picking up a session that already left a handoff note,
read it before doing anything else; it's written specifically for you.

## Memory

This project has a persistent memory system outside the repo (not
committed, not part of the codebase) for cross-session context about how
this user likes to work. Use it as usual; it's independent of the docs in
this repo, which are for any agent, not Claude-specific state.
