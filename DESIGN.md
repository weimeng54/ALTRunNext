# ALTRun desktop design

Windows keyboard launcher for an experienced ALTRun user. Its job is to appear immediately, accept a shortcut and launch it. Follow the original title → input → numbered alias/name rows → CMD structure. No branding, cards, icons per result, animation, blur or extra dashboards.

## Runtime owner

`src/theme.rs` owns runtime values; this file records their intent. `src/native.rs` owns native EDIT/LISTBOX behavior, painting and focus. Chinese UI, system-installed Microsoft YaHei UI; command preview uses Consolas. No downloaded fonts or browser runtime.

## Visual values

420 DIP panel, 22 DIP rows, maximum ten rows; short lists shrink. 10–12 DIP outside spacing, 30 DIP input well. Body 13 DIP, input 16 DIP, title 13 DIP semibold, command preview 11 DIP. Small native rounded window corners where supported. The selection's narrow blue rail connects the numbered shortcut to its row without adding visual clutter.

Palette: surface #F5F7FA; input/list #FFFFFF; main text #202C3A; secondary text #617084; selected #E7EFF8; accent #315F88; border #C6D0DC. Error text #A52C36. A high-contrast palette override remains unfinished; current styling is the light theme.

## Behavior

No debounce or animation. Native input handles caret, selection, Unicode and IME. IME Enter must commit composition without launching. Up/down and Tab choose a result; Esc clears then hides. Empty results show a short visible message. Errors retain the window and query for recovery. Single click selects and keeps typing focus, double click launches. Ctrl+Alt+R remains separate from the original hotkeys during coexistence. A context menu provides hide and exit.

Launch is explicitly enabled for the user trial; existing measurement/self-test modes remain non-executing. Parameter commands request a value in the same input field, Esc returns to search. Unsupported legacy placeholders show an error; never silently execute an altered command. Full legacy compatibility remains a separate unfinished task.

## Verification

## Shortcut manager

Alt+S or the tray opens a separate on-demand native window. Keep the launch panel unchanged. The manager uses a 760 × 420 DIP client area: left alias/name list; right alias, name, command and native parameter-type dropdown; bottom Add/Delete/Import, status, Save and Close. Use the same colors/type roles and visible native scrollbars. Text is Chinese, commands remain copyable. Draft editing, validation, backup persistence and close/delete confirmations follow UX-CONTRACT.md. The manager runs on its own UI thread so explicit disk operations do not block the resident launcher.

Cloud native interaction regression for input focus, empty state, parameter cancellation and composition; pure launch parsing tests; cloud synthetic UI screenshots at 100% and 150% DPI, including long Chinese labels and no results. No personal content in screenshots or CI. User judges actual feel; screenshot timing is not an acceptance gate.
