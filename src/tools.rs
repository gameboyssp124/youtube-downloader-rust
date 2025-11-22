use tokio::process::Command as TokioCommand;
use regex::Regex;
use once_cell::sync::Lazy;
use crate::types::{GitHubRelease, ProxyProtocol};

static FFMPEG_DATE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"ffmpeg version (\d{4}-\d{2}-\d{2})").unwrap()
});

pub fn format_proxy(raw: &str, protocol: ProxyProtocol) -> String {
    if raw.contains("://") { return raw.to_string(); }
    let p: Vec<&str> = raw.split(':').collect();
    let scheme = protocol.as_str();
    if p.len() == 4 { format!("{}://{}:{}@{}:{}", scheme, p[2], p[3], p[0], p[1]) } 
    else if p.len() == 2 { format!("{}://{}:{}", scheme, p[0], p[1]) } 
    else { raw.to_string() }
}

pub async fn auto_update_task() -> Result<String, String> {
    let y = check_and_update_ytdlp().await;
    let f = check_and_update_ffmpeg().await;
    match (y, f) {
        (Ok(y), Ok(f)) => Ok(format!("yt-dlp: {}, FFmpeg: {}", y, f)),
        (Err(e), _) => Err(format!("yt-dlp error: {}", e)),
        (_, Err(e)) => Err(format!("FFmpeg error: {}", e))
    }
}

async fn check_and_update_ytdlp() -> Result<String, String> {
    let local = if let Ok(o) = TokioCommand::new("yt-dlp").arg("--version").output().await {
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    } else { "0".into() };

    let client = reqwest::Client::new();
    let resp = client.get("https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest")
        .header("User-Agent", "rust-updater")
        .send().await.map_err(|e|e.to_string())?
        .json::<GitHubRelease>().await.map_err(|e|e.to_string())?;

    if local != resp.tag_name { download_ytdlp_task().await?; Ok(format!("Updated to {}", resp.tag_name)) } 
    else { Ok("Up to date".into()) }
}

async fn check_and_update_ffmpeg() -> Result<String, String> {
    let local = if let Ok(o) = TokioCommand::new("ffmpeg").arg("-version").output().await {
        let t = String::from_utf8_lossy(&o.stdout);
        FFMPEG_DATE_RE.captures(&t).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()).unwrap_or("0".into())
    } else { "0".into() };

    let client = reqwest::Client::new();
    let resp = client.get("https://api.github.com/repos/GyanD/codexffmpeg/releases/latest")
        .header("User-Agent", "rust-updater")
        .send().await.map_err(|e|e.to_string())?
        .json::<GitHubRelease>().await.map_err(|e|e.to_string())?;

    if local != resp.tag_name { download_ffmpeg_task().await?; Ok(format!("Updated to {}", resp.tag_name)) } 
    else { Ok("Up to date".into()) }
}

pub async fn download_ytdlp_task() -> Result<String, String> {
    let url = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
    let b = reqwest::get(url).await.map_err(|e|e.to_string())?.bytes().await.map_err(|e|e.to_string())?;
    tokio::fs::write("yt-dlp.exe", b).await.map_err(|e|e.to_string())?;
    Ok("Installed".into())
}

pub async fn download_ffmpeg_task() -> Result<String, String> {
    let url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
    let b = reqwest::get(url).await.map_err(|e|e.to_string())?.bytes().await.map_err(|e|e.to_string())?;
    let mut a = zip::ZipArchive::new(std::io::Cursor::new(b)).map_err(|e|e.to_string())?;
    for i in 0..a.len() {
        let mut f = a.by_index(i).map_err(|e|e.to_string())?;
        if f.enclosed_name().unwrap_or(std::path::Path::new("")).file_name().unwrap_or_default() == "ffmpeg.exe" {
            let mut o = std::fs::File::create("ffmpeg.exe").map_err(|e|e.to_string())?;
            std::io::copy(&mut f, &mut o).map_err(|e|e.to_string())?;
            return Ok("Installed".into());
        }
    }
    Err("Not in zip".into())
}