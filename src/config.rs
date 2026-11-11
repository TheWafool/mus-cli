use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Defines a fieldless enum that can be cycled through in the options form.
macro_rules! choice_enum {
    ($name:ident { $($var:ident => $label:expr),+ $(,)? } default $def:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name { $($var),+ }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),+];

            pub fn label(self) -> &'static str {
                match self { $($name::$var => $label),+ }
            }

            pub fn cycle(self, dir: i32) -> Self {
                let i = Self::ALL.iter().position(|x| *x == self).unwrap_or(0) as i32;
                let n = Self::ALL.len() as i32;
                Self::ALL[(i + dir).rem_euclid(n) as usize]
            }
        }

        impl Default for $name {
            fn default() -> Self { $name::$def }
        }
    };
}

choice_enum!(Mode { Audio => "Audio only", Video => "Video" } default Audio);

choice_enum!(AudioFormat {
    Best => "Original (no re-encode)",
    Mp3 => "MP3",
    M4a => "M4A (AAC)",
    Opus => "Opus",
    Vorbis => "Ogg Vorbis",
    Flac => "FLAC (lossless)",
    Alac => "ALAC (lossless)",
    Wav => "WAV (lossless)",
} default Mp3);

impl AudioFormat {
    pub fn arg(self) -> Option<&'static str> {
        Some(match self {
            AudioFormat::Best => return None,
            AudioFormat::Mp3 => "mp3",
            AudioFormat::M4a => "m4a",
            AudioFormat::Opus => "opus",
            AudioFormat::Vorbis => "vorbis",
            AudioFormat::Flac => "flac",
            AudioFormat::Alac => "alac",
            AudioFormat::Wav => "wav",
        })
    }

    /// Lossy formats we re-encode to, where a bitrate makes sense.
    pub fn takes_quality(self) -> bool {
        matches!(
            self,
            AudioFormat::Mp3 | AudioFormat::M4a | AudioFormat::Opus | AudioFormat::Vorbis
        )
    }
}

choice_enum!(AudioQuality {
    Best => "Best (VBR 0)",
    K320 => "320 kbps",
    K256 => "256 kbps",
    K192 => "192 kbps",
    K160 => "160 kbps",
    K128 => "128 kbps",
    K96 => "96 kbps",
} default Best);

impl AudioQuality {
    pub fn arg(self) -> &'static str {
        match self {
            AudioQuality::Best => "0",
            AudioQuality::K320 => "320K",
            AudioQuality::K256 => "256K",
            AudioQuality::K192 => "192K",
            AudioQuality::K160 => "160K",
            AudioQuality::K128 => "128K",
            AudioQuality::K96 => "96K",
        }
    }
}

choice_enum!(VideoContainer { Mp4 => "MP4", Mkv => "MKV", Webm => "WebM" } default Mp4);

impl VideoContainer {
    pub fn arg(self) -> &'static str {
        match self {
            VideoContainer::Mp4 => "mp4",
            VideoContainer::Mkv => "mkv",
            VideoContainer::Webm => "webm",
        }
    }
}

choice_enum!(MaxHeight {
    Best => "Best available",
    P2160 => "2160p (4K)",
    P1440 => "1440p",
    P1080 => "1080p",
    P720 => "720p",
    P480 => "480p",
    P360 => "360p",
} default P1080);

impl MaxHeight {
    pub fn px(self) -> Option<u32> {
        Some(match self {
            MaxHeight::Best => return None,
            MaxHeight::P2160 => 2160,
            MaxHeight::P1440 => 1440,
            MaxHeight::P1080 => 1080,
            MaxHeight::P720 => 720,
            MaxHeight::P480 => 480,
            MaxHeight::P360 => 360,
        })
    }
}

choice_enum!(VideoCodec {
    Any => "Any (best quality)",
    H264 => "H.264 (most compatible)",
    Vp9 => "VP9",
    Av1 => "AV1",
} default Any);

choice_enum!(FilenameStyle {
    Title => "Title",
    ArtistTitle => "Artist - Title",
    UploaderTitle => "Uploader - Title",
    TitleId => "Title [id]",
    Custom => "Custom template",
} default Title);

choice_enum!(Subtitles {
    Off => "Off",
    Embed => "Embed in video",
    File => "Separate files",
    Both => "Embed + separate files",
} default Off);

choice_enum!(SponsorBlock {
    Off => "Off",
    Mark => "Mark as chapters",
    Remove => "Cut segments out",
} default Off);

