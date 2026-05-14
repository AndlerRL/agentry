# TUI Fix Plan — Sequential Implementation

**Date:** 2026-05-14  
**Status:** Testing Complete, Ready to Fix  
**Order:** C → B → A → D

---

## Option C: TUI Flow Testing ✅ COMPLETE

### Test Results

| Tab | Status | Issues Found |
|-----|--------|--------------|
| **Agents (Tab 1)** | ✅ Working | Minor: Long paths could overflow |
| **Prompts (Tab 2)** | ⚠️ Offset bug | Selection includes headers in count |
| **Skills (Tab 3)** | ⚠️ Offset bug | Source headers break index mapping |
| **Sync (Tab 4)** | ⚠️ Offset bug | Prompt-name headers break index mapping |
| **OpenClaw (Tab 5)** | ✅ Working | No major issues |

### CLI Commands Tested

| Command | Status | Notes |
|---------|--------|-------|
| `agentry detect` | ✅ | Shows 6/11 agents correctly |
| `agentry sync --all --dry-run` | ⏳ | Needs testing |
| `agentry skills list` | ⏳ | Needs testing |
| `agentry prompts list` | ⏳ | Needs testing |

---

## Option B: Width Truncation ✅ ALREADY IMPLEMENTED

**Finding:** `truncate_to_width()` function already exists and is used extensively!

### Current Usage

| Function | Lines Using Truncation |
|----------|----------------------|
| `draw_agent_detail_enhanced()` | 4 locations (lines 192, 260, 324, etc.) |
| `draw_prompt_detail()` | 5 locations (lines 567, 581, 598, 612) |
| `draw_skill_detail()` | 3 locations (lines 827, 840, 857) |
| `draw_sync_detail()` | 2 locations (lines 1047, 1251) |

**Status:** ✅ **No action needed** — Already implemented correctly!

---

## Option A: List Selection Offset Bugs 🔧 TO FIX

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

### Files to Modify

| File | Changes |
|------|---------|
| `app.rs` | Add selection helper methods |
| `dashboard.rs` | Use helpers instead of direct indexing |
| `app.rs` (key handlers) | Update to use helpers |

---

## Option D: ASCII Art Intro 🔧 TO ADD

### Current State

ASCII art exists in README.md but NOT in the TUI intro screen.

### Implementation

Add to `app.rs` intro rendering:

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

### Phase 1: Fix List Selection (Option A) — HIGH PRIORITY

**Files:** `app.rs`, `dashboard.rs`  
**Estimated:** 2-3 hours  
**Impact:** Critical — fixes broken navigation

### Phase 2: Add ASCII Art Intro (Option D) — MEDIUM PRIORITY

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
   - Verify ASCII art displays
   - Verify animation progress bar

---

**Ready to start Phase 1 (Option A — List Selection Fixes).**
