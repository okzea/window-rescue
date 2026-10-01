# Window Rescue

A tiny Windows tray app that brings lost windows back on screen.

Unplugged a monitor, changed resolution, or got a dialog stuck off the edge? Press
**Ctrl+Alt+Home** and the active window jumps to the center of the screen under your mouse.
Click the tray icon and every off-screen window comes back.

- One ~280 KB executable. No installer, no runtime, no background service.
- No network access, no telemetry, no files written. Two registry values, that's all.
- Windows 10 and 11, multi-monitor and mixed-DPI setups.

## Use

| Action | What happens |
|---|---|
| **Ctrl+Alt+Home** | The active window jumps to the center of the screen under the mouse, shrunk if it does not fit. Maximized windows stay maximized. |
| **Click the tray icon** | Every window whose title bar is off screen comes back. |
| **Right-click the tray icon** | Change the shortcut, start with Windows, about, exit. |

Windows on other virtual desktops, minimized windows and tiny helper windows are never touched.

### Command line

```
WindowRescue.exe --rescue-all      bring back every off-screen window, then exit
WindowRescue.exe --rescue-active   bring the active window onto the screen under the mouse, then exit
```

Handy to trigger Window Rescue from another launcher or keyboard tool.

## Install

Download `WindowRescue.exe` from the [latest release](https://github.com/okzea/window-rescue/releases/latest),
put it anywhere, and run it. Right-click the tray icon and tick **Start with Windows** to keep it around.

To uninstall, untick **Start with Windows**, exit, and delete the file.

## Limits

Windows does not let a normal app move a window that runs as administrator. Window Rescue
tells you when that happens; start it as administrator too if you need to rescue those.

## Privacy

Window Rescue reads window positions on your own PC and nothing else. It never connects to the
internet and collects no data. It stores its settings in the registry:

- `HKCU\Software\WindowRescue` — `ShortcutModifiers`, `ShortcutKey`
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\WindowRescue` — only while **Start with Windows** is on

## Build

Needs [Rust](https://rustup.rs) (either the MSVC or the GNU toolchain) and PowerShell.

```powershell
./build.ps1             # dist/WindowRescue.exe, with icon, manifest and version info
./build.ps1 -Install    # also copy to %LOCALAPPDATA%\Programs\WindowRescue and start it
```

`cargo build --release` alone works too; the exe just has no icon or version info.
`assets/make-icon.ps1` redraws the icon.

## License

[MIT](LICENSE)
