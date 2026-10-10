## Purpose

Maintains the list of launchable Android apps, each with its package name, launcher activity and human-readable label, so the launcher can show and search apps without querying the system on every start.

## ADDED Requirements

### Requirement: Discover launchable apps
The system SHALL list every activity that handles `android.intent.action.MAIN` with category `android.intent.category.LAUNCHER` for the current user. Each entry SHALL record the package name and the fully qualified launcher activity.

#### Scenario: Apps with a launcher entry are discovered
- **WHEN** the index is built on a device where Chrome is installed
- **THEN** the index contains an entry with package `com.android.chrome` and activity `com.android.chrome/com.google.android.apps.chrome.Main`

#### Scenario: Packages without a launcher activity are excluded
- **WHEN** the index is built
- **THEN** packages that have no launcher activity (for example runtime resource overlays) are not in the index

### Requirement: Resolve human-readable labels
The system SHALL give each entry the application label from the app's APK. If the label cannot be read, the system SHALL use the last segment of the package name as the label.

#### Scenario: Label read from APK
- **WHEN** the index is built and Chrome's APK is readable
- **THEN** the Chrome entry has the label `Chrome`

#### Scenario: Label fallback
- **WHEN** the label for package `com.example.foo` cannot be read
- **THEN** the entry has the label `foo`

### Requirement: Persistent cache
The system SHALL store the index on disk and SHALL load it on startup. Startup SHALL NOT wait for a rebuild when a cached index exists.

#### Scenario: Warm start uses cache
- **WHEN** termdeck starts and a cached index exists
- **THEN** the apps from the cache are shown without waiting for system queries

#### Scenario: Cold start without cache
- **WHEN** termdeck starts and no cached index exists
- **THEN** the system builds the index, shows a loading indicator while it builds, and saves the result to disk

### Requirement: Incremental refresh
After loading the cache, the system SHALL refresh the index in the background and update the visible list when the refresh finishes. Labels SHALL be re-read only for apps that are new or whose APK changed since the last index. Apps that were uninstalled SHALL be removed. The user SHALL be able to trigger a refresh manually.

#### Scenario: Newly installed app appears
- **WHEN** an app is installed after the last index and termdeck starts
- **THEN** the app appears in the list once the background refresh finishes

#### Scenario: Uninstalled app disappears
- **WHEN** an indexed app has been uninstalled and the refresh finishes
- **THEN** the app is no longer shown in search results or recent apps

#### Scenario: Unchanged apps are not re-read
- **WHEN** a refresh runs and no APK has changed
- **THEN** no labels are re-read from APKs

### Requirement: Index failure is reported
If app discovery fails, the system SHALL keep the last good index and SHALL show an error message that the user can see.

#### Scenario: Discovery command fails
- **WHEN** the discovery query returns an error during refresh
- **THEN** the cached apps remain visible and a status message describes the failure
