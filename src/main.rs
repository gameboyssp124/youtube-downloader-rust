#![windows_subsystem = "windows"] // Hides the console window

mod types;
mod style;
mod tools;
mod logic;

use iced::widget::{button, column, container, pick_list, progress_bar, row, scrollable, text, text_input, toggler, Space};
use iced::{executor, time, alignment, Application, Command, Element, Length, Settings, Subscription, Theme};
use iced::theme; 

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use types::*;
use style::*;
use tools::*;
use logic::*;

pub fn main() -> iced::Result {
    YtDownloader::run(Settings::default())
}

struct YtDownloader {
    current_tab: AppTab,
    input_url: String,
    manual_proxy: String,
    cookie_path: Option<PathBuf>,
    output_dir: PathBuf,
    queue: Vec<DownloadItem>,
    next_id: usize,
    max_concurrent: usize,
    max_concurrent_input: String,
    active_downloads: usize,
    proxy_list: Vec<String>,
    proxy_counter: Arc<AtomicUsize>,
    selected_proxy_proto: ProxyProtocol,
    settings: AdvOptions,
    tool_status: String,
    is_analyzing: bool,
    modal_live_url: Option<String>,
}

impl Application for YtDownloader {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            YtDownloader {
                current_tab: AppTab::Dashboard,
                input_url: String::new(),
                manual_proxy: String::new(),
                cookie_path: None,
                output_dir: std::env::current_dir().unwrap_or_default(),
                queue: Vec::new(),
                next_id: 0,
                max_concurrent: 3,
                max_concurrent_input: "3".to_string(),
                active_downloads: 0,
                proxy_list: Vec::new(),
                proxy_counter: Arc::new(AtomicUsize::new(0)),
                selected_proxy_proto: ProxyProtocol::Socks5,
                settings: AdvOptions {
                    audio_fmt: AudioFormat::None,
                    container: crate::types::Container::Mp4, 
                    video_type: VideoType::Normal,
                    embed_subs: false,
                    sub_langs: "all".to_string(),
                    embed_meta: true,
                    embed_thumb: true,
                    filename_style: FilenameTemplate::Default,
                    sponsorblock: false,
                    playlist_items: String::new(),
                    rate_limit: String::new(),
                    custom_args: String::new(),
                },
                tool_status: "Initializing...".to_string(),
                is_analyzing: false,
                modal_live_url: None,
            },
            Command::perform(async {}, |_| Message::CheckForUpdates),
        )
    }

    fn title(&self) -> String { String::from("Yt-Dlp GUI | Ultimate") }
    fn theme(&self) -> Theme { Theme::Dark }

    fn subscription(&self) -> Subscription<Message> {
        let tick = time::every(std::time::Duration::from_millis(500)).map(|_| Message::Tick);
        let download_subs = self.queue.iter()
            .filter(|item| matches!(item.status, DownloadStatus::Downloading))
            .map(|item| {
                download_stream(
                    item.id, item.url.clone(), self.output_dir.clone(),
                    item.assigned_proxy.clone(), self.cookie_path.clone(), item.options.clone()
                )
            });
        Subscription::batch(std::iter::once(tick).chain(download_subs))
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Ignore => Command::none(),
            Message::TabChanged(tab) => { self.current_tab = tab; Command::none() }
            Message::UrlChanged(s) => { self.input_url = s; Command::none() }
            Message::ManualProxyChanged(s) => { self.manual_proxy = s; Command::none() }
            Message::ProxyProtocolChanged(p) => { self.selected_proxy_proto = p; Command::none() }
            
            Message::MaxConcurrentChanged(val) => {
                if val.chars().all(|c| c.is_numeric()) {
                    self.max_concurrent_input = val.clone();
                    if let Ok(num) = val.parse::<usize>() { self.max_concurrent = num.clamp(1, 50); }
                } else if val.is_empty() { self.max_concurrent_input = String::new(); }
                Command::none()
            }

            Message::RequestAddUrl => {
                if !self.input_url.trim().is_empty() && !self.is_analyzing {
                    self.is_analyzing = true;
                    self.tool_status = "Analyzing Link...".to_string();
                    let url = self.input_url.trim().to_string();
                    let proxy = if !self.manual_proxy.is_empty() { Some(format_proxy(&self.manual_proxy, self.selected_proxy_proto)) } else { None };
                    let cookie = self.cookie_path.clone();
                    Command::perform(analyze_url_task(url, proxy, cookie), Message::AnalysisFinished)
                } else { Command::none() }
            }

            Message::AnalysisFinished(result) => {
                self.is_analyzing = false;
                self.tool_status = "Analysis Complete".to_string();
                match result {
                    Ok(json) => {
                        if let Some(is_live) = json.get("is_live").and_then(|v| v.as_bool()) {
                            if is_live {
                                if let Some(url) = json.get("webpage_url").and_then(|s| s.as_str()).or(json.get("url").and_then(|s|s.as_str())) {
                                    self.modal_live_url = Some(url.to_string());
                                    self.input_url.clear();
                                    return Command::none();
                                }
                            }
                        }
                        if let Some(entries) = json.get("entries").and_then(|v| v.as_array()) {
                            let mut added_count = 0;
                            for entry in entries {
                                if let Some(id) = entry.get("id").and_then(|s| s.as_str()) {
                                    let title = entry.get("title").and_then(|s| s.as_str()).unwrap_or("Unknown Title");
                                    let url = format!("https://www.youtube.com/watch?v={}", id);
                                    self.queue.push(DownloadItem {
                                        id: self.next_id, url, title: title.to_string(), status: DownloadStatus::Queued, progress: 0.0, speed: "-".into(), total_size: "-".into(), assigned_proxy: None, options: self.settings.clone(),
                                    });
                                    self.next_id += 1; added_count += 1;
                                }
                            }
                            self.tool_status = format!("Added {} videos", added_count);
                        } else {
                            let title = json.get("title").and_then(|s| s.as_str()).unwrap_or("Video");
                            let url = json.get("webpage_url").and_then(|s| s.as_str()).or(json.get("url").and_then(|s|s.as_str())).unwrap_or(&self.input_url);
                            self.queue.push(DownloadItem {
                                id: self.next_id, url: url.to_string(), title: title.to_string(), status: DownloadStatus::Queued, progress: 0.0, speed: "-".into(), total_size: "-".into(), assigned_proxy: None, options: self.settings.clone(),
                            });
                            self.next_id += 1;
                        }
                        self.input_url.clear();
                        self.current_tab = AppTab::Dashboard;
                    },
                    Err(e) => { self.tool_status = format!("Analysis Failed: {}", e); }
                }
                Command::none()
            }

            Message::LiveDecision(from_start) => {
                if let Some(url) = self.modal_live_url.take() {
                    let mut opts = self.settings.clone();
                    if from_start { opts.custom_args.push_str(" --live-from-start"); }
                    self.queue.push(DownloadItem {
                        id: self.next_id, url, title: "Live Stream".into(), status: DownloadStatus::Queued, progress: 0.0, speed: "-".into(), total_size: "-".into(), assigned_proxy: None, options: opts,
                    });
                    self.next_id += 1;
                    self.current_tab = AppTab::Dashboard;
                }
                Command::none()
            }
            Message::CloseModal => { self.modal_live_url = None; Command::none() }

            Message::AudioFmtChanged(v) => { self.settings.audio_fmt = v; Command::none() }
            Message::ContainerChanged(v) => { self.settings.container = v; Command::none() }
            Message::VideoTypeChanged(v) => { self.settings.video_type = v; Command::none() }
            Message::FilenameStyleChanged(v) => { self.settings.filename_style = v; Command::none() }
            Message::ToggleEmbedSubs(v) => { self.settings.embed_subs = v; Command::none() }
            Message::SubLangsChanged(v) => { self.settings.sub_langs = v; Command::none() }
            Message::ToggleEmbedMeta(v) => { self.settings.embed_meta = v; Command::none() }
            Message::ToggleEmbedThumb(v) => { self.settings.embed_thumb = v; Command::none() }
            Message::ToggleSponsorBlock(v) => { self.settings.sponsorblock = v; Command::none() }
            Message::PlaylistItemsChanged(v) => { self.settings.playlist_items = v; Command::none() }
            Message::RateLimitChanged(v) => { self.settings.rate_limit = v; Command::none() }
            Message::CustomArgsChanged(v) => { self.settings.custom_args = v; Command::none() }
            
            Message::RetryDownload(id) => {
                if let Some(item) = self.queue.iter_mut().find(|x| x.id == id) { item.status = DownloadStatus::Queued; item.progress = 0.0; }
                Command::none()
            }
            Message::CancelDownload(id) => {
                if let Some(item) = self.queue.iter_mut().find(|x| x.id == id) {
                    if matches!(item.status, DownloadStatus::Downloading) { self.active_downloads = self.active_downloads.saturating_sub(1); }
                    item.status = DownloadStatus::Cancelled;
                }
                Command::none()
            }
            Message::Tick => {
                if self.active_downloads < self.max_concurrent {
                    if let Some(item) = self.queue.iter_mut().find(|x| matches!(x.status, DownloadStatus::Queued)) {
                        item.status = DownloadStatus::Downloading;
                        self.active_downloads += 1;
                        if !self.manual_proxy.is_empty() { item.assigned_proxy = Some(format_proxy(&self.manual_proxy, self.selected_proxy_proto)); } 
                        else if !self.proxy_list.is_empty() {
                            let count = self.proxy_counter.fetch_add(1, Ordering::SeqCst);
                            let p = self.proxy_list[count % self.proxy_list.len()].clone();
                            item.assigned_proxy = Some(format_proxy(&p, self.selected_proxy_proto));
                        }
                    }
                }
                Command::none()
            }
            Message::DownloadProgress(id, prog, spd, sz) => {
                if let Some(item) = self.queue.iter_mut().find(|x| x.id == id) { item.progress = prog; if !spd.is_empty() { item.speed = spd; } if !sz.is_empty() { item.total_size = sz; } }
                Command::none()
            }
            Message::DownloadFinished(id) => {
                if let Some(item) = self.queue.iter_mut().find(|x| x.id == id) { item.status = DownloadStatus::Finished; item.progress = 100.0; item.speed = String::from("Done"); self.active_downloads = self.active_downloads.saturating_sub(1); }
                Command::none()
            }
            Message::DownloadFailed(id, err) => {
                if let Some(item) = self.queue.iter_mut().find(|x| x.id == id) { item.status = DownloadStatus::Failed(err); item.speed = String::from("Failed"); self.active_downloads = self.active_downloads.saturating_sub(1); }
                Command::none()
            }
            
            Message::PickCookieFile => { Command::perform(async { rfd::AsyncFileDialog::new().pick_file().await.map(|f| f.path().to_path_buf()) }, Message::CookieFilePicked) }
            Message::CookieFilePicked(p) => { self.cookie_path = p; Command::none() }
            Message::PickOutputDir => { Command::perform(async { rfd::AsyncFileDialog::new().pick_folder().await.map(|f| f.path().to_path_buf()) }, Message::OutputDirPicked) }
            Message::OutputDirPicked(p) => { if let Some(path) = p { self.output_dir = path; } Command::none() }
            Message::PickProxyList => { Command::perform(async { let file = rfd::AsyncFileDialog::new().pick_file().await; if let Some(f) = file { tokio::fs::read_to_string(f.path().to_path_buf()).await.ok() } else { None } }, Message::ProxyListLoaded) }
            Message::ProxyListLoaded(c) => { if let Some(text) = c { self.proxy_list = text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect(); } Command::none() }
            Message::CheckForUpdates => { self.tool_status = "Checking updates...".to_string(); Command::perform(auto_update_task(), Message::ToolInstalled) }
            // Removed unused install handlers logic to avoid dead code logic
            Message::ToolInstalled(res) => { match res { Ok(m) => self.tool_status = m, Err(e) => self.tool_status = format!("Error: {}", e) } Command::none() }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        if self.modal_live_url.is_some() { return self.view_live_modal(); }

        let tab_btn = |label, tab, active_tab| {
            let style = if tab == active_tab { theme::Button::Primary } else { theme::Button::Secondary };
            button(text(label).size(14).horizontal_alignment(alignment::Horizontal::Center)).width(Length::Fill).style(style).on_press(Message::TabChanged(tab))
        };
        let tabs = row![
            tab_btn("Dashboard", AppTab::Dashboard, self.current_tab),
            tab_btn("Audio", AppTab::Audio, self.current_tab),
            tab_btn("Video", AppTab::Video, self.current_tab),
            tab_btn("Advanced", AppTab::Advanced, self.current_tab),
        ].spacing(10).padding(10);

        let content = match self.current_tab {
            AppTab::Dashboard => self.view_dashboard(),
            AppTab::Audio => self.view_audio_settings(),
            AppTab::Video => self.view_video_settings(),
            AppTab::Advanced => self.view_advanced_settings(),
        };
        column![tabs, content].into()
    }
}

