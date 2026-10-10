## Purpose

Starts an indexed app's launcher activity quickly and tells the user whether the launch succeeded, including the actionable reason when it did not.

## ADDED Requirements

### Requirement: Launch an app
The system SHALL start the selected entry's launcher activity. Launching an app that is already running SHALL bring its existing task to the front.

#### Scenario: Launch succeeds
- **WHEN** the user selects Chrome
- **THEN** Chrome comes to the foreground

#### Scenario: App already running
- **WHEN** the user selects an app whose task is already running
- **THEN** that task is brought to the front and the launch counts as successful

### Requirement: Launch latency
The time from the user selecting an app to the launch request completing SHALL be under 150 ms on the reference device (Pixel 5a, Android 14). This excludes the target app's own startup time.

#### Scenario: Warm launch timing
- **WHEN** the user launches an app that is already running
- **THEN** the launch request completes in under 150 ms

### Requirement: Launch failure detection
The system SHALL decide whether a launch succeeded from the launcher command's output, not only from its exit status. When a launch fails, the system SHALL show a status message with the reason and SHALL NOT record the app as recently used.

#### Scenario: Overlay permission missing
- **WHEN** Termux lacks the "Display over other apps" permission and the user launches an app
- **THEN** the status line says that the permission is required and how to grant it, and the app is not added to recent apps

#### Scenario: Activity no longer exists
- **WHEN** the user launches an entry whose app was uninstalled after the last index
- **THEN** a failure message is shown and a refresh of the index is started

### Requirement: Recent apps record
After a successful launch, the system SHALL move the app to the front of the recent-apps list.

#### Scenario: Launch updates recent apps
- **WHEN** the user successfully launches YT Music
- **THEN** YT Music is the first entry in the recent-apps list
