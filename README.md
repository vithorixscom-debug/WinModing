# WinModing

**A Modrinth-based Minecraft launcher with CurseForge.**

WinModing is a Windows desktop fork of the Modrinth App UI (Theseus / AstralRinth), with Modrinth and CurseForge in one catalog. No API keys to paste. Microsoft login uses the public Minecraft client.

<p align="center">
  <img src="src-tauri/icons/icon-256.png" alt="WinModing" width="128" height="128">
</p>

![WinModing](docs/screenshots/mods.png)

## What you get

- **Modrinth + CurseForge** in the same Browse tab (or one site at a time)
- Isolated profiles: mods, saves, and config stay per instance
- Fabric, Quilt, Forge, and NeoForge (Fabric/Quilt install is wired today)
- Microsoft, Ely.by, and offline accounts — no Azure app setup
- Discord shows **Playing Minecraft** only while the game is running
- Playtime on the profile, in hours
- Optional Fabric auto-optimize pack when you create a profile
- No telemetry, analytics, or ads

## Requirements

- Windows 10 or 11
- [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)

To build from source: Rust 1.77+ and Node.js 20+.

## Build

```bash
npm install --legacy-peer-deps
npm run build
cd src-tauri
cargo build --release
```

Development:

```bash
npm install --legacy-peer-deps
npm run tauri dev
```

The release binary is `WinModing.exe`. Bundled installers: `npm run tauri build`.

## User data

| Path | Contents |
| --- | --- |
| `%LOCALAPPDATA%\WinModing` | Instances, Java runtimes, metadata, database |
| `%LOCALAPPDATA%\WinModing\instances\<id>` | One Minecraft profile |

## Disclaimer

WinModing is not affiliated with Mojang, Microsoft, Modrinth, CurseForge, or AstralRinth. Minecraft is a trademark of Mojang Synergies AB.

## License

WinModing is free software licensed under the [GNU General Public License v3.0](LICENSE).

The UI under `packages/` is vendored from Modrinth (Theseus) / AstralRinth and is also GPL-3.0.
