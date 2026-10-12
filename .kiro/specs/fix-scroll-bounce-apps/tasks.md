# Implementation Plan

- [-] 1. Write bug condition exploration test
  - **Property 1: Bug Condition** — Stable Total Content Height During Scroll Deceleration
  - **CRITICAL**: This test MUST FAIL on unfixed code — failure confirms the bug exists
  - **DO NOT attempt to fix the test or the code when it fails**
  - **NOTE**: This test encodes the expected behavior — it will validate the fix when it passes after implementation
  - **GOAL**: Surface counterexamples showing that total allocated height changes between frames when `clip.top()` shifts
  - **Scoped PBT Approach**: Scope the property to the concrete failing case — a 100-item list rendered in two consecutive headless frames where `clip.top()` shifts by 0.4 px (simulating one step of scroll deceleration)
  - Add a `#[test]` in `apps/rigpulse/src/pages/apps.rs` (or a new `tests` module in that file):
    - Create an `egui::Context`, install the theme, and build a minimal `apps` slice + `indices` vec (e.g. 100 entries, all identical placeholder `Application` values)
    - Frame 1: render `app_list` inside a `ScrollArea` with `screen_rect` top = 0.0; capture `ui.min_rect().height()` as `height_frame_1`
    - Frame 2: render `app_list` with `screen_rect` shifted so that `clip.top()` differs by 0.4 px; capture `height_frame_2`
    - Assert `height_frame_1 == height_frame_2` — this assertion FAILS on unfixed code because a sub-pixel clip shift can change `first` by 1, altering total allocation by one full row height
  - Run test on UNFIXED code
  - **EXPECTED OUTCOME**: Test FAILS (confirms the bounce bug exists)
  - Document the counterexample found (e.g. "frame 1: 7000 px, frame 2: 6930 px — differ by 70 px")
  - Mark task complete when test is written, run, and failure is documented
  - _Requirements: 1.2, 1.3_

- [~] 2. Write preservation property tests (BEFORE implementing fix)
  - **Property 2: Preservation** — Row Rendering Unchanged
  - **IMPORTANT**: Follow observation-first methodology — run on UNFIXED code first to record baseline behavior
  - Observe: for a static scroll position (no clip shift), `app_list` renders the expected set of row indices; row content, selection state, and icon painting are correct
  - Write property-based tests in a `#[test]` function using `egui::Context` headless rendering:
    - **Row index coverage**: for a list of N items at a fixed clip, collect which row indices the closure was invoked for; assert the set matches what `VirtualRows::partition_point` would produce (verifying virtual culling is preserved)
    - **Selection state**: simulate a click response on row 3; assert `selected` is set to the correct app ID
    - **Wide/narrow heights**: render at available_width 900 and 600; assert `ui.min_rect().height()` equals `count * 70.0` and `count * 94.0` respectively
    - **Empty list**: render with `indices = []`; assert nothing is allocated beyond the scope container
  - Run all preservation tests on UNFIXED code
  - **EXPECTED OUTCOME**: All preservation tests PASS (establishes the baseline to protect)
  - Mark task complete when tests are written, run, and confirmed passing on unfixed code
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

- [ ] 3. Fix scroll bounce in `app_list`

  - [~] 3.1 Replace manual virtual scroll loop with `VirtualRows`
    - Add `use crate::virtual_rows::VirtualRows;` to the imports in `apps/rigpulse/src/pages/apps.rs`
    - Inside `app_list`, after the wide-mode column header block, remove:
      - The `let top = ...` / `let clip = ...` / `let first = ...` / `let last = ...` calculations
      - The `if first > 0 { ui.allocate_exact_size(...) }` spacer above
      - The `for (index, &source) in indices.iter().enumerate().take(last).skip(first)` loop
      - The `if last < indices.len() { ui.allocate_exact_size(...) }` spacer below
    - Replace with:
      ```rust
      let rows = VirtualRows::from_heights(std::iter::repeat_n(height, indices.len()));
      rows.show(ui, "app_list", |n, ui| {
          let source = indices[n];
          let Some(app) = apps.get(source) else { return; };
          // existing per-row rendering: rect from ui.max_rect(), highlight, icon,
          // cell() calls, action button child, divider hline — all unchanged
      });
      ```
    - Inside the closure, replace `rect` (previously from `allocate_exact_size`) with
      `ui.max_rect()` (the rect provided by `VirtualRows::show` to the child UI), and
      replace `response` (click sense) by adding `ui.interact(ui.max_rect(), id, Sense::click())`
      using an id derived from `app.id`
    - Keep the `ui.scope` wrapper, `item_spacing.y = 0.0`, `wide`, `height`, `columns`, and
      `cell` helper unchanged
    - _Bug_Condition: isBugCondition(frame) where frame.count > 0 AND clip.top() shifts between frames_
    - _Expected_Behavior: total allocation = indices.len() * height, constant across all frames_
    - _Preservation: Preservation Requirements from design — row rendering, selection, icons, header_
    - _Requirements: 2.1, 2.2, 2.3, 3.1, 3.2, 3.3, 3.4, 3.5_

  - [~] 3.2 Verify bug condition exploration test now passes
    - **Property 1: Expected Behavior** — Stable Total Content Height During Scroll Deceleration
    - **IMPORTANT**: Re-run the SAME test from task 1 — do NOT write a new test
    - The test from task 1 asserts `height_frame_1 == height_frame_2` across a 0.4 px clip shift
    - When this test passes it confirms that `VirtualRows` pre-allocates the full extent and eliminates the bounce
    - Run: `cargo test -p rigpulse scroll_bounce` (or the test name chosen in task 1)
    - **EXPECTED OUTCOME**: Test PASSES (confirms bug is fixed)
    - _Requirements: 2.2, 2.3_

  - [~] 3.3 Verify preservation tests still pass
    - **Property 2: Preservation** — Row Rendering Unchanged
    - **IMPORTANT**: Re-run the SAME tests from task 2 — do NOT write new tests
    - Run: `cargo test -p rigpulse app_list` (or the test names chosen in task 2)
    - **EXPECTED OUTCOME**: All preservation tests PASS (confirms no regressions in row rendering, selection, or layout)
    - Confirm all tests still pass after the fix

- [~] 4. Checkpoint — Ensure all tests pass
  - Run the full test suite: `cargo test -p rigpulse`
  - Confirm zero test failures
  - Confirm the app compiles without warnings related to the changed code
  - Ask the user if any questions arise before closing the spec