choice_enum!(PlaylistMode {
    Whole => "Whole playlist",
    Single => "Only the linked video",
} default Whole);

choice_enum!(Browser {
    Off => "None",
    Firefox => "Firefox",
    Chrome => "Chrome",
    Chromium => "Chromium",
    Brave => "Brave",
    Edge => "Edge",
    Vivaldi => "Vivaldi",
    Opera => "Opera",
} default Off);

impl Browser {
    pub fn arg(self) -> Option<&'static str> {
        Some(match self {
            Browser::Off => return None,
            Browser::Firefox => "firefox",
            Browser::Chrome => "chrome",
            Browser::Chromium => "chromium",
            Browser::Brave => "brave",
            Browser::Edge => "edge",
            Browser::Vivaldi => "vivaldi",
            Browser::Opera => "opera",
        })
    }
}

choice_enum!(SearchSource {
    YouTube => "YouTube",
    YouTubeMusic => "YouTube Music",
    SoundCloud => "SoundCloud",
} default YouTube);

/// Value lists for numeric options (cycled in the form).
pub const RETRIES: &[u32] = &[3, 5, 10, 25, 0]; // 0 = infinite
pub const FRAGMENTS: &[u32] = &[1, 2, 4, 8, 16];
pub const MAX_CONCURRENT: &[u32] = &[1, 2, 3, 4, 6, 8];
pub const SEARCH_COUNT: &[u32] = &[10, 20, 30, 50];

pub fn cycle_num(list: &[u32], cur: u32, dir: i32) -> u32 {
    let i = list.iter().position(|x| *x == cur).unwrap_or(0) as i32;
    list[(i + dir).rem_euclid(list.len() as i32) as usize]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    // Output
    pub output_dir: String,
    pub filename_style: FilenameStyle,
    pub custom_template: String,
    pub number_playlist: bool,
    pub playlist_folder: bool,
    pub restrict_filenames: bool,
    pub overwrite: bool,
    // Format
    pub mode: Mode,
    pub audio_format: AudioFormat,
    pub audio_quality: AudioQuality,
    pub video_container: VideoContainer,
    pub max_height: MaxHeight,
    pub video_codec: VideoCodec,
    // Metadata & extras
    pub embed_metadata: bool,
    pub embed_thumbnail: bool,
    pub square_cover: bool,
    pub embed_chapters: bool,
    pub subtitles: Subtitles,
    pub sub_langs: String,
    pub auto_subs: bool,
    pub split_chapters: bool,
    pub sponsorblock: SponsorBlock,
    pub sponsor_categories: String,
    // Playlist
    pub playlist: PlaylistMode,
    pub playlist_items: String,
    pub use_archive: bool,
    pub archive_file: String,
    // Network
    pub cookies_browser: Browser,
    pub rate_limit: String,
    pub retries: u32,
    pub fragments: u32,
    pub proxy: String,
    // App
    pub max_concurrent: u32,
    pub search_source: SearchSource,
    pub search_count: u32,
    pub notify: bool,
    pub ytdlp_path: String,
    pub extra_args: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            output_dir: "~/Music".into(),
            filename_style: FilenameStyle::Title,
            custom_template: "%(uploader)s/%(title)s.%(ext)s".into(),
            number_playlist: false,
            playlist_folder: true,
            restrict_filenames: false,
            overwrite: false,
            mode: Mode::Audio,
            audio_format: AudioFormat::Mp3,
            audio_quality: AudioQuality::Best,
            video_container: VideoContainer::Mp4,
            max_height: MaxHeight::P1080,
            video_codec: VideoCodec::H264,
            embed_metadata: true,
            embed_thumbnail: true,
            square_cover: true,
            embed_chapters: true,
            subtitles: Subtitles::Off,
            sub_langs: "en.*".into(),
            auto_subs: false,
            split_chapters: false,
            sponsorblock: SponsorBlock::Off,
            sponsor_categories: "sponsor,selfpromo,interaction,music_offtopic".into(),
            playlist: PlaylistMode::Whole,
            playlist_items: String::new(),
            use_archive: false,
            archive_file: "~/.local/share/mus-cli/archive.txt".into(),
            cookies_browser: Browser::Off,
            rate_limit: String::new(),
            retries: 10,
            fragments: 4,
            proxy: String::new(),
            max_concurrent: 3,
            search_source: SearchSource::YouTube,
            search_count: 20,
            notify: true,
            ytdlp_path: "yt-dlp".into(),
            extra_args: String::new(),
        }
    }
}

pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("mus-cli").join("config.toml"))
}

/// Expand a leading `~` to the home directory.
pub fn expand(path: &str) -> PathBuf {
    let path = path.trim();
    if path == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
    }
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    PathBuf::from(path)
}

impl Config {
    /// Load the config; on a parse error, fall back to defaults and return a warning.
    pub fn load() -> (Config, Option<String>) {
        let Some(path) = config_path() else {
            return (Config::default(), None);
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str(&text) {
                Ok(cfg) => (cfg, None),
                Err(e) => {
                    let backup = path.with_extension("toml.bak");
                    let _ = std::fs::copy(&path, &backup);
                    let first = e.to_string().lines().next().unwrap_or("").to_string();
                    (
                        Config::default(),
                        Some(format!("Config invalid ({first}); using defaults, backup saved")),
                    )
                }
            },
            Err(_) => (Config::default(), None),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = config_path().ok_or("no config directory")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| e.to_string())
    }
}

/// Built-in presets: a name, a description, and the changes they apply.
pub struct Preset {
    pub name: &'static str,
    pub desc: &'static str,
    pub apply: fn(&mut Config),
}

pub const PRESETS: &[Preset] = &[
    Preset {
        name: "Music · MP3 best",
        desc: "MP3 V0 with tags and square cover art",
        apply: |c| {
            c.mode = Mode::Audio;
            c.audio_format = AudioFormat::Mp3;
            c.audio_quality = AudioQuality::Best;
            c.embed_metadata = true;
            c.embed_thumbnail = true;
            c.square_cover = true;
        },
    },
    Preset {
        name: "Music · MP3 320k",
        desc: "Constant 320 kbps MP3 for old players",
        apply: |c| {
            c.mode = Mode::Audio;
            c.audio_format = AudioFormat::Mp3;
            c.audio_quality = AudioQuality::K320;
            c.embed_metadata = true;
            c.embed_thumbnail = true;
            c.square_cover = true;
        },
    },
    Preset {
        name: "Music · Original",
        desc: "Best audio stream as-is (usually Opus/M4A), no quality loss",
        apply: |c| {
            c.mode = Mode::Audio;
            c.audio_format = AudioFormat::Best;
            c.embed_metadata = true;
            c.embed_thumbnail = true;
            c.square_cover = true;
        },
    },
    Preset {
        name: "Music · FLAC",
        desc: "Lossless container (source is still lossy on most sites)",
        apply: |c| {
            c.mode = Mode::Audio;
            c.audio_format = AudioFormat::Flac;
            c.embed_metadata = true;
            c.embed_thumbnail = true;
            c.square_cover = true;
        },
    },
    Preset {
        name: "Video · 1080p MP4",
        desc: "Plays everywhere: H.264 preferred, up to 1080p",
        apply: |c| {
            c.mode = Mode::Video;
            c.video_container = VideoContainer::Mp4;
            c.max_height = MaxHeight::P1080;
            c.video_codec = VideoCodec::H264;
            c.embed_metadata = true;
            c.embed_thumbnail = true;
            c.embed_chapters = true;
        },
    },
    Preset {
        name: "Video · Best MKV",
        desc: "Highest resolution and codec available, with subtitles",
        apply: |c| {
            c.mode = Mode::Video;
            c.video_container = VideoContainer::Mkv;
            c.max_height = MaxHeight::Best;
            c.video_codec = VideoCodec::Any;
            c.subtitles = Subtitles::Embed;
            c.embed_metadata = true;
            c.embed_thumbnail = true;
            c.embed_chapters = true;
        },
    },
    Preset {
        name: "Video · 720p small",
        desc: "Smaller files for phones and slow connections",
        apply: |c| {
            c.mode = Mode::Video;
            c.video_container = VideoContainer::Mp4;
            c.max_height = MaxHeight::P720;
            c.video_codec = VideoCodec::H264;
        },
    },
    Preset {
        name: "Podcast / talk",
        desc: "Opus 96k, SponsorBlock segments cut out",
        apply: |c| {
            c.mode = Mode::Audio;
            c.audio_format = AudioFormat::Opus;
            c.audio_quality = AudioQuality::K96;
            c.sponsorblock = SponsorBlock::Remove;
            c.embed_chapters = true;
        },
    },
];
