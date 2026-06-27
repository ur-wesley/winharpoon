# WinHarpoon

Harpoon-style window switcher for Windows. Runs in the system tray and switches focus via global hotkeys.

## Features

- **Fuzzy launcher** (`Win+K` by default) — type to filter open windows, Enter to focus
- **App launcher** (`Alt+double-click` or **double-tap Alt** by default) — favorites menu + fuzzy search over installed programs; star to favorite, assign per-app hotkeys
- **Same-app cycle** (`Win+Alt+\``) — cycle between windows of the same executable (e.g. multiple VS Code instances)
- **Marked slots** (`Win+Alt+Shift+1..9` to mark, `Win+Alt+1..9` to jump) — pin up to 9 windows
- **Marks switcher** (`Win+Alt+M` by default) — hold Win+Alt to open a visual overlay of marked windows, tap M to cycle, release Win or Alt to switch
- **Mark cycling** (`Win+Alt+]` / `Win+Alt+[`) — quick blind jump between filled mark slots (no overlay)
- **Configurable hotkeys** — edit `%APPDATA%\winharpoon\config.toml` or use tray → Settings
- **Conflict warnings** — toast notification + tray tooltip when a hotkey is already registered by another app

## Build

```powershell
cd C:\Arbeit\winharpoon
cargo build --release
.\target\release\winharpoon.exe
```

## Install

Build the Windows installer (requires [Inno Setup 6](https://jrsoftware.org/isinfo.php)):

```powershell
.\scripts\build-installer.ps1
```

This produces `dist\WinHarpoon-Setup-1.1.0.exe`. The installer offers a **Start WinHarpoon when Windows starts** option during setup. You can change this anytime in tray → **Settings** → **General**.

## Default hotkeys

| Action | Default |
|--------|---------|
| Fuzzy launcher | `Win+K` |
| App menu | `Alt+double-click` or double-tap `Alt` (configurable scope in Settings) |
| Same-app next | `Win+Alt+Grave` |
| Same-app prev | `Win+Alt+Shift+Grave` |
| Marks switcher | `Win+Alt+M` (hold Win+Alt to open, M to cycle, release to confirm) |
| Mark slot N | `Win+Alt+Shift+N` |
| Jump slot N | `Win+Alt+N` |
| Next mark | `Win+Alt+BracketRight` |
| Prev mark | `Win+Alt+BracketLeft` |

## Config

Config: `%APPDATA%\winharpoon\config.toml`  
Marks: `%APPDATA%\winharpoon\marks.toml`  
Favorites: `%APPDATA%\winharpoon\favorites.toml`  
Logs: `%APPDATA%\winharpoon\winharpoon.log`

After changing hotkeys in Settings, click **Save** or use tray → **Reload config**.

If a hotkey fails to register, you'll get a toast and the binding shows in red in Settings. Remap the conflicting chord — Windows does not report which app owns it.

The marks switcher chord is handled by a low-level keyboard hook (not `RegisterHotKey`), so it won't appear in conflict detection but is still configurable in Settings.

## Caveats

- Cannot focus elevated (admin) windows from a non-elevated WinHarpoon instance
- Some fullscreen apps block foreground changes
- Hotkey conflicts with other tools (PowerToys, etc.) require remapping in Settings
