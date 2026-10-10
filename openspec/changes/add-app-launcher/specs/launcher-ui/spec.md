## Purpose

Defines the touch-friendly home screen of termdeck: pinned apps, recent apps and search. It covers keyboard and touch input, how pins and recent apps persist, and the visual style.

## ADDED Requirements

### Requirement: Home screen layout
The home screen SHALL show, from top to bottom: a grid of pinned apps, a list of recent apps, and a search prompt. With no pinned apps, the pinned area SHALL show a short hint explaining how to pin an app.

#### Scenario: Home screen with pins
- **WHEN** termdeck starts with Chrome and Settings pinned and Morphe recently used
- **THEN** Chrome and Settings appear as buttons in the pinned grid, Morphe appears in the recent list, and an empty search prompt is shown

#### Scenario: Home screen without pins
- **WHEN** termdeck starts with no pinned apps
- **THEN** the pinned area shows a hint on how to pin an app

### Requirement: Recent apps list
The recent list SHALL show up to 8 apps, most recently launched first. Pinned apps SHALL NOT be repeated in the recent list. Apps that are no longer installed SHALL be hidden.

#### Scenario: Pinned app not duplicated
- **WHEN** Chrome is pinned and was the most recent launch
- **THEN** Chrome appears only in the pinned grid, not in the recent list

### Requirement: Search on type
Typing a printable character SHALL enter search mode. In search mode the pinned grid and recent list are replaced by a list of all indexed apps, fuzzy-matched against the query by label and package name and ranked by match quality. Clearing the query or pressing Escape SHALL return to the home screen.

#### Scenario: Typing starts a search
- **WHEN** the user types `yt` on the home screen
- **THEN** the screen shows matching apps, such as "YT Music Morphe" and "YouTube Morphe", with the best match first

#### Scenario: Escape returns home
- **WHEN** the user presses Escape in search mode
- **THEN** the query is cleared and the home screen is shown

#### Scenario: Enter launches top result
- **WHEN** the user presses Enter in search mode with at least one result
- **THEN** the selected result (the first one by default) is launched

### Requirement: Keyboard navigation
The user SHALL be able to move the selection with the arrow keys, launch the selected app with Enter, and quit with Ctrl+C. Escape on the home screen with an empty query SHALL quit.

#### Scenario: Arrow keys move selection
- **WHEN** the user presses Down in search mode
- **THEN** the selection moves to the next result

### Requirement: Touch input
Tapping an app button or row SHALL launch that app. Swiping vertically over a list SHALL scroll it. No action SHALL require a long press.

#### Scenario: Tap launches
- **WHEN** the user taps the Chrome button in the pinned grid
- **THEN** Chrome is launched

#### Scenario: Swipe scrolls
- **WHEN** the search results are longer than the screen and the user swipes up over them
- **THEN** the list scrolls to show further results

### Requirement: Pin and unpin
Each row in the search results and the recent list SHALL have a tappable pin toggle that shows whether the app is pinned. Activating it SHALL pin or unpin the app without launching it. The toggle SHALL also be reachable from the keyboard for the selected row.

#### Scenario: Pin from search
- **WHEN** the user searches for "Password Store" and taps its pin toggle
- **THEN** Password Store is added to the end of the pinned grid and the app is not launched

#### Scenario: Unpin
- **WHEN** the user taps the pin toggle of an app that is already pinned
- **THEN** the app is removed from the pinned grid

### Requirement: State persists across runs
Pinned apps (in order) and the recent-apps list SHALL be saved to disk when they change and SHALL be restored on the next start.

#### Scenario: Pins survive restart
- **WHEN** the user pins Chrome, quits, and starts termdeck again
- **THEN** Chrome is in the pinned grid

### Requirement: Return to home after launch
After a successful launch, termdeck SHALL keep running, clear the query and show the home screen. When the user switches back to Termux, the launcher SHALL be ready to use.

#### Scenario: Back to launcher
- **WHEN** the user launches an app from search and later switches back to Termux
- **THEN** termdeck shows the home screen with an empty query

### Requirement: Visual style
Buttons and panels SHALL use sharp (square) corners. App buttons in the pinned grid SHALL be drawn as solid filled blocks with a contrasting label. The selected item SHALL be visibly distinct. Every touch target SHALL be at least 3 rows tall.

#### Scenario: Pinned buttons are filled
- **WHEN** the home screen is drawn with pinned apps
- **THEN** each pinned app is a filled block at least 3 rows tall with square corners and its label inside

### Requirement: Narrow screens
The layout SHALL adapt to the terminal width. The number of pinned-grid columns SHALL be derived from the width, and long labels SHALL be truncated instead of wrapping.

#### Scenario: Phone portrait width
- **WHEN** the terminal is about 45 columns wide
- **THEN** the pinned grid shows at least 2 columns and no label overflows its button
