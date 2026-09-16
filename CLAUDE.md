# Claude Code instructions for SparkGlass

Read `AGENTS.md` first — it's the canonical instructions for any agent
working in this repo (what the project is, who owns what, the
verify-don't-assume rule, calibration discipline, build/test commands, C
ABI rules). Everything in it applies to Claude too. This file only adds
what's specific to Claude Code.

## Keep the handoff doc current

Claude Code sessions don't have reliable visibility into their own
usage-limit percentage. Because of that, update
`docs/SparkGlass_HANDOFF.md` proactively at natural checkpoints — after
finishing a phase, before a risky change, whenever context is about to be
compacted — rather than waiting for a threshold that might not be visible
in time. If you're picking up a session that already left a handoff note,
read it before doing anything else; it's written specifically for you.
