# ALTRun desktop design

Windows keyboard launcher for an experienced ALTRun user. Its job is to appear immediately, accept a shortcut and launch it. Follow the original title → input → numbered alias/name rows → CMD structure. No branding, cards, icons per result, animation, blur or extra dashboards.

## Runtime owner

`src/theme.rs` owns runtime values; this file records their intent. `src/native.rs` owns native EDIT/LISTBOX behavior, painting and focus. Chinese UI, system-installed Microsoft YaHei UI; command preview uses Consolas. No downloaded fonts or browser runtime.

## Visual values

420 DIP panel, 22 DIP rows, maximum ten rows; short lists shrink. 10–12 DIP outside spacing, 30 DIP input well. Body 13 DIP, input 16 DIP, title 13 DIP semibold, command preview 11 DIP. Small native rounded window corners where supported. The selection's narrow blue rail connects the numbered shortcut to its row without adding visual clutter.

Palette: surface #F5F7FA; input/list #FFFFFF; main text #202C3A; secondary text #617084; selected #E7EFF8; accent #315F88; border #C6D0DC. Error text #A52C36. Light, dark and system palettes plus the Windows high-contrast override are implemented. User colors and native font selection are applied when settings are saved.

## Behavior

No debounce or animation. Native input handles caret, selection, Unicode and IME. IME Enter must commit composition without launching. Up/down and Tab choose a result; Esc clears then hides. Empty results show a short visible message. Errors retain the window and query for recovery. Single click selects and keeps typing focus, double click launches. Ctrl+Alt+R remains separate from the original hotkeys during coexistence. A context menu provides hide and exit.

Launch is explicitly enabled for the user trial; existing measurement/self-test modes remain non-executing. Parameter commands request a value in the same input field, Esc returns to search. Context and window templates resolve on the execution worker; unsupported forms fail visibly. Full capability routing and explicit compatibility differences are recorded in docs/compatibility.md. Release builds launch selected entries by default; --safe-mode and Debug defaults retain non-executing fixtures.

## Verification

## Shortcut manager

Alt+S or the tray opens a separate on-demand native window. Keep the launch panel unchanged. The manager uses a 760 × 420 DIP client area: left alias/name list; right alias, name, command and native parameter-type dropdown; bottom Add/Delete/Import, status, Save and Close. Use the same colors/type roles and visible native scrollbars. Text is Chinese, commands remain copyable. Draft editing, validation, backup persistence and close/delete confirmations follow UX-CONTRACT.md. The manager runs on its own UI thread so explicit disk operations do not block the resident launcher.

Cloud native interaction regression for input focus, empty state, parameter cancellation and composition; pure launch parsing tests; cloud synthetic UI screenshots at 100% and 150% DPI, including long Chinese labels and no results. No personal content in screenshots or CI. User judges actual feel; screenshot timing is not an acceptance gate.

## Complete settings and maintenance

Settings use native tabs with fields visible together by purpose: General, Hotkeys, Search and Launch, Appearance, Fonts and Colors, Window Position, and Advanced. General puts startup, tray and Send to together. Hotkeys show all three bindings, with an explicit Record button and a Clear action for each. Use native checkboxes, edit fields, dropdowns and file/font/color pickers, with the same surface, paper and text palette as the manager. Keep save and validation status above the fixed footer. Compare normalized form values to the saved snapshot when closing. Language changes take effect next startup.

The manager adds work directory, enabled/type controls and menu-only maintenance actions. Keep optional maintenance and UI buttons outside the default launch path. Both lists expose text to native accessibility. Persist manager position/selection; fit windows to work areas, including menu/non-client bounds.
