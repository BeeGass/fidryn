# Agent scripts

Scripts here are kept and rerun. Add one only when an agent will run it again as part of working in this repository.

One-off commands do not land here. Run those in the shell, or leave a note in the scratchpad if the investigation itself matters.

The top-level listing has no `scripts/` directory. Do not invent product scripts in this folder.

## `shoot.py`

Screenshots and audits the site and the mill in headless Chrome. Standard library only (Python 3.12+).

- `python3 .agents/scripts/shoot.py serve` serves `site/` on http://127.0.0.1:8752 with clean URLs.
- `python3 .agents/scripts/shoot.py site --out .agents/scratchpad/shots/site` screenshots every page at 390, 820, and 1440 pixels in light and dark, and fails on sideways scrolling, JavaScript errors (including `console.error`), or a failed search, drawer, theme, or specimen check.
- `python3 .agents/scripts/shoot.py mill --out .agents/scratchpad/shots/mill` does the same for a running `fidryn ui` (default `http://127.0.0.1:8751`) in its first-run, run, table, JSON, contingent, and diagnostics states.

Both also press Tab round every page (the mill after its last state, with a result and history on it) and fail when a focus ring is missing or cut off, or a focused control is out of view or covered.

Chrome runs with a throwaway profile, background downloads turned off, and every host except 127.0.0.1 blocked; the profile is deleted afterwards.
