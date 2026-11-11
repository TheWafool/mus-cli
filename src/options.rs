//! Data-driven option fields: metadata for the form plus get/set on `Config`.

use crate::config::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum F {
    OutputDir,
    FilenameStyle,
    CustomTemplate,
    NumberPlaylist,
    PlaylistFolder,
    RestrictFilenames,
    Overwrite,
    Mode,
    AudioFormat,
    AudioQuality,
    VideoContainer,
    MaxHeight,
    VideoCodec,
    EmbedMetadata,
    EmbedThumbnail,
    SquareCover,
    EmbedChapters,
    Subtitles,
    SubLangs,
    AutoSubs,
    SplitChapters,
    SponsorBlock,
    SponsorCats,
    Playlist,
    PlaylistItems,
    UseArchive,
    ArchiveFile,
    Cookies,
    RateLimit,
    Retries,
    Fragments,
    Proxy,
    MaxConcurrent,
    SearchSource,
    SearchCount,
    Notify,
    YtdlpPath,
    ExtraArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Toggle,
    Choice,
    Text,
    Path,
}

pub struct Field {
    pub id: F,
    pub section: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    pub help: &'static str,
    pub placeholder: &'static str,
}

const fn f(
    id: F,
    section: &'static str,
    label: &'static str,
    kind: Kind,
    help: &'static str,
    placeholder: &'static str,
) -> Field {
    Field { id, section, label, kind, help, placeholder }
}

use Kind::*;

