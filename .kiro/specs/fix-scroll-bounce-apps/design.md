# Fix Scroll Bounce — Apps List Bugfix Design

## Overview

The Apps page (`apps/rigpulse/src/pages/apps.rs`) uses a hand-rolled virtual scrolling loop
inside `app_list`. The loop calculates which rows to render by dividing `clip_rect` offsets
by row height, then allocates spacer blocks for off-screen rows above and below the visible
range. Because the spacer sizes depend on `clip_rect.top()`, and that value shifts slightly
every frame while the egui scroll area decelerates, the sum of all allocations fluctuates.
egui responds to any change in content height by nudging the scroll offset — which changes
the clip again — producing the visible bounce.

The project already ships `VirtualRows` (`apps/rigpulse/src/virtual_rows.rs`), which fixes
this exact problem by calling `ui.allocate_space(total)` once before rendering any rows,
locking in the content height for the entire frame. Replacing the manual loop with
`VirtualRows` is the minimal, targeted fix.

## Glossary

- **Bug_Condition (C)**: The condition under which the scroll bounce manifests — any frame
  where `app_list` is rendered with one or more entries while a scroll deceleration is in
  progress (i.e., `clip_rect.top()` differs from the previous frame).
- **Property (P)**: The desired invariant — the total height allocated by `app_list` in a
  single frame equals `indices.len() as f32 * height` regardless of the current scroll
  offset or clip position.
- **Preservation**: All row-level rendering behavior (icon, name, publisher, size, action
  button, selection highlight, alternating background, divider line) must remain identical.
- **`app_list`**: The free function in `apps/rigpulse/src/pages/apps.rs` that owns the
  virtual scrolling implementation to be replaced.
- **`VirtualRows`**: The struct in `apps/rigpulse/src/virtual_rows.rs` that pre-allocates
  the full content extent before rendering visible rows.
- **`wide`**: Boolean derived from `ui.available_width() >= 850.0`; controls row height
  (70 px vs 94 px) and column-header visibility.

## Bug Details

### Bug Condition

The bounce manifests whenever `app_list` renders a non-empty list while the scroll area
is decelerating. The manual spacer calculation:

```
first = floor((clip.top() - top) / height).max(0)
last  = ceil((clip.bottom() - top) / height) + 1

spacer_above = first * height
spacer_below = (count - last) * height
```

is sensitive to fractional changes in `clip.top()`. A sub-pixel shift in `clip.top()` can
change `first` by ±1, producing a step-change in the total allocated height of ±`height`
(70 px or 94 px), which forces egui to correct the scroll offset.

**Formal Specification:**
```
FUNCTION isBugCondition(frame)
  INPUT: frame of type RenderFrame
         (frame.clip_top = clip_rect.top() this frame,
          frame.prev_clip_top = clip_rect.top() previous frame,
          frame.count = indices.len())
  OUTPUT: boolean

  RETURN frame.count > 0
         AND frame.clip_top != frame.prev_clip_top
         AND uses_manual_spacer_calculation(app_list)
END FUNCTION
```

### Examples

- 500 apps visible, user flicks the list downward. During the 300 ms deceleration, `clip.top()`
  decreases by ~0.3 px per frame. Every few frames, `first` ticks down by 1 → spacer above
  shrinks by 70 px → total height drops 70 px → egui bumps scroll up 70 px → user sees jitter.
- Same scenario with `VirtualRows`: `allocate_space(500 * 70)` is called first; the clip
  change only affects which rows are rendered, not the total allocation.
- Empty list (`indices.is_empty()`): `app_list` returns early before the virtual loop; no
  spacers involved — bug does not manifest.

## Expected Behavior

### Preservation Requirements

**Unchanged Behaviors:**
- Row rendering logic (icon, name, publisher, size, action button, highlight, divider) must
  produce pixel-identical output to the current implementation.
- The wide-mode column header (36 px row with "Aplikasi / versi", "Publisher", etc.) is
  rendered before the scroll area and must remain unaffected.
- Alternating row background and selection highlight must continue to work.
- Clicking a row selects it; clicking "Detail" sets `details_open = true`.
- The `item_spacing.y = 0.0` override inside `ui.scope` must be preserved.

**Scope:**
All inputs where isBugCondition returns false — including empty lists, single-frame renders
with no clip change, and non-scroll interactions (mouse clicks, selection, detail button) —
must be completely unaffected by this fix.

## Hypothesized Root Cause

The root cause is confirmed from the code and is a single-point issue:

1. **No pre-allocated total extent**: `app_list` never calls `ui.allocate_space` for the
   full list height before entering the render loop. The total allocated height is the sum
   of two spacers and the visible rows, and the spacer sizes change when `clip.top()` shifts.

2. **Spacer boundary uses floor/ceil on a floating-point division**: Integer row boundaries
   derived from `clip_rect` offsets are inherently unstable at the pixel level during
   animation, amplifying the effect of sub-pixel clip changes into full-row spacer jumps.

3. **`VirtualRows` already exists and is correct**: Its `show` method calls
   `ui.allocate_space(total)` as its first action, which fixes both issues above.

## Correctness Properties

Property 1: Bug Condition — Stable Total Content Height During Scroll Deceleration

_For any_ frame where the apps list contains at least one entry and the scroll area is
decelerating (clip top differs from previous frame), the fixed `app_list` function SHALL
allocate exactly `indices.len() as f32 * height` pixels of vertical space, independent
of the current clip position.

**Validates: Requirements 2.2, 2.3**

Property 2: Preservation — Row Rendering Unchanged

_For any_ input where the bug condition does NOT hold (empty list, static scroll position,
or non-scroll interaction), the fixed `app_list` function SHALL produce the same rendered
rows, selection state, and icon painting as the original function.

**Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5**

## Fix Implementation

### Changes Required

**File**: `apps/rigpulse/src/pages/apps.rs`

**Function**: `app_list`

**Specific Changes**:

1. **Remove the manual clip-based first/last calculation** and the two spacer
   `allocate_exact_size` calls that precede and follow the row loop.

2. **Replace the manual loop with `VirtualRows::show`**:
   - Construct `VirtualRows::from_heights(std::iter::repeat_n(height, indices.len()))` to
     describe a uniform-height list.
   - Call `virtual_rows.show(ui, "app_list", |n, ui| { … })` where the closure body
     contains the existing per-row rendering logic, with `indices[n]` used to look up the
     source app.
   - The `VirtualRows` instance can be constructed inline each frame because
     `from_heights` with a uniform height is O(n) but avoids the clip dependency entirely.
     Alternatively, it can be stored in `Controller` and rebuilt when `indices.len()` or
     `wide` changes — but inline construction is simpler and sufficient.

3. **Import `VirtualRows`** at the top of `apps.rs`:
   ```rust
   use crate::virtual_rows::VirtualRows;
   ```

4. **Keep everything outside `VirtualRows::show` unchanged**: the `ui.scope` wrapper,
   `item_spacing.y = 0.0`, the wide-mode column header block, and the `app_list` function
   signature.

5. **No changes to `apps_service.rs`** are required for the minimal fix; storing
   `VirtualRows` in `Controller` is an optional optimization.

### Pseudo-implementation

```rust
fn app_list(ui, apps, indices, icons, selected, details) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let width = ui.available_width();
        let wide = width >= 850.0;
        let height = if wide { 70.0 } else { 94.0 };

        // ... wide-mode header unchanged ...

        // REPLACED: manual clip math + two spacer allocations + loop
        let rows = VirtualRows::from_heights(std::iter::repeat_n(height, indices.len()));
        rows.show(ui, "app_list", |n, ui| {
            let source = indices[n];
            let Some(app) = apps.get(source) else { return; };
            // ... all existing per-row rendering logic, unchanged ...
        });
    });
}
```

## Testing Strategy

### Validation Approach

The testing strategy follows a two-phase approach: first, surface counterexamples that
demonstrate the bounce on unfixed code, then verify the fix eliminates the bounce while
preserving row rendering.

### Exploratory Bug Condition Checking

**Goal**: Surface counterexamples that demonstrate the total content height changes between
frames as `clip.top()` shifts — confirming the bug exists before applying the fix.

**Test Plan**: Render `app_list` via egui's headless `Context` in two consecutive frames
where the clip rect top differs slightly (simulating deceleration), and assert that the
total allocated height is the same in both frames. Run on UNFIXED code — expect FAILURE.

**Test Cases**:
1. **Clip shift test**: Render a list of 100 items at `clip.top() = 0.0`, then at
   `clip.top() = 0.4` (sub-pixel shift). Assert both frames allocate `100 * height` px.
   (will fail on unfixed code because `first` may change by 1)
2. **Deceleration simulation**: Feed 20 frames with linearly decreasing scroll offsets
   to simulate coast-down. Assert allocated height is constant across all frames.
   (will fail on unfixed code)

**Expected Counterexamples**:
- Total allocated height differs between frame N and frame N+1 by exactly `height` (70 or
  94 px) when a fractional clip shift causes `first` to increment or decrement.

### Fix Checking

**Goal**: Verify that for all frames where the bug condition holds, the fixed function
allocates a constant total height.

**Pseudocode:**
```
FOR ALL frame WHERE isBugCondition(frame) DO
  height_allocated := render_app_list_fixed(frame)
  ASSERT height_allocated = frame.count * row_height
END FOR
```

### Preservation Checking

**Goal**: Verify that for all non-buggy inputs, the fixed function produces the same
rendered rows as the original.

**Pseudocode:**
```
FOR ALL frame WHERE NOT isBugCondition(frame) DO
  ASSERT render_app_list_original(frame) = render_app_list_fixed(frame)
END FOR
```

**Testing Approach**: Property-based testing is recommended because:
- Row rendering has many parameters (wide/narrow, icon present/absent, selected/unselected,
  alternating index, various name/publisher/size combinations).
- PBT generates many input combinations automatically, catching edge cases.
- Preservation is a universal statement ("for all non-buggy inputs") that maps directly to
  a PBT property.

**Test Cases**:
1. **Row count preservation**: Observe which row indices are rendered by the original for
   a static clip, then verify the fixed version renders the same indices.
2. **Selection state preservation**: Click a row in the fixed version; assert `selected`
   is updated identically to the original.
3. **Wide/narrow layout**: Render at width 900 and 600; assert the correct header
   visibility and row heights in both versions.
4. **Empty list**: Verify neither version renders any rows or spacers.

### Unit Tests

- Test that `VirtualRows::from_heights(repeat_n(70.0, N)).show(...)` allocates exactly
  `N * 70.0` px total, regardless of clip position.
- Test that clicking a row calls the `row` closure for the correct index.
- Test the wide/narrow height switch (70 vs 94 px).

### Property-Based Tests

- Generate random `(count, scroll_offset, clip_height)` triples; assert total height is
  always `count * row_height` for both wide and narrow modes (Property 1).
- Generate random app lists and clip positions; assert the set of rendered row indices
  matches `VirtualRows`'s partition logic (Property 2).

### Integration Tests

- Full `apps_page` render with mock inventory; scroll to bottom, assert no layout
  inconsistency across 30 frames of simulated deceleration.
- Resize window from 900 px to 600 px mid-scroll; assert no height jump.
