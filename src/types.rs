use std::path::PathBuf;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppTab { #[default] Dashboard, Audio, Video, Advanced }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProxyProtocol { 
    #[default] 
    Socks5, 
    Socks4, 
    Http, 
    Https 
}
impl ProxyProtocol {
    pub const ALL: [ProxyProtocol; 4] = [ProxyProtocol::Socks5, ProxyProtocol::Socks4, ProxyProtocol::Http, ProxyProtocol::Https];
    pub fn as_str(&self) -> &'static str {
        match self {
            ProxyProtocol::Socks5 => "socks5",
            ProxyProtocol::Socks4 => "socks4",
            ProxyProtocol::Http => "http",
            ProxyProtocol::Https => "https",
        }
    }
}
impl std::fmt::Display for ProxyProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.as_str()) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioFormat { #[default] None, Mp3, Aac, M4a, Wav, Flac, Opus }
impl AudioFormat {
    pub const ALL: [AudioFormat; 7] = [AudioFormat::None, AudioFormat::Mp3, AudioFormat::Aac, AudioFormat::M4a, AudioFormat::Wav, AudioFormat::Flac, AudioFormat::Opus];
    pub fn as_str(&self) -> &'static str { match self { AudioFormat::None => "Video (Default)", AudioFormat::Mp3 => "mp3", AudioFormat::Aac => "aac", AudioFormat::M4a => "m4a", AudioFormat::Wav => "wav", AudioFormat::Flac => "flac", AudioFormat::Opus => "opus" } }
}
impl std::fmt::Display for AudioFormat { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.as_str()) } }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Container { #[default] Mp4, Mkv, Webm }
impl Container {
    pub const ALL: [Container; 3] = [Container::Mp4, Container::Mkv, Container::Webm];
    pub fn as_str(&self) -> &'static str { match self { Container::Mp4 => "mp4", Container::Mkv => "mkv", Container::Webm => "webm" } }
}
impl std::fmt::Display for Container { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.as_str()) } }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoType {
    #[default]
    Normal,
    VR360, 
    ThreeD 
}
impl VideoType {
    pub const ALL: [VideoType; 3] = [VideoType::Normal, VideoType::VR360, VideoType::ThreeD];
    pub fn as_str(&self) -> &'static str {
        match self {
            VideoType::Normal => "Normal / Best",
            VideoType::VR360 => "Prefer VR / 360°",
            VideoType::ThreeD => "Prefer 3D",
        }
    }
}
impl std::fmt::Display for VideoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.as_str()) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilenameTemplate {
    #[default]
    Default, 
    Clean,   
    Channel, 
    Numbered 
}
impl FilenameTemplate {
    pub const ALL: [FilenameTemplate; 4] = [FilenameTemplate::Default, FilenameTemplate::Clean, FilenameTemplate::Channel, FilenameTemplate::Numbered];
    pub fn as_str(&self) -> &'static str {
        match self {
            FilenameTemplate::Default => "Default (Title [ID])",
            FilenameTemplate::Clean => "Clean (Title only)",
            FilenameTemplate::Channel => "Channel - Title",
            FilenameTemplate::Numbered => "Playlist Index - Title",
        }
    }
    pub fn to_cmd_arg(&self) -> Option<String> {
        match self {
            FilenameTemplate::Default => None,
            FilenameTemplate::Clean => Some("%(title)s.%(ext)s".to_string()),
            FilenameTemplate::Channel => Some("%(uploader)s - %(title)s.%(ext)s".to_string()),
            FilenameTemplate::Numbered => Some("%(playlist_index)s - %(title)s.%(ext)s".to_string()),
        }
    }
}
impl std::fmt::Display for FilenameTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.as_str()) }
}

#[derive(Debug, Clone)]
pub struct AdvOptions {
    pub audio_fmt: AudioFormat,
    pub container: Container,
    pub video_type: VideoType,
    pub embed_subs: bool,
    pub sub_langs: String, 
    pub embed_meta: bool,
    pub embed_thumb: bool,
    pub filename_style: FilenameTemplate, 
    pub sponsorblock: bool,
    pub playlist_items: String,
    pub rate_limit: String,
    pub custom_args: String,
}

#[derive(Debug, Clone)]
pub struct DownloadItem {
    pub id: usize,
    pub url: String,
    pub title: String,
    pub status: DownloadStatus,
    pub progress: f32,
    pub speed: String,
    pub total_size: String,
    pub assigned_proxy: Option<String>,
    pub options: AdvOptions,
}

#[derive(Debug, Clone)]
pub enum DownloadStatus { Queued, Downloading, Finished, Failed(String), Cancelled }

#[derive(Deserialize, Debug)]
pub struct GitHubRelease { pub tag_name: String }

#[derive(Debug, Clone)]
pub enum Message {
    Ignore,
    TabChanged(AppTab),
    UrlChanged(String),
    ManualProxyChanged(String),
    RequestAddUrl,
    AnalysisFinished(Result<serde_json::Value, String>),
    LiveDecision(bool),
    CloseModal,
    
    // Settings
    AudioFmtChanged(AudioFormat),
    ContainerChanged(Container),
    VideoTypeChanged(VideoType),
    ToggleEmbedSubs(bool),
    SubLangsChanged(String), 
    ToggleEmbedMeta(bool),
    ToggleEmbedThumb(bool),
    FilenameStyleChanged(FilenameTemplate), 
    ToggleSponsorBlock(bool),
    PlaylistItemsChanged(String),
    RateLimitChanged(String),
    CustomArgsChanged(String),
    ProxyProtocolChanged(ProxyProtocol),
    MaxConcurrentChanged(String),

    // File/IO
    PickCookieFile, CookieFilePicked(Option<PathBuf>),
    PickOutputDir, OutputDirPicked(Option<PathBuf>),
    PickProxyList, ProxyListLoaded(Option<String>),
    
    // Download Control
    RetryDownload(usize), CancelDownload(usize),
    Tick, CheckForUpdates,
    
    // Feedback
    DownloadProgress(usize, f32, String, String),
    DownloadFinished(usize),
    DownloadFailed(usize, String),
    
    // Removed unused install messages, kept ToolInstalled
    ToolInstalled(Result<String, String>),
}