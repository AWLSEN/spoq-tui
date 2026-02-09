# Conversation Scroll Performance Spec

## Context
- Symptom: Conversation view scroll becomes slow as text accumulates.
- Target: Keep scroll latency stable as history grows.

## Current Behavior (Observed in Code)
- `render_messages_area` renders all message lines each frame and uses `Paragraph::new(lines)` with full `lines` vector, even when only a subset is visible.
- Height caching and virtualization helpers exist, but the "render all messages" path is still used for the conversation view.

## Primary Hypothesis
Scrolling slows down because every scroll tick rebuilds and renders the entire conversation (all messages + wrapping + spans), which scales linearly with total history size.

## Questions to Confirm
1) How many messages and total visual lines does the slowdown start at (e.g., 200 msgs / 5k lines / 20k lines)?
2) Is the slowdown worse while streaming (segments and markdown cache updates) or only during manual scrolling?
3) Does toggling reasoning blocks, tool events, or subagent events change the slowdown onset?

## Success Criteria
- Scroll latency is roughly constant beyond 1,000+ messages (target: <16ms on typical dev machine, <33ms worst-case).
- No visible jumps or line misalignment when scrolling.
- Streaming remains smooth without regressing input latency or keystroke handling.
- Correct max scroll boundaries with no "dead zones" or stuck-at-top/bottom.

## Plan (Surgical Changes)
1) Re-enable virtualization in `render_messages_area`:
   - Use `height_cache` and `virtualization::calculate_visible_range`.
   - Render only messages in the visible window plus a small buffer.
   - Render to a local `Vec<Line>` limited to the visible window.
   - Apply `Paragraph::scroll` only for the top header offset + intra-message offset.
2) Keep existing cache behavior:
   - Continue to use `rendered_lines_cache` for completed messages.
   - Use `estimate_message_height_fast` only to drive the visible range.
   - Invalidate rendered cache on viewport width change (already in `prepare_render`).
3) Buffer strategy (robust against boundary jitter):
   - Buffer by **messages** not lines (e.g., 1 message above + 1 below) to reduce estimate drift.
   - Clamp buffer to `[0, message_count]` to avoid bounds errors.
4) Preserve unified scroll semantics:
   - Compute `scroll_from_top` from unified scroll.
   - Add header line count to `scroll_from_top` before virtualization.
   - Convert visible offsets to top-based line offsets for `Paragraph`.
5) Keep accurate `max_scroll`:
   - `max_scroll = total_visual_lines.saturating_sub(viewport_height)`
   - `total_visual_lines = header_visual_lines + height_cache.total_lines + footer_lines`
6) Avoid per-frame full recompute:
   - `prepare_render` continues to update height cache incrementally.
   - Only compute visible range in render.
7) Add optional debug metrics (behind a debug flag):
   - Render time per frame.
   - Total lines vs rendered lines.
   - Visible range indices + intra-offset for troubleshooting.

## Edge Cases
- Header lines (error banners) remain fully rendered and are included in scroll math.
- Input section and permission overlays must stay visible at bottom when not scrolled.
- Streaming message height changes should invalidate only affected cached height entries.
- Max scroll must remain accurate to avoid hitting artificial boundaries.
- Very short viewports (e.g., 20x8) should still render a stable subset.
- Messages with file chips / image chips add extra lines; height estimates must account for them.
- Tool events and subagent events add variable height; estimates should bias high to avoid gaps.
- Reasoning collapse/expand should trigger height cache updates for affected messages.

## Testing
- Unit tests for visible-range calculations with header lines + input section.
- Unit tests for `max_scroll` math with header + footer + message heights.
- Manual test with:
  - 1,000+ messages.
  - Mixed content (tool events, reasoning blocks, file chips).
  - Active streaming + user scroll.
  - Long reasoning blocks (expanded/collapsed).
  - Narrow viewport (<= 60 cols).

## Alternatives Considered
- Use a third-party scroll view widget to handle windowing.
- Rely on full render but throttle updates (would still degrade at large sizes).

## Risks
- Off-by-one in scroll math causing jumps.
- Inaccurate height estimation leading to visual mismatches at boundaries.
- Input anchor positioning may need adjustment when only partial content is rendered.
- Height cache invalidation bugs can cause stale offsets (mitigate with assertions in debug).

## Reliability & Robustness Additions
- Add an internal invariant check in debug builds:
  - `height_cache.total_lines == sum(visual_lines)` after updates.
  - `cumulative_offset` monotonic and last offset + last height == total_lines.
- Add a fallback path:
  - If height cache is missing/invalid, render a minimal safe subset or rebuild cache.
- Ensure scroll position clamps after any message list mutation:
  - Clamp `unified_scroll` to `max_scroll` after adding/removing messages.
- Guard against negative offsets when header lines exceed viewport height.
