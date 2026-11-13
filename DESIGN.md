# ALTRunNext desktop design

Windows keyboard launcher. The panel shows the selected name, search input, numbered alias/name rows and CMD. The user can type and launch as soon as the panel appears.

## Runtime owner

`src/theme.rs` owns runtime colors. `src/theme_ui.rs` paints native controls. `src/native.rs` owns the launch panel and its input focus. The UI embeds Simplified Chinese and English. Microsoft YaHei UI is the default system font, and Consolas is used for CMD.

## Visual values

420 DIP panel, 22 DIP rows, maximum ten rows by default. Short result lists shrink. The input well is 30 DIP. Panel text is configurable, and the command preview is 12 DIP. A narrow blue rail marks the current selection.

Light mode uses window #F5F7FB, paper #FFFFFF, text #172535, secondary text #4F6074, selection #DCEAFA, and accent #245C99. Dark mode uses window #191E25, control surface #222932, input #171D24, text #F0F4F8, secondary text #B4C0CE, selection #283D55, and accent #9AC7FF. Windows high contrast uses system colors and suppresses wallpaper. User colors and fonts apply to the launch panel when saved.

## Behavior

No debounce or animation. Native input handles caret, selection, Unicode and IME. IME Enter must commit composition without launching. Up/down and Tab choose a result; Esc clears then hides. Empty results show a short visible message. Errors retain the window and query for recovery. Single click selects and keeps typing focus, double click launches. Alt+R opens the panel by default. A context menu provides hide and exit.

Parameter commands request a value in the same input field. Esc returns to search. Context and window templates resolve on the execution worker. Release builds launch selected entries by default. `--safe-mode` displays results without executing them.

## Shortcut manager

Alt+S or the tray opens the shortcut manager. It shares the settings window's fixed 1102 × 721 DIP client size, caption, rounded corners and footer proportions. Both windows open centered on the screen under the pointer. The title bar supports moving the window. Add offers manual entry or browsing for a file or folder. Existing notes can be edited. The footer places Try opening next to Save and applies it to the selected draft. Tools offers a missing-path check, a choice of list display order, and shortcut import and export. Draft editing, validation and confirmations follow UX-CONTRACT.md. The manager runs on a separate UI thread.

Cloud native interaction regression for input focus, empty state, parameter cancellation and composition; pure launch parsing tests; cloud synthetic UI screenshots at 100% and 150% DPI, including long Chinese labels and no results. No personal content in screenshots or CI. User judges actual feel; screenshot timing is not an acceptance gate.

## Complete settings and maintenance

Settings use a native sidebar for General, Hotkeys, Search and Launch, Appearance, Fonts and Colors, and Window Position. The sidebar, window and footer share the same dimensions across pages. The content uses 15 DIP text with a larger page title. General groups startup, tray, File Explorer, popup sound and language. The fixed footer groups Reset, Import and Export on the left and Close and Save on the right. Switches accept clicks on the switch and Space when focused. Settings use native focus, accessible checkbox state, owner-drawn controls and themed dialogs. Compare normalized form values to the saved snapshot when closing.

Appearance and Fonts and Colors show a live launcher preview with sample commands. Background images use proportional cropping and a strength control capped at 34% image opacity. The search field, results and CMD use solid surfaces. Image decoding, downsampling and tinting run off the UI thread, and the launcher paints from the cached bitmap. Saving a language choice updates the launcher, tray and newly opened windows.

The manager reveals the parameter format only when a shortcut asks for input at launch. Document shortcuts offer an open-with choice. Explorer opens a compact quick-add dialog, with a scrollable table for multiple items. New file-system targets store absolute paths independently of command text. Lists expose text to native accessibility. Windows fit the active monitor's work area. The launcher can optionally remember a manually dragged position.

The launcher uses a two-column result list with a configurable visible row count. Numeric edits share one rounded field with inset up and down arrows. The image strength slider and its numeric field stay in sync.