impl YtDownloader {
    fn view_live_modal(&self) -> Element<'_, Message> {
        let content = column![
            text("🔴 Livestream Detected!").size(24).font(iced::font::Font::with_name("bold")),
            text("How would you like to download this stream?").size(16),
            Space::with_height(20.0),
            button("From Current Moment (Live)").on_press(Message::LiveDecision(false)).style(theme::Button::Primary).padding(12).width(250),
            text("Captures what happens from now on.").size(12).style(theme::Text::Color(hex_color("#bac2de"))),
            Space::with_height(10.0),
            button("From Start (DVR)").on_press(Message::LiveDecision(true)).style(theme::Button::Secondary).padding(12).width(250),
            text("Attempts to download from the beginning (if supported).").size(12).style(theme::Text::Color(hex_color("#bac2de"))),
            Space::with_height(20.0),
            button("Cancel").on_press(Message::CloseModal).style(theme::Button::Destructive)
        ].spacing(10).padding(40).align_items(alignment::Alignment::Center);
        
        container(content).width(Length::Fill).height(Length::Fill).center_x().center_y().style(theme::Container::Custom(Box::new(DarkBackgroundStyle))).into()
    }

    fn view_dashboard(&self) -> Element<'_, Message> {
        let btn_text = if self.is_analyzing { "Analyzing..." } else { "Download" };
        let input_row = row![
            text_input("Paste Link...", &self.input_url).on_input(Message::UrlChanged).on_submit(Message::RequestAddUrl).padding(10),
            button(text(btn_text).size(16)).on_press_maybe(if self.is_analyzing { None } else { Some(Message::RequestAddUrl) }).style(theme::Button::Primary).padding(10)
        ].spacing(10);
        let items: Element<Message> = column(self.queue.iter().map(|item| {
            let (status_icon, status_color) = match &item.status {
                DownloadStatus::Queued => ("⏳", hex_color("#a6adc8")), DownloadStatus::Downloading => ("🚀", hex_color("#89b4fa")), DownloadStatus::Finished => ("✅", hex_color("#a6e3a1")), DownloadStatus::Failed(_) => ("❌", hex_color("#f38ba8")), DownloadStatus::Cancelled => ("⛔", hex_color("#fab387")),
            };
            let info_text = match &item.status {
                DownloadStatus::Failed(e) => format!("Error: {}", e), DownloadStatus::Downloading => format!("{} | {}", item.speed, item.total_size), DownloadStatus::Finished => format!("Completed: {}", item.total_size), _ => String::new(),
            };
            let buttons = match item.status {
                DownloadStatus::Downloading => row![button(text("✖").size(12)).on_press(Message::CancelDownload(item.id)).style(theme::Button::Destructive)],
                _ => row![button(text("↻").size(12)).on_press(Message::RetryDownload(item.id)).style(theme::Button::Secondary)]
            };
            
            container(column![
                row![text(status_icon), text(&item.title).width(Length::Fill).size(14), text(format!("{:.1}%", item.progress)).style(theme::Text::Color(status_color)), buttons].spacing(10).align_items(alignment::Alignment::Center),
                progress_bar(0.0..=100.0, item.progress).height(6).style(theme::ProgressBar::Custom(Box::new(BarStyle { color: status_color }))),
                text(info_text).size(10).style(theme::Text::Color(hex_color("#bac2de")))
            ].spacing(8)).style(theme::Container::Custom(Box::new(DarkCardStyle))).padding(12).into()
        }).collect::<Vec<_>>()).spacing(10).into();
        let footer = row![text(&self.tool_status).size(12).style(theme::Text::Color(hex_color("#fab387"))), Space::with_width(Length::Fill), button("Update Tools").on_press(Message::CheckForUpdates).style(theme::Button::Destructive).padding(5)].align_items(alignment::Alignment::Center);
        
        container(column![input_row, Space::with_height(10.0), scrollable(items), Space::with_height(10.0), footer].padding(20)).style(theme::Container::Custom(Box::new(DarkBackgroundStyle))).width(Length::Fill).height(Length::Fill).into()
    }

    fn view_audio_settings(&self) -> Element<'_, Message> {
        let col = column![
            text("Audio Extraction").size(20).font(iced::font::Font::with_name("bold")), Space::with_height(10.0),
            row![text("Format:"), pick_list(&AudioFormat::ALL[..], Some(self.settings.audio_fmt), Message::AudioFmtChanged)].spacing(20).align_items(alignment::Alignment::Center),
        ].spacing(20).padding(20);
        
        container(col).style(theme::Container::Custom(Box::new(DarkBackgroundStyle))).width(Length::Fill).height(Length::Fill).into()
    }

    fn view_video_settings(&self) -> Element<'_, Message> {
        let col = column![
            text("Video & Post-Processing").size(20).font(iced::font::Font::with_name("bold")),
            row![text("Container Preference:"), pick_list(&crate::types::Container::ALL[..], Some(self.settings.container), Message::ContainerChanged)].spacing(20),
            row![text("Video Type:"), pick_list(&VideoType::ALL[..], Some(self.settings.video_type), Message::VideoTypeChanged)].spacing(20),
            
            row![text("Filename Format:"), pick_list(&FilenameTemplate::ALL[..], Some(self.settings.filename_style), Message::FilenameStyleChanged)].spacing(20),
            
            column![
                toggler(Some("Embed Subtitles".to_string()), self.settings.embed_subs, Message::ToggleEmbedSubs).width(Length::Fill),
                {
                    let content: Element<Message> = if self.settings.embed_subs {
                        row![
                            text("Languages (e.g. en,ja):"), 
                            text_input("all", &self.settings.sub_langs)
                                .on_input(Message::SubLangsChanged)
                                .width(100)
                        ].spacing(10).into()
                    } else {
                        <iced::widget::Space as Into<Element<Message>>>::into(Space::with_height(Length::Fixed(0.0)))
                    };
                    content
                }
            ].spacing(5),

            toggler(Some("Embed Metadata".to_string()), self.settings.embed_meta, Message::ToggleEmbedMeta).width(Length::Fill),
            toggler(Some("Embed Thumbnail".to_string()), self.settings.embed_thumb, Message::ToggleEmbedThumb).width(Length::Fill),
        ].spacing(20).padding(20);
        
        container(col).style(theme::Container::Custom(Box::new(DarkBackgroundStyle))).width(Length::Fill).height(Length::Fill).into()
    }

    fn view_advanced_settings(&self) -> Element<'_, Message> {
        let col = column![
            text("Advanced & Network").size(20).font(iced::font::Font::with_name("bold")),
            row![button("Output Folder").on_press(Message::PickOutputDir), text(self.output_dir.to_string_lossy()).size(12)].spacing(10).align_items(alignment::Alignment::Center),
            row![button("Cookie File").on_press(Message::PickCookieFile), text(self.cookie_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or("None".into())).size(12)].spacing(10).align_items(alignment::Alignment::Center),
            
            row![text("Proxy Protocol:"), pick_list(&ProxyProtocol::ALL[..], Some(self.selected_proxy_proto), Message::ProxyProtocolChanged)].spacing(20).align_items(alignment::Alignment::Center),
            row![text("Manual Proxy"), text_input("ip:port:user:pass", &self.manual_proxy).on_input(Message::ManualProxyChanged)].spacing(10).align_items(alignment::Alignment::Center),
            row![button("Load Proxy List").on_press(Message::PickProxyList), text(format!("{} Loaded", self.proxy_list.len()))].spacing(10).align_items(alignment::Alignment::Center),
            
            row![text("Max Downloads:"), text_input("3", &self.max_concurrent_input).on_input(Message::MaxConcurrentChanged).width(50)].spacing(10).align_items(alignment::Alignment::Center),

            toggler(Some("Use SponsorBlock (Remove Ads)".to_string()), self.settings.sponsorblock, Message::ToggleSponsorBlock).width(Length::Fill),
            text("Playlist Items (e.g. 1,2,5-10):"), text_input("1-10", &self.settings.playlist_items).on_input(Message::PlaylistItemsChanged),
            text("Rate Limit (e.g. 5M, 500K):"), text_input("Unlimited", &self.settings.rate_limit).on_input(Message::RateLimitChanged),
            text("Custom Arguments (Paste extra flags here):"), text_input("--geo-bypass --user-agent ...", &self.settings.custom_args).on_input(Message::CustomArgsChanged),
        ].spacing(15).padding(20);
        
        container(scrollable(col)).style(theme::Container::Custom(Box::new(DarkBackgroundStyle))).width(Length::Fill).height(Length::Fill).into()
    }
}