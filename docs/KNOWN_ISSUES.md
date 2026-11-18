# Known issues — 1.0.0-rc.1

- The installer has no Authenticode signature (author decision 2026-11-17, also for 1.0.0): it is published with `SHA256SUMS.txt`, and Windows SmartScreen warns on install. Check the installer with `Get-FileHash` against `SHA256SUMS.txt` (README).
- Windows 10/11 x64 only; no macOS, Linux or ARM support in this package.
- Automatic updates are opt-in and offer only releases marked as latest on GitHub. The RC is a pre-release, so RC users install 1.0.0 by hand; automatic updates start with the first release after 1.0.0. Without the update check, the releases page address is in Settings → About (to copy; the app does not open a browser).
- Only strict random battles with full results count towards statistics. Old, one-block or incomplete replays may remain Partial and show WN8 N/A.
- Very old replay formats are not guaranteed to parse.
- Analyses planned after 1.0 are not included: what separates your wins from your losses, a local AI recap (spec 20 §11, spec 28). The Maps screen is greyed out until then.
- The comparison with the players of your battles needs data: values from 200 results of other players in a tier × class group (50–199: approximate), your median from 10 of your battles there. A new player sees "Tło od 200 wyników, masz N" for a while. The background comes only from your own battles (your servers and your level), not from a global table.
- The session, battle and week summaries are Polish only; the English version hides their cards. The battle summary has no fact about how long the tank lived (the database has no such field yet). The selection thresholds come from the research study and will be reviewed after two weeks of use.
- Technical replay metadata does not provide a complete official Polish display-name catalogue for every map; safe cleaned identifiers or “unknown” may be shown. Vehicle names come from the game's own translation files; without them a readable form of the stored code is shown.
- Network loss or unavailable XVM data disables WN8 until a valid local snapshot exists, but does not block replay tracking.
- The window has its own title bar instead of the Windows frame. Windows 11 snap layouts (the flyout when hovering the maximize button) are not available; dragging to screen edges, Win+arrow keys, double-click maximize and resizing at the edges work.
- A map without a loading screen in the game files (one on the current client) shows its smaller picture at most 1.25× on the right of headers and rows, over a blurred copy, instead of a stretched picture.
- "Find the game" checks install entries in the registry, the usual folders on fixed drives and Steam libraries. A game copied to another folder without an installer entry is not found; choose it with "Choose the game folder". Network drives are not searched.
- Diagnostics are copied to the clipboard rather than saved through a file dialog to avoid adding broad filesystem capabilities.
- The installer has not yet been verified on a second physical computer. This candidate requires a controlled installation test before it is shared.
