# Bugfix Requirements Document

## Introduction

The Apps page (Nyaa) contains a scrollable list of installed applications rendered with a
manual virtual scrolling implementation in `app_list` (`apps/rigpulse/src/pages/apps.rs`).
After the user stops scrolling, the list briefly jumps back — a scroll bounce / jitter
artifact. The bounce occurs because the virtual row allocations depend on `clip_rect`, whose
`top` edge changes every frame during deceleration, causing the total allocated height to
fluctuate, which in turn causes egui to adjust the scroll offset, creating a feedback loop.

## Bug Analysis

### Current Behavior (Defect)

1.1 WHEN the user scrolls the apps list and then releases the scroll input THEN the system
briefly scrolls the list back toward the previous position (bounce / jitter) instead of
stopping smoothly.

1.2 WHEN `clip_rect.top()` shifts by even a fractional pixel between frames during scroll
deceleration THEN the system recalculates `first`/`last` visible rows and re-allocates
the spacer blocks with slightly different heights, causing the total content height to
change across frames.

1.3 WHEN the total content height changes across frames THEN the system triggers an egui
scroll offset correction, which itself causes another clip change, perpetuating the bounce
loop until deceleration completes.

### Expected Behavior (Correct)

2.1 WHEN the user scrolls the apps list and then releases the scroll input THEN the system
SHALL stop at the released scroll position without any visible jump or bounce.

2.2 WHEN `clip_rect.top()` shifts between frames during scroll deceleration THEN the system
SHALL maintain a constant total content height so that egui has no reason to adjust the
scroll offset.

2.3 WHEN the apps list is rendered with any number of entries THEN the system SHALL
pre-allocate the full content height once per frame before rendering individual rows, using
`VirtualRows::show`, which calls `ui.allocate_space(total)` as its first action.

### Unchanged Behavior (Regression Prevention)

3.1 WHEN an apps list with a non-empty filtered result is displayed THEN the system SHALL
CONTINUE TO render only the rows currently visible in the viewport (virtual scrolling is
preserved).

3.2 WHEN the layout switches between wide (≥ 850 px) and narrow modes THEN the system SHALL
CONTINUE TO render rows at the correct height (70 px wide / 94 px narrow) and display the
column header row only in wide mode.

3.3 WHEN the user clicks a row or the Detail button THEN the system SHALL CONTINUE TO update
`selected` and `details_open` state correctly.

3.4 WHEN an app icon is available in the icon cache THEN the system SHALL CONTINUE TO paint
it at the correct position within the row.

3.5 WHEN the window is resized THEN the system SHALL CONTINUE TO reflow the list to the
new width without visual artifacts.
