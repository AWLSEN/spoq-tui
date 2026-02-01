# AskUserQuestion Bug Fix Plan - Command Deck

## Executive Summary

Two critical bugs in the AskUserQuestion dialog when accessed from the command deck:

1. **Enter key submits instead of toggling in multi-select mode** - User expects Enter to select options like Space does, but it submits immediately
2. **UI is not dynamic - text gets truncated** - Questions, options, and descriptions get cut off with "..." instead of wrapping

## Root Cause Analysis

### Bug #1: Enter Key Behavior in Multi-Select Mode

**Current Behavior:**
- Enter → `DashboardQuestionConfirm` command → `question_confirm()` → immediate submit
- Space → `DashboardQuestionToggleOption` command → `question_toggle_option()` → toggle selection

**Problem:**
- `question_confirm()` has NO multi-select mode check
- Both single-select and multi-select use the same confirm flow
- In multi-select, pressing Enter on an option submits instead of toggling it

**Expected Behavior:**
- For multi-select questions: Enter should toggle the currently highlighted option (same as Space)
- Submission should require a separate action (like pressing Enter on "Other" or a "Submit" button)

**Files Involved:**
- `src/input/keybindings.rs:241` - Enter → DashboardQuestionConfirm mapping
- `src/input/handlers/permission.rs:177-182` - Command handler
- `src/state/dashboard.rs:1265-1313` - `question_confirm()` implementation
- `src/state/dashboard.rs:1196-1204` - `question_toggle_option()` for comparison

### Bug #2: UI Text Truncation

**Current Behavior:**
- Question text limited to 2 wrapped lines, then "..."
- Option labels truncated to single line with "..."
- Option descriptions truncated to single line with "..."
- "Other" input field truncated

**Problem:**
- `question_card.rs:156` - `wrap_text(config.question, area.width, 2)` hardcoded max 2 lines
- `question_card.rs:221` - `truncate_with_ellipsis(&option_text, ...)` single-line truncation
- `question_card.rs:241` - `truncate_with_ellipsis(desc, ...)` single-line truncation
- `overlay.rs:251` - `let question_section = 2;` hardcoded allocation

**Expected Behavior:**
- Question text should wrap to as many lines as needed (within card height limit)
- Option labels should wrap across multiple lines
- Option descriptions should wrap across multiple lines
- Card height should adjust based on actual content

**Files Involved:**
- `src/ui/dashboard/question_card.rs:156` - Question text rendering
- `src/ui/dashboard/question_card.rs:221` - Option label rendering
- `src/ui/dashboard/question_card.rs:241` - Option description rendering
- `src/ui/dashboard/overlay.rs:251` - Card height calculation

## Implementation Plan

### Phase 1: Fix Enter Key Behavior in Multi-Select Mode

**Goal:** Make Enter toggle options in multi-select mode, not submit

**Option A: Change Enter to Toggle (RECOMMENDED)**
- Modify `DashboardQuestionConfirm` handler to check multi-select mode
- If multi-select: call `question_toggle_option()` instead of `question_confirm()`
- If single-select: call `question_confirm()` as before
- Submission in multi-select requires different gesture (see Option B)

**Option B: Add Explicit Submit Action for Multi-Select**
- Keep Enter as toggle for multi-select
- Add new keybinding for submit (e.g., Ctrl+Enter or dedicated button)
- Update help text to show "Enter: Toggle | Ctrl+Enter: Submit"

**Implementation Steps:**

1. **Update keybinding handler** (`src/input/handlers/permission.rs:177-182`):
   ```rust
   Command::DashboardQuestionConfirm => {
       // Check if current question is multi-select
       if app.dashboard.is_current_question_multi_select() {
           // For multi-select, Enter toggles the option
           app.dashboard.question_toggle_option();
       } else {
           // For single-select, Enter confirms
           if let Some((thread_id, request_id, answers)) = app.dashboard.question_confirm() {
               app.submit_dashboard_question(&thread_id, &request_id, answers);
           }
       }
       true
   }
   ```

