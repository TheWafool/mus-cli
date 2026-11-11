# mus

A terminal UI for [yt-dlp](https://github.com/yt-dlp/yt-dlp): paste links or search, pick options, watch the queue.

## Requirements

- `yt-dlp` and `ffmpeg` on `PATH` (Arch: `sudo pacman -S yt-dlp ffmpeg`)
- Recommended: `deno` — yt-dlp needs a JS runtime for full YouTube support
- Optional: `notify-send` (notifications), `wl-clipboard`/`xclip` (Ctrl+V paste)

## Run

```sh
cargo run --release              # or: cargo install --path .  →  mus-cli
mus-cli URL...                   # queue links straight away
```

## Tabs

1. **Download**: paste one or more links (Enter queues them; plain text searches). Has quick options, live activity, and a command preview.
2. **Search**: YouTube / YouTube Music / SoundCloud. Space selects, Enter queues.
3. **Queue**: parallel downloads with progress, speed and ETA. Cancel, retry, remove, reorder, pause, and open the folder.
4. **Options**: every setting, with help text and presets. Saved to `~/.config/mus-cli/config.toml`.

Press `?` in the app for all keys. Options are snapshotted when a link is queued, so changing them later doesn't affect queued items.
