use iced::Subscription;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;
use regex::Regex;
use once_cell::sync::Lazy;
use crate::types::{Message, AdvOptions, AudioFormat, VideoType};

static PROGRESS_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(\d+(?:\.\d+)?)%\s+of\s+([~0-9a-zA-Z\.]+)(?:\s+at\s+([0-9a-zA-Z\./]+))?").unwrap()
});

pub async fn analyze_url_task(url: String, proxy: Option<String>, cookie: Option<PathBuf>) -> Result<serde_json::Value, String> {
    let local_yt = std::env::current_dir().unwrap_or_default().join("yt-dlp.exe");
    let cmd_name = if local_yt.exists() { local_yt.to_string_lossy().to_string() } else { "yt-dlp".to_string() };

    let mut cmd = TokioCommand::new(cmd_name);
    cmd.arg("-J").arg("--flat-playlist").arg(&url);

    if let Some(p) = proxy { cmd.arg("--proxy").arg(p); }
    if let Some(c) = cookie { cmd.arg("--cookies").arg(c); }
    #[cfg(windows)] cmd.creation_flags(0x08000000);

    let output = cmd.output().await.map_err(|e| format!("Execution failed: {}", e))?;
    
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    let json_text = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&json_text).map_err(|e| format!("JSON Parse Error: {}", e))
}

pub fn download_stream(id: usize, url: String, dir: PathBuf, proxy: Option<String>, cookie: Option<PathBuf>, opts: AdvOptions) -> Subscription<Message> {
    iced::subscription::unfold(id, State::Starting, move |state| {
        let url = url.clone(); let dir = dir.clone(); let proxy = proxy.clone(); let cookie = cookie.clone(); let opts = opts.clone();
        async move {
            match state {
                State::Starting => {
                    let local_yt = std::env::current_dir().unwrap_or_default().join("yt-dlp.exe");
                    let cmd_name = if local_yt.exists() { local_yt.to_string_lossy().to_string() } else { "yt-dlp".to_string() };
                    let mut cmd = TokioCommand::new(cmd_name);
                    cmd.arg("--newline").arg("--encoding").arg("utf-8").arg("--no-overwrites").arg("--ignore-errors").arg("-P").arg(&dir).arg(&url);
                    
                    if let Some(p) = &proxy { cmd.arg("--proxy").arg(p); }
                    if let Some(c) = &cookie { cmd.arg("--cookies").arg(c); }
                    if let Ok(local_exe) = std::env::current_dir().map(|d| d.join("ffmpeg.exe")) { if local_exe.exists() { cmd.arg("--ffmpeg-location").arg(local_exe); } }

                    // Apply Filename Template
                    if let Some(tmpl) = opts.filename_style.to_cmd_arg() {
                        cmd.arg("-o").arg(tmpl);
                    }

                    match opts.audio_fmt {
                        AudioFormat::None => {
                            let container = opts.container.as_str();
                            // Video Type Logic (VR/3D)
                            match opts.video_type {
                                VideoType::Normal => { cmd.arg("-f").arg("bestvideo+bestaudio/best"); },
                                VideoType::VR360 => { 
                                    // Prefer VR formats
                                    cmd.arg("-S").arg("vr,res,fps,codec"); 
                                    cmd.arg("-f").arg("bestvideo+bestaudio/best");
                                },
                                VideoType::ThreeD => {
                                    // Prefer 3D formats
                                    cmd.arg("-S").arg("3d,res,fps,codec"); 
                                    cmd.arg("-f").arg("bestvideo+bestaudio/best");
                                }
                            }
                            
                            cmd.arg("--merge-output-format").arg(container);
                            if opts.embed_subs { 
                                cmd.arg("--embed-subs"); 
                                if !opts.sub_langs.is_empty() { cmd.arg("--sub-langs").arg(&opts.sub_langs); }
                            }
                            if opts.embed_meta { cmd.arg("--embed-metadata"); }
                            if opts.embed_thumb { cmd.arg("--embed-thumbnail"); }
                        },
                        fmt => {
                            cmd.arg("-x").arg("--audio-format").arg(fmt.as_str()).arg("--audio-quality").arg("0");
                            if opts.embed_meta { cmd.arg("--embed-metadata"); }
                            if opts.embed_thumb { cmd.arg("--embed-thumbnail"); }
                        }
                    }
                    if opts.sponsorblock { cmd.arg("--sponsorblock-remove").arg("all"); }
                    if !opts.rate_limit.is_empty() { cmd.arg("-r").arg(&opts.rate_limit); }
                    if !opts.custom_args.is_empty() { for arg in opts.custom_args.split_whitespace() { cmd.arg(arg); } }

                    #[cfg(windows)] cmd.creation_flags(0x08000000); 
                    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
                    match cmd.spawn() {
                        Ok(mut child) => {
                            let stdout = child.stdout.take().unwrap();
                            (Message::DownloadProgress(id, 0.0, String::new(), String::new()), State::Running(BufReader::new(stdout), child))
                        }
                        Err(e) => (Message::DownloadFailed(id, format!("Startup Fail: {}", e)), State::Finished),
                    }
                }
                State::Running(mut reader, mut child) => {
                    let mut line_buf = Vec::new();
                    use tokio::io::AsyncReadExt; 
                    match reader.read_until(b'\n', &mut line_buf).await {
                        Ok(0) => {
                            let status = child.wait().await.ok();
                            if let Some(s) = status { if s.success() { return (Message::DownloadFinished(id), State::Finished); } }
                            let mut err_msg = String::new();
                            if let Some(mut stderr) = child.stderr.take() { let _ = stderr.read_to_string(&mut err_msg).await; }
                            if err_msg.trim().is_empty() { err_msg = "Unknown error (Non-zero exit)".to_string(); }
                            else { if let Some(last) = err_msg.lines().last() { err_msg = last.to_string(); } }
                            (Message::DownloadFailed(id, err_msg), State::Finished)
                        }
                        Ok(_) => {
                            let line = String::from_utf8_lossy(&line_buf);
                            if let Some(caps) = PROGRESS_RE.captures(&line) {
                                if let Some(m) = caps.get(1) {
                                    if let Ok(p) = m.as_str().parse::<f32>() {
                                        let size = caps.get(2).map(|x| x.as_str()).unwrap_or("?");
                                        let speed = caps.get(3).map(|x| x.as_str()).unwrap_or("?");
                                        return (Message::DownloadProgress(id, p, speed.to_string(), size.to_string()), State::Running(reader, child));
                                    }
                                }
                            }
                            (Message::Ignore, State::Running(reader, child))
                        }
                        Err(e) => (Message::DownloadFailed(id, format!("IO Error: {}", e)), State::Finished),
                    }
                }
                State::Finished => { std::future::pending::<()>().await; (Message::Ignore, State::Finished) }
            }
        }
    })
}

enum State { Starting, Running(BufReader<tokio::process::ChildStdout>, tokio::process::Child), Finished }