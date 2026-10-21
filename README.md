# WinModing

Windows Minecraft launcher built with Rust, Tauri v2, and Vue 3.

WinModing keeps each profile in its own folder, talks to Modrinth and CurseForge from one catalog, and can show Discord Rich Presence while you play Minecraft.

<p align="center">
  <img src="src-tauri/icons/icon-256.png" alt="WinModing" width="128" height="128">
</p>

![WinModing](docs/screenshots/mods.png)

## Features

- Search and install mods from **Modrinth**, **CurseForge**, or both
- Fabric, Quilt, Forge, and NeoForge profiles (Fabric/Quilt install is wired today)
- Isolated instances: mods, saves, and config stay per-profile
- Microsoft / Ely.by / offline accounts (Microsoft sign-in opens a login window; no Azure app is required)
- Discord Rich Presence only while Minecraft is running (`Playing Minecraft`)
- Playtime on the instance profile, in hours
- Optional Fabric auto-optimize pack when you create a profile

## Requirements

- Windows 10 or 11
- [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/)
- Rust 1.77 or newer
- Node.js LTS (20+)

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

The release binary is `src-tauri/target/release/WinModing.exe`. Bundled installers are produced with `npm run tauri build`.

CI never injects API keys. A CurseForge key compiled into the binary can be extracted from a public release.

## Keys and setup

Do not commit real values. Copy `.env.example` to `.env` only on your machine. All `WINMODING_*` values are read at **runtime**.

| What | Environment | File under `%LOCALAPPDATA%\WinModing\meta\` |
| --- | --- | --- |
| CurseForge API | `WINMODING_CF_KEY` | `curseforge.key` (also Settings → Catalog) |
| Optional Azure public client id | `WINMODING_MS_CLIENT_ID` | `microsoft.client_id` |
| Discord image (https URL) | `WINMODING_DISCORD_IMAGE` | `discord.image` |
| Discord application id | `WINMODING_DISCORD_CLIENT_ID` | `discord.client_id` |

Microsoft login uses the public Minecraft Xbox client and a desktop redirect by default. Set `WINMODING_MS_CLIENT_ID` only if you want your own Azure loopback app.

CurseForge search needs **your** Eternal API key. Paste it in Settings. WinModing does not ship a key.

Discord: while Minecraft is running the profile shows **Playing Minecraft**. Voice overlay uses Minecraft's Discord application (not Modrinth). Optional custom artwork: host `docs/branding/discord-presence.png` (for example from GitHub Releases) and put the https URL in `discord.image`. Put your own Discord application id in `discord.client_id` if you want a custom voice icon; Modrinth/Theseus ids are ignored.

## User data

| Path | Contents |
| --- | --- |
| `%LOCALAPPDATA%\WinModing` | Instances, Java runtimes, metadata, database |
| `%LOCALAPPDATA%\WinModing\instances\<id>` | One Minecraft profile |
| `%LOCALAPPDATA%\WinModing\meta` | Libraries, assets, versions, API keys |

## Disclaimer

WinModing is not affiliated with Mojang, Microsoft, Modrinth, CurseForge, or AstralRinth. Minecraft is a trademark of Mojang Synergies AB.

## License

WinModing is free software licensed under the [GNU General Public License v3.0](LICENSE).

The UI under `packages/` is vendored from Modrinth (Theseus) / AstralRinth and is also GPL-3.0.