pub const FIELDS: &[Field] = &[
    // Output
    f(F::OutputDir, "Output", "Output folder", Path,
      "Where files are saved. `~` is expanded. The folder is created if needed.", "~/Music"),
    f(F::FilenameStyle, "Output", "File name", Choice,
      "How files are named. Pick \"Custom template\" to write your own yt-dlp output template.", ""),
    f(F::CustomTemplate, "Output", "Custom template", Text,
      "yt-dlp output template, e.g. %(uploader)s/%(title)s.%(ext)s. Slashes create subfolders. \
       .%(ext)s is appended if missing.", "%(title)s.%(ext)s"),
    f(F::NumberPlaylist, "Output", "Number playlist items", Toggle,
      "Prefix files from a playlist with their position, e.g. \"03 - Song.mp3\".", ""),
    f(F::PlaylistFolder, "Output", "Playlist subfolder", Toggle,
      "Put playlists and albums into a folder named after the playlist. Detected from the URL \
       (list=, /playlist, /album/, /sets/, channels).", ""),
    f(F::RestrictFilenames, "Output", "ASCII-only names", Toggle,
      "Restrict file names to ASCII and replace spaces/& with underscores.", ""),
    f(F::Overwrite, "Output", "Overwrite existing", Toggle,
      "Re-download and overwrite files that already exist. Off skips them.", ""),
    // Format
    f(F::Mode, "Format", "Mode", Choice,
      "Audio only extracts the sound track. Video downloads video + audio merged into one file.", ""),
    f(F::AudioFormat, "Format", "Audio format", Choice,
      "Format to convert audio to (needs ffmpeg). \"Original\" keeps the source stream untouched, \
       which is the highest quality.", ""),
    f(F::AudioQuality, "Format", "Audio quality", Choice,
      "Bitrate for lossy formats. \"Best\" is variable bitrate at top quality (V0 for MP3).", ""),
    f(F::VideoContainer, "Format", "Container", Choice,
      "MP4 plays everywhere. MKV accepts every codec and subtitle type. WebM is open-format.", ""),
    f(F::MaxHeight, "Format", "Max resolution", Choice,
      "Best video at or below this height. Falls back to the closest available.", ""),
    f(F::VideoCodec, "Format", "Preferred codec", Choice,
      "Codec preference. H.264 is the safest for old devices and editors; AV1 is smallest.", ""),
    // Metadata & extras
    f(F::EmbedMetadata, "Metadata & extras", "Embed metadata", Toggle,
      "Write title, artist, album, date and description tags into the file.", ""),
    f(F::EmbedThumbnail, "Metadata & extras", "Embed thumbnail", Toggle,
      "Embed the thumbnail as cover art. Skipped automatically for WAV and WebM.", ""),
    f(F::SquareCover, "Metadata & extras", "Square cover art", Toggle,
      "Crop the thumbnail to a centered square so it looks right in music players (audio only).", ""),
    f(F::EmbedChapters, "Metadata & extras", "Embed chapters", Toggle,
      "Add chapter markers to the file.", ""),
    f(F::Subtitles, "Metadata & extras", "Subtitles", Choice,
      "Download subtitles (video mode). Embedding works best with MKV/MP4.", ""),
    f(F::SubLangs, "Metadata & extras", "Subtitle languages", Text,
      "Comma-separated language codes, regex allowed. e.g. en.*,de or all", "en.*"),
    f(F::AutoSubs, "Metadata & extras", "Auto-generated subs", Toggle,
      "Also use auto-generated (machine) subtitles when no real ones exist.", ""),
    f(F::SplitChapters, "Metadata & extras", "Split by chapters", Toggle,
      "Also save each chapter as its own file, in a folder named after the video. Great for albums \
       uploaded as one long video.", ""),
    f(F::SponsorBlock, "Metadata & extras", "SponsorBlock", Choice,
      "Use the SponsorBlock database to mark or remove sponsor segments, intros, and off-topic parts.", ""),
    f(F::SponsorCats, "Metadata & extras", "SponsorBlock categories", Text,
      "Comma-separated: sponsor, intro, outro, selfpromo, preview, filler, interaction, \
       music_offtopic, poi_highlight, chapter, or all.", "sponsor,selfpromo"),
    // Playlist
    f(F::Playlist, "Playlist", "Playlist links", Choice,
      "For a link like watch?v=…&list=…, download the whole playlist or only that video.", ""),
    f(F::PlaylistItems, "Playlist", "Items", Text,
      "Which items to get, e.g. 1-10,15 or ::-1 for reversed or -5: for the last five. Empty = all.", "all"),
    f(F::UseArchive, "Playlist", "Download archive", Toggle,
      "Remember downloaded videos in an archive file and skip them next time. Ideal for syncing \
       playlists.", ""),
    f(F::ArchiveFile, "Playlist", "Archive file", Path,
      "Path of the archive file.", "~/.local/share/mus-cli/archive.txt"),
    // Network
    f(F::Cookies, "Network", "Cookies from browser", Choice,
      "Use your browser's logged-in cookies, for age-restricted, members-only or private videos.", ""),
    f(F::RateLimit, "Network", "Rate limit", Text,
      "Max download speed per download, e.g. 500K or 4M. Empty = unlimited.", "unlimited"),
    f(F::Retries, "Network", "Retries", Choice,
      "How many times to retry a failed download.", ""),
    f(F::Fragments, "Network", "Parallel fragments", Choice,
      "Fragments downloaded at once for DASH/HLS streams. Higher can be much faster.", ""),
    f(F::Proxy, "Network", "Proxy", Text,
      "e.g. socks5://127.0.0.1:1080 or http://host:port. Empty = none.", "none"),
    // App
    f(F::MaxConcurrent, "App", "Parallel downloads", Choice,
      "How many queue items download at the same time.", ""),
    f(F::SearchSource, "App", "Search source", Choice,
      "Where the Search tab looks.", ""),
    f(F::SearchCount, "App", "Search results", Choice,
      "How many results to fetch per search.", ""),
    f(F::Notify, "App", "Desktop notifications", Toggle,
      "Send a desktop notification when a download finishes or fails (notify-send on Linux).", ""),
    f(F::YtdlpPath, "App", "yt-dlp binary", Path,
      "Command or path used to run yt-dlp.", "yt-dlp"),
    f(F::ExtraArgs, "App", "Extra arguments", Text,
      "Additional yt-dlp arguments, added last. Shell-style quoting works.", "e.g. --sleep-interval 2"),
];