2. **Add multi-select check method** to `Dashboard` (if not exists):
   ```rust
   pub fn is_current_question_multi_select(&self) -> bool {
       // Already exists in dashboard.rs:1158-1169
   }
   ```

3. **Add submit keybinding for multi-select** (`src/input/keybindings.rs`):
   ```rust
   dashboard_question.insert(
       KeyCombo::ctrl(KeyCode::Enter),
       Command::DashboardQuestionSubmit,  // New command
   );
   ```

4. **Create new command** (`src/input/command.rs`):
   ```rust
   DashboardQuestionSubmit,  // Explicitly submit multi-select answers
   ```

5. **Update help text** (`src/ui/dashboard/question_card.rs:~434`):
   - Single-select: "Enter: Confirm"
   - Multi-select: "Space/Enter: Toggle | Ctrl+Enter: Submit"

**Files to Modify:**
- `src/input/handlers/permission.rs` - Add multi-select check in handler
- `src/input/keybindings.rs` - Add Ctrl+Enter binding for submit
- `src/input/command.rs` - Add DashboardQuestionSubmit command
- `src/ui/dashboard/question_card.rs` - Update help text based on multi_select mode

### Phase 2: Fix UI Text Truncation - Dynamic Sizing

**Goal:** Make question dialog resize dynamically based on content, wrap text instead of truncating

**Approach:**
1. Calculate actual question height after wrapping (not hardcoded 2)
2. Wrap option labels and descriptions instead of truncating
3. Adjust card height calculation to account for actual content
4. Add scrolling if content exceeds terminal height

**Implementation Steps:**

#### Step 2.1: Dynamic Question Text Height

**File:** `src/ui/dashboard/overlay.rs:240-265`

**Current:**
```rust
let question_section = 2;  // HARDCODED
```

**Replace with:**
```rust
// Calculate actual wrapped question height
let question_section = if let Some(question_text) = first_question_text {
    let wrapped = wrap_text(question_text, card_width_for_wrapping, usize::MAX);
    wrapped.len().max(1).min(6)  // Min 1 line, max 6 lines for questions
} else {
    2  // Fallback
};
```

**Requirements:**
- Need access to question text in `calculate_card_dimensions()`
- Must pass question_data or extract text in caller
- Use same `wrap_text()` function as rendering code

#### Step 2.2: Wrap Option Labels and Descriptions

**File:** `src/ui/dashboard/question_card.rs:187-251`

**Current (lines 221-223):**
```rust
let option_text = if config.multi_select {
    format!("{} {}", checkbox, option.label)
} else {
    format!("{} {}", radio, option.label)
};
let option_line = truncate_with_ellipsis(&option_text, (area.width - option_indent) as usize);
```

**Replace with:**
```rust
let option_text = if config.multi_select {
    format!("{} {}", checkbox, option.label)
} else {
    format!("{} {}", radio, option.label)
};

// Wrap option text across multiple lines if needed
let max_width = (area.width - option_indent) as usize;
let wrapped_option_lines = wrap_text(&option_text, max_width, usize::MAX);

// Render all wrapped lines
for (line_idx, wrapped_line) in wrapped_option_lines.iter().enumerate() {
    if current_y >= area.y + area.height {
        break;  // Out of space
    }

    let line_content = if line_idx == 0 {
        // First line: full option text
        wrapped_line.clone()
    } else {
        // Continuation lines: indent to align with text
        format!("{}{}", " ".repeat(option_indent as usize), wrapped_line)
    };

    // ... render line ...
    current_y += 1;
}
```

**Similarly for descriptions (lines 238-248):**
```rust
// Wrap description text
let wrapped_desc_lines = wrap_text(desc, desc_width as usize, usize::MAX);
for wrapped_desc_line in wrapped_desc_lines {
    // ... render each line with proper indent ...
    current_y += 1;
}
```

#### Step 2.3: Dynamic Height Calculation for Options

**File:** `src/ui/dashboard/overlay.rs:265-290`

