# Known issues — 1.0.0-beta.2

- The beta installer has no Authenticode signature: it is published as a GitHub pre-release with `SHA256SUMS.txt`, and Windows SmartScreen warns on install. Check the installer with `Get-FileHash` against `SHA256SUMS.txt` (README). Code signing and automatic updates come with the stable 1.0.0.
- Windows 10/11 x64 only; no macOS, Linux or ARM support in this package.
- No automatic application updates in the beta. Updates are installed manually.
- Only strict random battles with full results count towards statistics. Old, one-block or incomplete replays may remain Partial and show WN8 N/A.
- Very old replay formats are not guaranteed to parse.
- Analyses planned after 2.0 are not included: reference tables per tier and class, what separates your wins from your losses, a local AI recap (spec 20 §11, spec 28). The Maps screen is greyed out until then.
- Technical replay metadata does not provide a complete official Polish display-name catalogue for every map; safe cleaned identifiers or “unknown” may be shown. Vehicle names come from the game's own translation files; without them a readable form of the stored code is shown.
- Network loss or unavailable XVM data disables WN8 until a valid local snapshot exists, but does not block replay tracking.
- The window has its own title bar instead of the Windows frame. Windows 11 snap layouts (the flyout when hovering the maximize button) are not available; dragging to screen edges, Win+arrow keys, double-click maximize and resizing at the edges work.
- A map without a loading screen in the game files (one on the current client) shows its smaller picture at most 1.25× on the right of headers and rows, over a blurred copy, instead of a stretched picture.
- Diagnostics are copied to the clipboard rather than saved through a file dialog to avoid adding broad filesystem capabilities.
- The installer has not yet been verified on a second physical computer. This candidate requires a controlled installation test before it is shared.
