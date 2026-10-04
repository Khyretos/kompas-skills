//! W4: optional local voice. Speech to text with Whisper on OVMS (GPU), text to speech
//! with Kokoro on OVMS (CPU). Audio only passes through memory; nothing is stored.
//! (Handlers drafted by Qwen3.5 9B; the WAV code was rewritten in review.)
use axum::{Json, body::Bytes, extract::{Query, State}, http::header, response::{IntoResponse, Response}};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{process::Stdio, time::Duration};
use tokio::io::AsyncWriteExt;
use crate::{AppState, error::{ApiError, ApiResult}};

fn env(k: &str, d: &str) -> String {
    std::env::var(k).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| d.to_string())
}

fn enabled() -> bool {
    env("VOICE", "on") != "off"
}

fn authed(r: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    match std::env::var("OVMS_API_KEY").ok().filter(|k| !k.is_empty()) {
        Some(k) => r.bearer_auth(k),
        None => r,
    }
}

/// Kokoro voices offered in Settings (English first, then Spanish; Kokoro has no Dutch).
pub const VOICES: &[(&str, &str)] = &[
    ("af_heart", "English (US), Heart"),
    ("am_michael", "English (US), Michael"),
    ("bf_emma", "English (UK), Emma"),
    ("bm_george", "English (UK), George"),
    ("ef_dora", "Spanish, Dora"),
    ("em_alex", "Spanish, Alex"),
];

fn voice_or_default(v: &str) -> &'static str {
    VOICES.iter().find(|(id, _)| *id == v).map(|(id, _)| *id).unwrap_or("af_heart")
}

pub async fn info() -> Json<Value> {
    Json(json!({
        "enabled": enabled(),
        "voices": VOICES.iter().map(|(id, label)| json!({ "id": id, "label": label })).collect::<Vec<_>>(),
    }))
}

#[derive(Deserialize)]
pub struct SttQuery {
    #[serde(default)]
    lang: Option<String>,
}