**Current:**
```rust
// Options section: 1 row per option, or 2 if descriptions shown
let options_section = if has_descriptions && available_rows > options_count * 2 {
    options_count * 2  // Each option + description
} else {
    options_count  // Just options
};
```

**Replace with:**
```rust
// Options section: calculate based on wrapped option text + descriptions
let options_section = if let Some(questions) = &question_data {
    let mut total_option_rows = 0;

    for question in &questions.questions {
        for option in &question.options {
            // Calculate wrapped option label height
            let option_text = format!("[] {}", option.label);  // Approximate with checkbox
            let wrapped_lines = wrap_text(&option_text, card_width_for_wrapping, usize::MAX);
            total_option_rows += wrapped_lines.len();

            // Add description lines if present and space permits
            if let Some(desc) = &option.description {
                if !desc.is_empty() {
                    let wrapped_desc = wrap_text(desc, card_width_for_wrapping, usize::MAX);
                    total_option_rows += wrapped_desc.len();
                }
            }
        }
    }

    total_option_rows
} else {
    options_count  // Fallback
};
```

#### Step 2.4: Add Scrolling Support (Optional Enhancement)

If total content height exceeds terminal height, add scrolling:

**File:** `src/state/dashboard.rs`

**Add scroll state:**
```rust
pub struct DashboardQuestionState {
    // ... existing fields ...
    pub scroll_offset: usize,  // NEW: vertical scroll position
}
```

**Add scroll commands:**
- `Command::DashboardQuestionScrollUp`
- `Command::DashboardQuestionScrollDown`

**Keybindings:**
```rust
dashboard_question.insert(KeyCombo::plain(KeyCode::PageUp), Command::DashboardQuestionScrollUp);
dashboard_question.insert(KeyCombo::plain(KeyCode::PageDown), Command::DashboardQuestionScrollDown);
```

**Rendering:**
```rust
// In question_card.rs, offset rendering by scroll_offset
let render_start_y = area.y - scroll_offset;
```

### Phase 3: Testing & Validation

**Test Cases:**

1. **Single-select question:**
   - Enter confirms immediately
   - No Ctrl+Enter needed

2. **Multi-select question:**
   - Space toggles option
   - Enter toggles option (NEW)
   - Ctrl+Enter submits
   - Multiple options can be selected

3. **Long question text:**
   - Wraps across multiple lines (3-6 lines)
   - No "..." truncation unless exceeds 6 lines

4. **Long option labels:**
   - Wrap across multiple lines
   - Proper indentation on continuation lines

5. **Option descriptions:**
   - Wrap across multiple lines
   - Display when space permits

6. **Multi-question flow:**
   - Tab navigation works
   - Answer tracking persists
   - Submission only when all answered

7. **"Other" text input:**
   - Enter activates text input when "Other" is highlighted
   - Enter submits when "Other" text is entered
   - Escape cancels "Other" mode

## Implementation Order

### Priority 1: Fix Enter Key Behavior (Bug #1)
**Complexity:** Low
**Impact:** High - Core UX issue
**Files:** 4 files, ~30 lines of code

1. Update `permission.rs` handler with multi-select check
2. Add `DashboardQuestionSubmit` command
3. Add Ctrl+Enter keybinding
4. Update help text

**Estimated Time:** 1-2 hours

### Priority 2: Fix Question Text Wrapping (Bug #2a)
**Complexity:** Medium
**Impact:** High - Immediate visibility improvement
**Files:** 2 files, ~50 lines of code

1. Update `overlay.rs` to calculate dynamic question height
2. Pass question_data to `calculate_card_dimensions()`
3. Remove hardcoded `question_section = 2`

**Estimated Time:** 2-3 hours

### Priority 3: Fix Option Text Wrapping (Bug #2b)
**Complexity:** Medium-High
**Impact:** Medium - Improves option readability
**Files:** 1 file, ~80 lines of code

1. Update `question_card.rs` option rendering loop
2. Replace `truncate_with_ellipsis()` with `wrap_text()`
3. Handle multi-line rendering with proper indentation
4. Update description rendering similarly

