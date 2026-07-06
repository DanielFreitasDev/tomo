//! Streamed response capture with a memory cap and temp-file spill.

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use tokio_util::sync::CancellationToken;

use crate::CoreError;
use crate::model::{BodyCapture, ResponseData, Timing};

pub struct CaptureConfig {
    /// Bytes kept in memory for preview/parsing.
    pub cap_bytes: u64,
    /// Directory for spill files (full bodies beyond the cap).
    pub spill_dir: PathBuf,
}

pub async fn capture(
    resp: reqwest::Response,
    cfg: &CaptureConfig,
    cancel: &CancellationToken,
    started: Instant,
    timeout_ms: u64,
) -> Result<ResponseData, CoreError> {
    let ttfb = started.elapsed();

    let status = resp.status();
    let http_version = format!("{:?}", resp.version());
    let final_url = resp.url().to_string();

    let headers: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                String::from_utf8_lossy(v.as_bytes()).into_owned(),
            )
        })
        .collect();

    let (mime, charset) = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| parse_content_type(v))
        .unwrap_or((None, None));

    let mut body = BodyCapture {
        mime,
        charset,
        ..Default::default()
    };

    let mut resp = resp;
    let mut spill: Option<(std::fs::File, PathBuf)> = None;
    let download_started = Instant::now();

    // Stream the body; on ANY early return (I/O error or cancellation) remove a
    // partial spill file so it never leaks to disk.
    if let Err(e) = stream_body(&mut resp, cfg, cancel, &mut body, &mut spill, timeout_ms).await {
        if let Some((_, path)) = &spill {
            let _ = std::fs::remove_file(path);
        }
        return Err(e);
    }

    if let Some((file, path)) = spill {
        file.sync_all().map_err(|e| CoreError::io(&path, e))?;
        body.spill_path = Some(path);
    }

    body.is_binary = body.bytes.iter().take(8192).any(|b| *b == 0);

    let total = started.elapsed();
    Ok(ResponseData {
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or("").to_string(),
        http_version,
        headers,
        final_url,
        timing: Timing {
            total_ms: total.as_millis() as u64,
            ttfb_ms: ttfb.as_millis() as u64,
            download_ms: download_started.elapsed().as_millis() as u64,
        },
        body,
        warnings: Vec::new(),
        console: Vec::new(),
        tests: Vec::new(),
        asserts: Vec::new(),
        script_error: None,
        runtime_sets: indexmap::IndexMap::new(),
    })
}

/// Stream the response into the preview buffer (up to the cap) and, beyond the
/// cap, into a spill file. Kept separate so the caller can remove a partial
/// spill on any error return.
async fn stream_body(
    resp: &mut reqwest::Response,
    cfg: &CaptureConfig,
    cancel: &CancellationToken,
    body: &mut BodyCapture,
    spill: &mut Option<(std::fs::File, PathBuf)>,
    timeout_ms: u64,
) -> Result<(), CoreError> {
    loop {
        let chunk = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(CoreError::Cancelled),
            chunk = resp.chunk() => chunk.map_err(|e| CoreError::from_reqwest(e, timeout_ms))?,
        };
        let Some(chunk) = chunk else { break };

        body.total_size += chunk.len() as u64;

        // preview buffer up to the cap
        let room = (cfg.cap_bytes as usize).saturating_sub(body.bytes.len());
        if room > 0 {
            body.bytes
                .extend_from_slice(&chunk[..room.min(chunk.len())]);
        }

        if body.total_size > cfg.cap_bytes {
            body.truncated = true;
            if spill.is_none() {
                std::fs::create_dir_all(&cfg.spill_dir)
                    .map_err(|e| CoreError::io(&cfg.spill_dir, e))?;
                let path = cfg
                    .spill_dir
                    .join(format!("tomo-body-{}.bin", uuid::Uuid::new_v4()));
                let mut file = std::fs::File::create(&path).map_err(|e| CoreError::io(&path, e))?;
                // backfill what we already buffered
                file.write_all(&body.bytes)
                    .map_err(|e| CoreError::io(&path, e))?;
                *spill = Some((file, path));
            }
            if let Some((file, path)) = spill.as_mut() {
                // the preview buffer already holds the head; spill gets everything
                let already = body.total_size - chunk.len() as u64;
                let skip = (cfg.cap_bytes).saturating_sub(already) as usize;
                let skip = skip.min(chunk.len());
                // bytes below the cap were backfilled above on spill creation;
                // for later chunks skip == 0 and the whole chunk is written
                file.write_all(&chunk[skip..])
                    .map_err(|e| CoreError::io(path.as_path(), e))?;
            }
        }
    }
    Ok(())
}

fn parse_content_type(value: &str) -> (Option<String>, Option<String>) {
    match value.parse::<mime::Mime>() {
        Ok(m) => {
            let essence = m.essence_str().to_string();
            let charset = m.get_param(mime::CHARSET).map(|c| c.as_str().to_string());
            (Some(essence), charset)
        }
        Err(_) => (Some(value.to_string()), None),
    }
}