/// Fields shown on the Download tab.
pub const QUICK: &[F] = &[
    F::Mode,
    F::AudioFormat,
    F::AudioQuality,
    F::VideoContainer,
    F::MaxHeight,
    F::OutputDir,
    F::FilenameStyle,
    F::Playlist,
    F::EmbedThumbnail,
    F::SponsorBlock,
];

pub fn field(id: F) -> &'static Field {
    FIELDS.iter().find(|x| x.id == id).expect("field defined")
}

fn on_off(b: bool) -> String {
    if b { "On".into() } else { "Off".into() }
}

impl Config {
    /// Whether a field has any effect with the current settings.
    pub fn relevant(&self, id: F) -> bool {
        let audio = self.mode == Mode::Audio;
        match id {
            F::CustomTemplate => self.filename_style == FilenameStyle::Custom,
            F::AudioFormat => audio,
            F::AudioQuality => audio && self.audio_format.takes_quality(),
            F::VideoContainer | F::MaxHeight | F::VideoCodec | F::Subtitles => !audio,
            F::SquareCover => audio && self.embed_thumbnail,
            F::SubLangs | F::AutoSubs => !audio && self.subtitles != Subtitles::Off,
            F::SponsorCats => self.sponsorblock != SponsorBlock::Off,
            F::ArchiveFile => self.use_archive,
            F::NumberPlaylist | F::PlaylistFolder | F::PlaylistItems => {
                self.playlist == PlaylistMode::Whole
            }
            _ => true,
        }
    }

    pub fn display(&self, id: F) -> String {
        match id {
            F::OutputDir => self.output_dir.clone(),
            F::FilenameStyle => self.filename_style.label().into(),
            F::CustomTemplate => self.custom_template.clone(),
            F::NumberPlaylist => on_off(self.number_playlist),
            F::PlaylistFolder => on_off(self.playlist_folder),
            F::RestrictFilenames => on_off(self.restrict_filenames),
            F::Overwrite => on_off(self.overwrite),
            F::Mode => self.mode.label().into(),
            F::AudioFormat => self.audio_format.label().into(),
            F::AudioQuality => self.audio_quality.label().into(),
            F::VideoContainer => self.video_container.label().into(),
            F::MaxHeight => self.max_height.label().into(),
            F::VideoCodec => self.video_codec.label().into(),
            F::EmbedMetadata => on_off(self.embed_metadata),
            F::EmbedThumbnail => on_off(self.embed_thumbnail),
            F::SquareCover => on_off(self.square_cover),
            F::EmbedChapters => on_off(self.embed_chapters),
            F::Subtitles => self.subtitles.label().into(),
            F::SubLangs => self.sub_langs.clone(),
            F::AutoSubs => on_off(self.auto_subs),
            F::SplitChapters => on_off(self.split_chapters),
            F::SponsorBlock => self.sponsorblock.label().into(),
            F::SponsorCats => self.sponsor_categories.clone(),
            F::Playlist => self.playlist.label().into(),
            F::PlaylistItems => self.playlist_items.clone(),
            F::UseArchive => on_off(self.use_archive),
            F::ArchiveFile => self.archive_file.clone(),
            F::Cookies => self.cookies_browser.label().into(),
            F::RateLimit => self.rate_limit.clone(),
            F::Retries => match self.retries {
                0 => "Infinite".into(),
                n => n.to_string(),
            },
            F::Fragments => self.fragments.to_string(),
            F::Proxy => self.proxy.clone(),
            F::MaxConcurrent => self.max_concurrent.to_string(),
            F::SearchSource => self.search_source.label().into(),
            F::SearchCount => self.search_count.to_string(),
            F::Notify => on_off(self.notify),
            F::YtdlpPath => self.ytdlp_path.clone(),
            F::ExtraArgs => self.extra_args.clone(),
        }
    }

