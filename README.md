# mus

A polished terminal UI for [yt-dlp](https://github.com/yt-dlp/yt-dlp). Paste links or search, pick your format, and watch the queue. Works with the keyboard and the mouse.

- **Download**: paste one or many links (Enter queues them; plain text searches). Quick options, live activity panel, and a preview of the exact yt-dlp command.
- **Search**: YouTube, YouTube Music, or SoundCloud. Select several results and queue them in one go.
- **Queue**: parallel downloads with progress, speed, ETA, playlist position, and post-processing steps. Cancel, retry, reorder, pause, and open the folder.
- **Options**: 38 settings with help text and presets. Covers audio/video, format and bitrate, resolution and codec, tags, square cover art, chapters, subtitles, SponsorBlock, playlist ranges, download archive, browser cookies, rate limit, proxy, and more. Saved automatically.

## Install

mus needs **yt-dlp** and **ffmpeg** at runtime (see [Dependencies](#dependencies)).

### Arch Linux (AUR)

```sh
yay -S mus-cli        # latest release
yay -S mus-cli-git    # latest commit
```

(or `paru -S …`, or manually: `git clone https://aur.archlinux.org/mus-cli.git && cd mus-cli && makepkg -si`)

### Prebuilt binaries (Linux, macOS, Windows)

Download the archive for your platform from [Releases](https://github.com/TheWafool/mus-cli/releases), extract it, and put `mus-cli` (or `mus-cli.exe`) somewhere on your `PATH`:

```sh
# Linux / macOS example
tar xzf mus-cli-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
install -Dm755 mus-cli-v0.1.0-x86_64-unknown-linux-gnu/mus-cli ~/.local/bin/mus-cli
```

On macOS, if Gatekeeper blocks it: `xattr -d com.apple.quarantine ~/.local/bin/mus-cli`.

### With Cargo (any OS with Rust)

```sh
cargo install --git https://github.com/TheWafool/mus-cli
```

This installs to `~/.cargo/bin` (make sure it's on your `PATH`). Requires Rust 1.88+ (`rustup update`).

### From source

```sh
git clone https://github.com/TheWafool/mus-cli
cd mus-cli
cargo build --release
install -Dm755 target/release/mus-cli ~/.local/bin/mus-cli
```

## Dependencies

| Platform | Command |
| --- | --- |
| Arch | `sudo pacman -S yt-dlp ffmpeg deno` |
| Debian / Ubuntu | `sudo apt install ffmpeg pipx && pipx install yt-dlp` (the apt yt-dlp is usually too old) |
| Fedora | `sudo dnf install yt-dlp ffmpeg` |
| openSUSE | `sudo zypper install yt-dlp ffmpeg` |
| macOS (Homebrew) | `brew install yt-dlp ffmpeg deno` |
| Windows (winget) | `winget install yt-dlp.yt-dlp Gyan.FFmpeg DenoLand.Deno` |
| Windows (Scoop) | `scoop install yt-dlp ffmpeg deno` |
| Any (Python) | `pip install -U yt-dlp` |

**deno** is recommended because yt-dlp needs a JavaScript runtime for full YouTube support. Optional on Linux: `libnotify` (notifications), `wl-clipboard` or `xclip` (Ctrl+V paste), `xdg-utils` (open folders).

## Usage

```sh
mus-cli                 # open the UI
mus-cli URL [URL...]    # queue links straight away
mus-cli --help
```

| Key | Action |
| --- | --- |
| `Tab` / `1`–`4` | switch tabs |
| `Enter` | download link / search / queue selection |
| `Esc` | leave a text field |
| `←` `→` `Space` | change an option |
| `p` / `r` | presets / reset option |
| `c` `r` `d` `o` | cancel / retry / remove / open folder (queue) |
| `?` | all keys |
| `q`, `Ctrl+C` | quit |

**Mouse**: click tabs, rows, values (right-click steps back), and the key hints in the footer. Scroll lists with the wheel. Hold **Shift** to select text in your terminal.

Config lives at `~/.config/mus-cli/config.toml` (Linux), `~/Library/Application Support/mus-cli/` (macOS), or `%APPDATA%\mus-cli\` (Windows). Your normal yt-dlp config file is respected too. Options are snapshotted when a link is queued, so changing them later doesn't affect queued items.

## License

MIT
