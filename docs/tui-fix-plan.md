# TUI Fix Plan — Sequential Implementation

**Date:** 2026-05-14  
**Status:** All Options Complete (C, B, A, D)  
**Order:** C → B → A → D

---

## Option C: TUI Flow Testing ✅ COMPLETE

### Test Results

| Tab                  | Status        | Issues Found                            |
| -------------------- | ------------- | --------------------------------------- |
| **Agents (Tab 1)**   | ✅ Working    | Minor: Long paths could overflow        |
| **Prompts (Tab 2)**  | ⚠️ Offset bug | Selection includes headers in count     |
| **Skills (Tab 3)**   | ⚠️ Offset bug | Source headers break index mapping      |
| **Sync (Tab 4)**     | ⚠️ Offset bug | Prompt-name headers break index mapping |
| **OpenClaw (Tab 5)** | ✅ Working    | No major issues                         |

### CLI Commands Tested

| Command                        | Status | Notes                       |
| ------------------------------ | ------ | --------------------------- |
| `agentry detect`               | ✅     | Shows 6/11 agents correctly |
| `agentry sync --all --dry-run` | ✅     | Tested, works               |
| `agentry skills list`          | ✅     | Tested, works               |
| `agentry prompts list`         | ✅     | Tested, works — duplicate GEMINI entry bug found and fixed via dedup in discovery.rs (name+scope first-wins) |

---

## Option B: Width Truncation ✅ ALREADY IMPLEMENTED

**Finding:** `truncate_to_width()` function already exists and is used extensively!

### Current Usage

| Function                       | Lines Using Truncation                  |
| ------------------------------ | --------------------------------------- |
| `draw_agent_detail_enhanced()` | 4 locations (lines 192, 260, 324, etc.) |
| `draw_prompt_detail()`         | 5 locations (lines 567, 581, 598, 612)  |
| `draw_skill_detail()`          | 3 locations (lines 827, 840, 857)       |
| `draw_sync_detail()`           | 2 locations (lines 1047, 1251)          |

**Status:** ✅ **No action needed** — Already implemented correctly!

---

## Option A: List Selection Offset Bugs ✅ COMPLETE

**Status: Implemented** — all selection helpers exist in `app.rs` and all key handlers use them. The root-cause explanation and fix strategy below document what was done.

### Root Cause

All grouped lists have this structure:

```
[Header Row 0]    ← list_selected = 0 (header, not data!)
[Data Item 0]     ← list_selected = 1 (should map to data[0])
[Data Item 1]     ← list_selected = 2 (should map to data[1])
[Header Row 3]    ← list_selected = 3 (header, not data!)
[Data Item 2]     ← list_selected = 4 (should map to data[2])
```

But code does: `data[list_selected]` which is wrong!

### Affected Tabs

1. **Prompts Tab** — Global/Project headers
2. **Skills Tab** — Source headers (`<source> (N installed)`)
3. **Sync Tab** — Prompt-name grouping headers

### Fix Strategy

**Replace** `list_selected: usize` **with per-tab selection tracking:**

```rust
pub enum TabSelection {
    Agents { agent_idx: usize },
    Prompts {
        group: PromptGroup,  // Global or Project(root)
        item_idx: usize,     // Index within group
    },
    Skills {
        source: String,
        skill_idx: usize,
    },
    Sync {
        prompt_name: String,
        mapping_idx: usize,
    },
    OpenClaw { workspace_idx: usize },
}
```

**OR** simpler approach: Add helper methods that map `list_selected` → actual data index:

```rust
impl App {
    fn selected_prompt(&self) -> Option<&UnifiedPrompt> {
        // Walk through grouped structure, skip headers
        // Return prompt at correct index
    }

    fn selected_skill(&self) -> Option<&AvailableSkill> {
        // Same for skills
    }

    fn selected_sync_entry(&self) -> Option<&SyncResultEntry> {
        // Same for sync
    }
}
```

### Files Modified

| File                    | Changes                                |
| ----------------------- | -------------------------------------- |
| `app.rs`                | Selection helper methods added         |
| `dashboard.rs`          | Uses helpers instead of direct indexing |
| `app.rs` (key handlers) | Updated to use helpers                 |

### What Was Implemented

- `selected_prompt_index()` (app.rs:637), `selected_skill()` (app.rs:700), `selected_skill_index()` (app.rs:739), `selected_sync_entry()` (app.rs:776), `selected_workspace_index()` (app.rs:808) — all walk the grouped structure, skipping headers
- Dashboard renderers build the same grouped structure the helpers assume
- All key handlers (`on_enter`, `on_delete`, `on_edit`, `on_insert`, `on_update`, `on_remove`, `on_github`, `on_workflow`) use the helpers, not raw indexing
- Remaining raw `[list_selected]` indexing is Agents-tab only (flat list, no headers — correct)

---

## Option D: ASCII Art Intro ✅ COMPLETE

**Status: Implemented** — the full ASCII art from README.md lives in `crates/agentry-tui/src/ui/intro.rs` as the `ASCII_ART` const (23 lines, 92 chars wide), rendered by `draw_intro()` with animation progress.

### Current State

ASCII art from README.md is now rendered in the TUI intro screen via `draw_intro()` in `ui/intro.rs`.

### Implementation

Implemented in `ui/intro.rs` (original plan sketch):

```rust
fn draw_intro(&self, f: &mut Frame) {
    let ascii_art = vec![
        "███████████████████████████████████████████████████████████████",
        "█▌                                                                ▐█",
        "█▌                    AGENTRY                                     ▐█",
        // ... (full ASCII art from README)
        "███████████████████████████████████████████████████████████████",
    ];

    // Render centered with animation progress
}
```

---

## Implementation Order

### Phase 1: Fix List Selection (Option A) ✅ DONE

**Files:** `app.rs`, `dashboard.rs`  
**Estimated:** 2-3 hours  
**Impact:** Critical — fixes broken navigation

### Phase 2: Add ASCII Art Intro (Option D) ✅ DONE

**Files:** `app.rs` (intro rendering)  
**Estimated:** 30 minutes  
**Impact:** Polish — better first impression

---

## Test Plan After Fixes

1. **Prompts Tab:**
   - Navigate with j/k
   - Select global prompt (should open correct one)
   - Select project prompt (should open correct one)
   - Press Enter to edit (should open correct file)

2. **Skills Tab:**
   - Navigate through source groups
   - Select skill from different sources
   - Press Enter to view details
   - Press 'i' to install (should target correct skill)

3. **Sync Tab:**
   - Navigate through prompt groups
   - Select specific sync mapping
   - Verify correct destination shown

4. **Intro Screen:**
   - Launch `agentry` (no args)
   - Verify ASCII art displays ✅ (verified — art renders from `ui/intro.rs` `ASCII_ART`)
   - Verify animation progress bar

---

**All phases complete (C, B, A, D). Remaining work: regression testing per the checklist above.**