    pub fn toggle_value(&self, id: F) -> Option<bool> {
        Some(match id {
            F::NumberPlaylist => self.number_playlist,
            F::PlaylistFolder => self.playlist_folder,
            F::RestrictFilenames => self.restrict_filenames,
            F::Overwrite => self.overwrite,
            F::EmbedMetadata => self.embed_metadata,
            F::EmbedThumbnail => self.embed_thumbnail,
            F::SquareCover => self.square_cover,
            F::EmbedChapters => self.embed_chapters,
            F::AutoSubs => self.auto_subs,
            F::SplitChapters => self.split_chapters,
            F::UseArchive => self.use_archive,
            F::Notify => self.notify,
            _ => return None,
        })
    }

    /// Flip a toggle or step a choice. Returns false for text fields.
    pub fn cycle(&mut self, id: F, dir: i32) -> bool {
        match id {
            F::NumberPlaylist => self.number_playlist ^= true,
            F::PlaylistFolder => self.playlist_folder ^= true,
            F::RestrictFilenames => self.restrict_filenames ^= true,
            F::Overwrite => self.overwrite ^= true,
            F::EmbedMetadata => self.embed_metadata ^= true,
            F::EmbedThumbnail => self.embed_thumbnail ^= true,
            F::SquareCover => self.square_cover ^= true,
            F::EmbedChapters => self.embed_chapters ^= true,
            F::AutoSubs => self.auto_subs ^= true,
            F::SplitChapters => self.split_chapters ^= true,
            F::UseArchive => self.use_archive ^= true,
            F::Notify => self.notify ^= true,
            F::FilenameStyle => self.filename_style = self.filename_style.cycle(dir),
            F::Mode => self.mode = self.mode.cycle(dir),
            F::AudioFormat => self.audio_format = self.audio_format.cycle(dir),
            F::AudioQuality => self.audio_quality = self.audio_quality.cycle(dir),
            F::VideoContainer => self.video_container = self.video_container.cycle(dir),
            F::MaxHeight => self.max_height = self.max_height.cycle(dir),
            F::VideoCodec => self.video_codec = self.video_codec.cycle(dir),
            F::Subtitles => self.subtitles = self.subtitles.cycle(dir),
            F::SponsorBlock => self.sponsorblock = self.sponsorblock.cycle(dir),
            F::Playlist => self.playlist = self.playlist.cycle(dir),
            F::Cookies => self.cookies_browser = self.cookies_browser.cycle(dir),
            F::Retries => self.retries = cycle_num(RETRIES, self.retries, dir),
            F::Fragments => self.fragments = cycle_num(FRAGMENTS, self.fragments, dir),
            F::MaxConcurrent => {
                self.max_concurrent = cycle_num(MAX_CONCURRENT, self.max_concurrent, dir)
            }
            F::SearchSource => self.search_source = self.search_source.cycle(dir),
            F::SearchCount => self.search_count = cycle_num(SEARCH_COUNT, self.search_count, dir),
            _ => return false,
        }
        true
    }

    fn text_mut(&mut self, id: F) -> Option<&mut String> {
        Some(match id {
            F::OutputDir => &mut self.output_dir,
            F::CustomTemplate => &mut self.custom_template,
            F::SubLangs => &mut self.sub_langs,
            F::SponsorCats => &mut self.sponsor_categories,
            F::PlaylistItems => &mut self.playlist_items,
            F::ArchiveFile => &mut self.archive_file,
            F::RateLimit => &mut self.rate_limit,
            F::Proxy => &mut self.proxy,
            F::YtdlpPath => &mut self.ytdlp_path,
            F::ExtraArgs => &mut self.extra_args,
            _ => return None,
        })
    }

    pub fn set_text(&mut self, id: F, value: String) {
        if let Some(s) = self.text_mut(id) {
            *s = value;
        }
    }

    /// Restore one field to its default value.
    pub fn reset_field(&mut self, id: F) {
        let d = Config::default();
        match field(id).kind {
            Kind::Text | Kind::Path => self.set_text(id, d.display(id)),
            _ => {
                let target = d.display(id);
                for _ in 0..32 {
                    if self.display(id) == target {
                        break;
                    }
                    self.cycle(id, 1);
                }
            }
        }
    }
}