/// Any recording the browser makes (webm/opus, ogg, mp4/aac, wav) as 16 kHz mono PCM WAV.
async fn to_wav16k(audio: Bytes) -> anyhow::Result<Vec<u8>> {
    let mut child = tokio::process::Command::new("ffmpeg")
        .args(["-v", "error", "-nostdin", "-i", "pipe:0", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le", "-f", "wav", "pipe:1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    // Write from a task: ffmpeg's stdout must be read at the same time, or a long
    // recording fills the pipe and both sides wait forever.
    let mut stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("no stdin"))?;
    tokio::spawn(async move {
        let _ = stdin.write_all(&audio).await;
    });
    let out = match tokio::time::timeout(Duration::from_secs(30), child.wait_with_output()).await {
        Ok(out) => out?,
        Err(_) => anyhow::bail!("audio conversion took too long"),
    };
    if !out.status.success() {
        let err: String = String::from_utf8_lossy(&out.stderr).chars().take(200).collect();
        anyhow::bail!("{}", err.trim());
    }
    Ok(out.stdout)
}

pub async fn transcribe(State(s): State<AppState>, Query(q): Query<SttQuery>, body: Bytes) -> ApiResult<Json<Value>> {
    if !enabled() {
        return Err(ApiError::BadRequest("Voice is off on this server.".into()));
    }
    if body.is_empty() {
        return Err(ApiError::BadRequest("No audio.".into()));
    }
    let wav = to_wav16k(body).await.map_err(|e| ApiError::BadRequest(format!("That recording could not be read: {e}")))?;
    let mut form = reqwest::multipart::Form::new()
        .text("model", env("VOICE_STT_MODEL", "Whisper"))
        .part("file", reqwest::multipart::Part::bytes(wav).file_name("speech.wav").mime_str("audio/wav").map_err(|e| ApiError::Internal(e.into()))?);
    if let Some(lang) = q.lang.filter(|l| ["en", "es", "nl"].contains(&l.as_str())) {
        form = form.text("language", lang);
    }
    let url = format!("{}/audio/transcriptions", env("VOICE_STT_URL", "http://ovms:8000/v3"));
    let r = authed(s.http.post(url)).multipart(form).timeout(Duration::from_secs(60)).send().await.map_err(|e| ApiError::Internal(e.into()))?;
    if !r.status().is_success() {
        return Err(ApiError::BadRequest(format!("Speech to text failed: {}", r.status())));
    }
    let v: Value = r.json().await.map_err(|e| ApiError::Internal(e.into()))?;
    Ok(Json(json!({ "text": v["text"].as_str().unwrap_or("").trim() })))
}

#[derive(Deserialize)]
pub struct SpeakBody {
    text: String,
    #[serde(default)]
    voice: String,
}

pub async fn speak(State(s): State<AppState>, Json(b): Json<SpeakBody>) -> ApiResult<Response> {
    if !enabled() {
        return Err(ApiError::BadRequest("Voice is off on this server.".into()));
    }
    let text = b.text.trim();
    if text.is_empty() {
        return Err(ApiError::BadRequest("Nothing to read.".into()));
    }
    if text.chars().count() > 600 {
        return Err(ApiError::BadRequest("Read at most 600 characters at a time.".into()));
    }
    let url = format!("{}/audio/speech", env("VOICE_TTS_URL", "http://ovms-cpu:8000/v3"));
    let body = json!({ "model": env("VOICE_TTS_MODEL", "Voice"), "input": text, "voice": voice_or_default(&b.voice) });
    let r = authed(s.http.post(url)).json(&body).timeout(Duration::from_secs(60)).send().await.map_err(|e| ApiError::Internal(e.into()))?;
    if !r.status().is_success() {
        return Err(ApiError::BadRequest(format!("Text to speech failed: {}", r.status())));
    }
    let bytes = r.bytes().await.map_err(|e| ApiError::Internal(e.into()))?;
    let wav = to_pcm16(&bytes).unwrap_or_else(|| bytes.to_vec());
    Ok(([(header::CONTENT_TYPE, "audio/wav"), (header::CACHE_CONTROL, "no-store")], wav).into_response())
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// Kokoro answers 32-bit float WAV, which not every browser plays: convert it to 16-bit PCM.
/// None when it isn't a float WAV (already PCM, or not a WAV at all).
pub fn to_pcm16(wav: &[u8]) -> Option<Vec<u8>> {
    if wav.get(0..4)? != b"RIFF" || wav.get(8..12)? != b"WAVE" {
        return None;
    }
    let (mut format, mut channels, mut rate, mut bits, mut data) = (None, 0u16, 0u32, 0u16, None);
    let mut at = 12;
    while at + 8 <= wav.len() {
        let id = &wav[at..at + 4];
        let size = u32_at(wav, at + 4)? as usize;
        let body = at + 8;
        if id == b"fmt " {
            format = u16_at(wav, body);
            channels = u16_at(wav, body + 2)?;
            rate = u32_at(wav, body + 4)?;
            bits = u16_at(wav, body + 14)?;
        } else if id == b"data" {
            // Some writers leave the size at 0 or too big for a stream: take what is there.
            let end = if size == 0 { wav.len() } else { (body + size).min(wav.len()) };
            data = Some(&wav[body..end]);
            break;
        }
        at = body + size + (size & 1);
    }
    if format? != 3 || bits != 32 || channels == 0 {
        return None;
    }
    let samples: Vec<u8> = data?
        .chunks_exact(4)
        .flat_map(|c| ((f32::from_le_bytes([c[0], c[1], c[2], c[3]]).clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes())
        .collect();
    let len = samples.len() as u32;
    let mut out = Vec::with_capacity(44 + samples.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&samples);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WAV with a fmt chunk (format, bits) and the given sample bytes.
    fn wav(format: u16, bits: u16, rate: u32, data: &[u8]) -> Vec<u8> {
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&format.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&rate.to_le_bytes());
        w.extend_from_slice(&(rate * bits as u32 / 8).to_le_bytes());
        w.extend_from_slice(&(bits / 8).to_le_bytes());
        w.extend_from_slice(&bits.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&(data.len() as u32).to_le_bytes());
        w.extend_from_slice(data);
        w
    }

    #[test]
    fn float_wav_becomes_16_bit_pcm() {
        let floats: Vec<u8> = [0.5f32, -1.0, 2.0].iter().flat_map(|f| f.to_le_bytes()).collect();
        let pcm = to_pcm16(&wav(3, 32, 24000, &floats)).unwrap();
        assert_eq!(pcm.len(), 44 + 6);
        assert_eq!(u16_at(&pcm, 20), Some(1)); // PCM
        assert_eq!(u32_at(&pcm, 24), Some(24000));
        assert_eq!(u32_at(&pcm, 28), Some(48000)); // byte rate
        assert_eq!(u16_at(&pcm, 34), Some(16));
        assert_eq!(&pcm[36..40], b"data");
        assert_eq!(u32_at(&pcm, 40), Some(6));
        let s: Vec<i16> = pcm[44..].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        assert_eq!(s, [16384, -32767, 32767]); // 0.5 rounds up; 2.0 is clamped
    }

    #[test]
    fn a_list_chunk_before_data_is_skipped() {
        let floats: Vec<u8> = 0.25f32.to_le_bytes().to_vec();
        let mut w = wav(3, 32, 16000, &floats);
        // Insert an odd-sized LIST chunk (padded to even) between fmt and data.
        let list = [b"LIST".as_slice(), &3u32.to_le_bytes(), b"abc\0"].concat();
        w.splice(36..36, list);
        let pcm = to_pcm16(&w).unwrap();
        assert_eq!(i16::from_le_bytes([pcm[44], pcm[45]]), 8192);
    }

    #[test]
    fn pcm_and_other_bytes_are_left_alone() {
        assert_eq!(to_pcm16(&wav(1, 16, 8000, &[1, 0, 2, 0])), None);
        assert_eq!(to_pcm16(b"not a wav"), None);
        assert_eq!(to_pcm16(b""), None);
    }

    #[test]
    fn unknown_voices_fall_back_to_heart() {
        assert_eq!(voice_or_default("ef_dora"), "ef_dora");
        assert_eq!(voice_or_default("x"), "af_heart");
    }
}