**Estimated Time:** 3-4 hours

### Priority 4: Add Scrolling Support (Enhancement)
**Complexity:** High
**Impact:** Low - Nice to have
**Files:** 3 files, ~120 lines of code

1. Add scroll state to `DashboardQuestionState`
2. Add scroll commands and keybindings
3. Update rendering to use scroll offset
4. Add scroll indicators in UI

**Estimated Time:** 4-5 hours (OPTIONAL)

## Risks & Mitigations

### Risk 1: Breaking Existing Single-Select Flow
**Mitigation:** Add multi-select check before changing Enter behavior, preserve existing logic for single-select

### Risk 2: Card Height Calculation Overflow
**Mitigation:** Add max height caps (e.g., 85% of terminal height), handle gracefully when content exceeds available space

### Risk 3: Performance with Large Questions
**Mitigation:** Cache wrapped text results, reuse calculations, limit max lines per question/option

### Risk 4: Wrapping Breaks Alignment
**Mitigation:** Use consistent indentation, test with various terminal widths, verify with edge cases

## Success Criteria

### Bug #1 Fixed:
- [ ] Multi-select questions: Enter toggles currently highlighted option
- [ ] Multi-select questions: Ctrl+Enter submits all selections
- [ ] Single-select questions: Enter confirms (unchanged)
- [ ] Help text correctly shows keybindings based on multi_select mode
- [ ] No regression in multi-question tab navigation

### Bug #2 Fixed:
- [ ] Question text wraps up to 6 lines (no "..." unless exceeds 6)
- [ ] Option labels wrap across multiple lines (no truncation)
- [ ] Option descriptions wrap across multiple lines
- [ ] Card height adjusts dynamically based on content
- [ ] All text readable on narrow terminals (80 cols)
- [ ] No visual glitches or overlaps

## Files Modified Summary

| File | Lines Changed | Purpose |
|------|---------------|---------|
| `src/input/handlers/permission.rs` | ~10 | Add multi-select check in Enter handler |
| `src/input/keybindings.rs` | ~4 | Add Ctrl+Enter binding |
| `src/input/command.rs` | ~2 | Add DashboardQuestionSubmit command |
| `src/ui/dashboard/question_card.rs` | ~100 | Wrap option text, update help text |
| `src/ui/dashboard/overlay.rs` | ~40 | Dynamic question height calculation |
| `src/state/dashboard.rs` | ~5 | Expose multi-select check method |

**Total:** ~161 lines changed across 6 files

## Validation Plan

1. **Manual Testing:**
   - Test all scenarios in "Test Cases" section above
   - Test on narrow terminals (80 cols) and wide (200+ cols)
   - Test with 1-4 question multi-question prompts
   - Test with long question text (5+ sentences)
   - Test with long option labels (50+ chars)

2. **Edge Cases:**
   - Empty question text
   - No options
   - Single option
   - 10+ options (requires scrolling)
   - Very narrow terminal (40 cols)
   - Mixed multi-select and single-select in multi-question prompt

3. **Regression Testing:**
   - Session-level question dialog still works (uses different code path)
   - Command deck still navigable after question dismissed
   - WebSocket submission format unchanged
   - Backend receives correct payload

## Next Steps

1. Review this plan with stakeholders
2. Prioritize phases based on urgency
3. Implement Phase 1 (Enter key fix) first - quickest win
4. Implement Phase 2 & 3 (UI wrapping) - higher complexity
5. Consider Phase 4 (scrolling) based on user feedback after 1-3

## Questions for Review

1. **Enter key behavior:** Should Enter toggle in multi-select, or should we use a different key entirely? (Current plan: Enter = toggle, Ctrl+Enter = submit)
2. **Max question lines:** Is 6 lines for question text reasonable, or should it be higher/lower?
3. **Scrolling priority:** Should we implement scrolling support in initial fix, or defer to future enhancement?
4. **Session-level parity:** Should we also update the session-level question dialog with these improvements?
