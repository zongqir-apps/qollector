use crate::core::clipboard_manager::ClipboardManager;
use crate::ocr::native::NativeOcr;
use crate::ocr::provider::OcrProvider;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MONITOR_POLL_INTERVAL: Duration = Duration::from_millis(500);
const SETTLE_DELAY: Duration = Duration::from_millis(3000);
const MAX_SETTLE_DELAY: Duration = Duration::from_millis(8000);

pub struct ChatContextCaptureState {
    enabled: AtomicBool,
    mode: parking_lot::RwLock<String>,
}

impl ChatContextCaptureState {
    pub fn new(enabled: bool, mode: String) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            mode: parking_lot::RwLock::new(mode),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_mode(&self, mode: &str) {
        let mut m = self.mode.write();
        *m = mode.to_string();
    }

    pub fn mode(&self) -> String {
        self.mode.read().clone()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredDialogueMessage {
    pub speaker: String,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextSnapshotPayload<'a> {
    pub captured_at: String,
    pub source_app: &'a str,
    pub window_title: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_format: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<StoredDialogueMessage>>,
}

#[cfg(target_os = "windows")]
mod platform_impl {
    use std::io::Cursor;
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    extern "system" {
        fn PrintWindow(hwnd: HWND, hdcblt: HDC, nflags: u32) -> BOOL;
    }

    pub struct WindowCaptureResult {
        pub hash: String,
        pub sample: Vec<u8>,
        pub width: i32,
        pub height: i32,
        pub jpeg_bytes: Vec<u8>,
    }

    pub fn get_foreground_hwnd() -> Option<isize> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                None
            } else {
                Some(hwnd.0 as isize)
            }
        }
    }

    pub fn is_chat_target(source_app: &str, window_title: &str) -> bool {
        let app = source_app.to_lowercase();
        let title = window_title.to_lowercase();

        app.contains("微信")
            || app.contains("wechat")
            || app.contains("企业微信")
            || app.contains("wxwork")
            || app.contains("wecom")
            || title.contains("微信")
            || title.contains("企业微信")
    }

    pub fn capture_window(hwnd_val: isize) -> Option<WindowCaptureResult> {
        unsafe {
            let hwnd = HWND(hwnd_val as *mut core::ffi::c_void);
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err() {
                return None;
            }
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            if width <= 100 || height <= 100 {
                return None;
            }

            let hdc_screen = GetDC(None);
            if hdc_screen.is_invalid() {
                return None;
            }
            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let hbm = CreateCompatibleBitmap(hdc_screen, width, height);
            let old_obj = SelectObject(hdc_mem, hbm);

            // Try PrintWindow first (PW_RENDERFULLCONTENT = 2)
            let printed = PrintWindow(hwnd, hdc_mem, 2);
            if !printed.as_bool() {
                let hdc_window = GetDC(hwnd);
                let _ = BitBlt(hdc_mem, 0, 0, width, height, hdc_window, 0, 0, SRCCOPY);
                let _ = ReleaseDC(hwnd, hdc_window);
            }

            let mut bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut buffer: Vec<u8> = vec![0; (width * height * 4) as usize];
            let lines = GetDIBits(
                hdc_mem,
                hbm,
                0,
                height as u32,
                Some(buffer.as_mut_ptr() as *mut _),
                &mut bi,
                DIB_RGB_COLORS,
            );

            SelectObject(hdc_mem, old_obj);
            let _ = DeleteObject(hbm);
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(None, hdc_screen);

            if lines <= 0 {
                return None;
            }

            // 1) Fast 32x32 stride grayscale thumbnail for instant diff
            // Exclude only the bottom input box area (up to 120px or 20% max) to avoid clipping newest message bubbles
            let input_box_height = 120.min(height / 5);
            let sample_height = (height - input_box_height).max(1);
            let mut sample = Vec::with_capacity(32 * 32);
            for y in 0..32 {
                let row = (y * sample_height / 32) as usize;
                for x in 0..32 {
                    let col = (x * width / 32) as usize;
                    let idx = (row * width as usize + col) * 4;
                    if idx + 2 < buffer.len() {
                        let gray = ((buffer[idx + 2] as u32 * 77
                            + buffer[idx + 1] as u32 * 150
                            + buffer[idx] as u32 * 29)
                            >> 8) as u8;
                        sample.push(gray);
                    }
                }
            }
            let hash = blake3::hash(&sample).to_hex().to_string();

            // 2) Fast convert BGRA to RGB8 for JPEG compression
            let mut rgb_buffer = Vec::with_capacity((width * height * 3) as usize);
            for chunk in buffer.chunks_exact(4) {
                rgb_buffer.push(chunk[2]); // R
                rgb_buffer.push(chunk[1]); // G
                rgb_buffer.push(chunk[0]); // B
            }

            let rgb_image = image::RgbImage::from_raw(width as u32, height as u32, rgb_buffer)?;
            let dynamic_img = image::DynamicImage::ImageRgb8(rgb_image);
            let mut jpeg_buf = Cursor::new(Vec::new());
            dynamic_img.write_to(&mut jpeg_buf, image::ImageFormat::Jpeg).ok()?;

            Some(WindowCaptureResult {
                hash,
                sample,
                width,
                height,
                jpeg_bytes: jpeg_buf.into_inner(),
            })
        }
    }
}

async fn send_context_snapshot_to_collector(
    endpoint: &str,
    token: &str,
    payload: &ContextSnapshotPayload<'_>,
) -> Result<(), String> {
    let url = format!("{}/v1/context-snapshots", endpoint.trim_end_matches('/'));
    let response = crate::http_client::client()
        .post(url)
        .header("Authorization", format!("Bearer {}", token))
        .json(payload)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("Upload snapshot failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Collector returned HTTP {}", response.status()));
    }

    Ok(())
}

async fn dispatch_chat_snapshot(
    manager: Arc<ClipboardManager>,
    state: Arc<ChatContextCaptureState>,
    app_name: String,
    title: String,
    jpeg_data: Vec<u8>,
) {
    let collector_cfg = manager.config_repo().load_collector_config().await.ok().flatten();
    if let Some(cfg) = collector_cfg {
        if cfg.enabled && !cfg.endpoint.is_empty() && !cfg.token.is_empty() {
            let current_mode = state.mode();
            tauri::async_runtime::spawn(async move {
                if current_mode == "local_ocr" {
                    let ocr = NativeOcr::new();
                    match ocr.recognize_bytes(&jpeg_data).await {
                        Ok(res) => {
                            let mut messages = Vec::new();
                            for line in res.text.lines() {
                                let trimmed = line.trim();
                                if trimmed.is_empty() {
                                    continue;
                                }
                                messages.push(StoredDialogueMessage {
                                    speaker: "chat".to_string(),
                                    role: "user".to_string(),
                                    content: trimmed.to_string(),
                                });
                            }
                            if !messages.is_empty() {
                                let payload = ContextSnapshotPayload {
                                    captured_at: chrono::Utc::now().to_rfc3339(),
                                    source_app: &app_name,
                                    window_title: &title,
                                    engine: Some("local_ocr"),
                                    image_format: None,
                                    image_base64: None,
                                    messages: Some(messages),
                                };
                                if let Err(e) = send_context_snapshot_to_collector(
                                    &cfg.endpoint,
                                    &cfg.token,
                                    &payload,
                                ).await {
                                    eprintln!("[ChatContextCapture] Upload local OCR failed: {e}");
                                } else {
                                    println!("[ChatContextCapture] Successfully uploaded local OCR snapshot for {app_name} - {title}");
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("[ChatContextCapture] Local OCR recognition failed: {e}");
                        }
                    }
                } else {
                    use base64::Engine;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg_data);
                    let payload = ContextSnapshotPayload {
                        captured_at: chrono::Utc::now().to_rfc3339(),
                        source_app: &app_name,
                        window_title: &title,
                        engine: Some("cloud_llm"),
                        image_format: Some("jpeg"),
                        image_base64: Some(b64),
                        messages: None,
                    };
                    if let Err(e) = send_context_snapshot_to_collector(
                        &cfg.endpoint,
                        &cfg.token,
                        &payload,
                    ).await {
                        eprintln!("[ChatContextCapture] Upload snapshot error: {e}");
                    } else {
                        println!("[ChatContextCapture] Successfully uploaded snapshot for {app_name} - {title}");
                    }
                }
            });
        } else {
            println!(
                "[ChatContextCapture] Settled snapshot captured for {} ({}), size {} KB (Collector not active)",
                app_name,
                title,
                jpeg_data.len() / 1024
            );
        }
    }
}

pub fn start_chat_context_monitor(
    manager: Arc<ClipboardManager>,
    state: Arc<ChatContextCaptureState>,
) {
    tauri::async_runtime::spawn(async move {
        #[cfg(target_os = "windows")]
        {
            let mut last_hwnd: Option<isize> = None;
            let mut last_hash = String::new();
            let mut last_sample: Vec<u8> = Vec::new();
            let mut dirty = false;
            let mut changed_at = Instant::now();
            let mut first_changed_at = Instant::now();
            let mut pending_jpeg: Option<Vec<u8>> = None;
            let mut pending_app = String::new();
            let mut pending_title = String::new();

            loop {
                tokio::time::sleep(MONITOR_POLL_INTERVAL).await;

                if !state.is_enabled() {
                    dirty = false;
                    pending_jpeg = None;
                    continue;
                }

                let metadata = match manager.get_active_window_info() {
                    Ok(meta) if !meta.is_self => meta,
                    _ => continue,
                };

                // Scenario 1: Focus switched away to another application (Blur Drain)
                if !platform_impl::is_chat_target(&metadata.source_app, &metadata.window_title) {
                    if let Some(old_hwnd) = last_hwnd {
                        // Attempt one last fresh capture of the chat window that just blurred
                        if let Some(fresh) = tokio::task::spawn_blocking(move || platform_impl::capture_window(old_hwnd)).await.ok().flatten() {
                            if fresh.hash != last_hash {
                                pending_jpeg = Some(fresh.jpeg_bytes);
                                dirty = true;
                            }
                        }
                    }
                    if dirty {
                        if let Some(jpeg_data) = pending_jpeg.take() {
                            let app = std::mem::take(&mut pending_app);
                            let title = std::mem::take(&mut pending_title);
                            dispatch_chat_snapshot(manager.clone(), state.clone(), app, title, jpeg_data).await;
                        }
                        dirty = false;
                    }
                    last_hwnd = None;
                    last_hash.clear();
                    last_sample.clear();
                    continue;
                }

                let hwnd_val = match platform_impl::get_foreground_hwnd() {
                    Some(val) => val,
                    None => continue,
                };

                // Scenario 2: Switched to another chat window (different HWND Window Switch Drain)
                if last_hwnd != Some(hwnd_val) {
                    if let Some(old_hwnd) = last_hwnd {
                        // Attempt one last fresh capture of the previous chat window
                        if let Some(fresh) = tokio::task::spawn_blocking(move || platform_impl::capture_window(old_hwnd)).await.ok().flatten() {
                            if fresh.hash != last_hash {
                                pending_jpeg = Some(fresh.jpeg_bytes);
                                dirty = true;
                            }
                        }
                    }
                    if dirty {
                        if let Some(jpeg_data) = pending_jpeg.take() {
                            let app = std::mem::take(&mut pending_app);
                            let title = std::mem::take(&mut pending_title);
                            dispatch_chat_snapshot(manager.clone(), state.clone(), app, title, jpeg_data).await;
                        }
                        dirty = false;
                    }
                    last_hwnd = Some(hwnd_val);
                    last_hash.clear();
                    last_sample.clear();
                }

                // Capture window in blocking task
                let capture = tokio::task::spawn_blocking(move || platform_impl::capture_window(hwnd_val))
                    .await
                    .ok()
                    .flatten();

                let Some(capture) = capture else {
                    continue;
                };

                let now = Instant::now();
                if capture.hash != last_hash {
                    // Scenario 3: Switched conversation within the same window (Large scene diff drain)
                    let is_conversation_switch = if !last_sample.is_empty() && last_sample.len() == capture.sample.len() {
                        let diff_count = last_sample.iter().zip(capture.sample.iter())
                            .filter(|(&a, &b)| (a as i16 - b as i16).abs() > 20)
                            .count();
                        // If > 20% of 32x32 sample points changed substantially, it is a conversation or tab switch
                        diff_count > (capture.sample.len() * 20 / 100)
                    } else {
                        false
                    };

                    if is_conversation_switch && dirty {
                        // Drain previous conversation before tracking the new one
                        if let Some(jpeg_data) = pending_jpeg.take() {
                            let app = std::mem::take(&mut pending_app);
                            let title = std::mem::take(&mut pending_title);
                            dispatch_chat_snapshot(manager.clone(), state.clone(), app, title, jpeg_data).await;
                        }
                    }

                    if !dirty || is_conversation_switch {
                        first_changed_at = now;
                    }
                    last_hash = capture.hash;
                    last_sample = capture.sample;
                    changed_at = now;
                    dirty = true;
                    pending_jpeg = Some(capture.jpeg_bytes);
                    pending_app = metadata.source_app.clone();
                    pending_title = metadata.window_title.clone();
                } else if dirty {
                    let settled = now.duration_since(changed_at) >= SETTLE_DELAY;
                    let max_reached = now.duration_since(first_changed_at) >= MAX_SETTLE_DELAY;

                    if settled || max_reached {
                        dirty = false;
                        if let Some(jpeg_data) = pending_jpeg.take() {
                            let app = std::mem::take(&mut pending_app);
                            let title = std::mem::take(&mut pending_title);
                            dispatch_chat_snapshot(manager.clone(), state.clone(), app, title, jpeg_data).await;
                        }
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        }
    });
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatContextProbeResult {
    pub is_chat_target: bool,
    pub source_app: String,
    pub window_title: String,
    pub window_width: i32,
    pub window_height: i32,
    pub captured: bool,
    pub image_size_bytes: usize,
    pub image_preview_base64: Option<String>,
    pub engine: String,
    pub collector_ready: bool,
    pub collector_endpoint: Option<String>,
    pub uploaded: bool,
    pub message_count: usize,
    pub messages_preview: Vec<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn probe_and_send_chat_context(
    manager: tauri::State<'_, Arc<ClipboardManager>>,
    state: tauri::State<'_, Arc<ChatContextCaptureState>>,
    delay_ms: Option<u64>,
) -> Result<ChatContextProbeResult, String> {
    let delay = delay_ms.unwrap_or(2_500).clamp(500, 10_000);
    tokio::time::sleep(Duration::from_millis(delay)).await;

    #[cfg(not(target_os = "windows"))]
    {
        let _ = manager;
        Ok(ChatContextProbeResult {
            is_chat_target: false,
            source_app: "Unsupported".to_string(),
            window_title: "".to_string(),
            window_width: 0,
            window_height: 0,
            captured: false,
            image_size_bytes: 0,
            image_preview_base64: None,
            engine: state.mode(),
            collector_ready: false,
            collector_endpoint: None,
            uploaded: false,
            message_count: 0,
            messages_preview: Vec::new(),
            error: Some("Chat context capture is currently supported on Windows only".to_string()),
        })
    }

    #[cfg(target_os = "windows")]
    {
        let metadata = match manager.get_active_window_info() {
            Ok(meta) => meta,
            Err(e) => {
                return Ok(ChatContextProbeResult {
                    is_chat_target: false,
                    source_app: "Unknown".to_string(),
                    window_title: "".to_string(),
                    window_width: 0,
                    window_height: 0,
                    captured: false,
                    image_size_bytes: 0,
                    image_preview_base64: None,
                    engine: state.mode(),
                    collector_ready: false,
                    collector_endpoint: None,
                    uploaded: false,
                    message_count: 0,
                    messages_preview: Vec::new(),
                    error: Some(format!("Failed to get active window: {e}")),
                });
            }
        };

        let is_target = platform_impl::is_chat_target(&metadata.source_app, &metadata.window_title);
        let hwnd_val = match platform_impl::get_foreground_hwnd() {
            Some(v) => v,
            None => {
                return Ok(ChatContextProbeResult {
                    is_chat_target: is_target,
                    source_app: metadata.source_app,
                    window_title: metadata.window_title,
                    window_width: 0,
                    window_height: 0,
                    captured: false,
                    image_size_bytes: 0,
                    image_preview_base64: None,
                    engine: state.mode(),
                    collector_ready: false,
                    collector_endpoint: None,
                    uploaded: false,
                    message_count: 0,
                    messages_preview: Vec::new(),
                    error: Some("GetForegroundWindow returned invalid HWND".to_string()),
                });
            }
        };

        let capture = tokio::task::spawn_blocking(move || platform_impl::capture_window(hwnd_val))
            .await
            .ok()
            .flatten();

        let Some(capture) = capture else {
            return Ok(ChatContextProbeResult {
                is_chat_target: is_target,
                source_app: metadata.source_app,
                window_title: metadata.window_title,
                window_width: 0,
                window_height: 0,
                captured: false,
                image_size_bytes: 0,
                image_preview_base64: None,
                engine: state.mode(),
                collector_ready: false,
                collector_endpoint: None,
                uploaded: false,
                message_count: 0,
                messages_preview: Vec::new(),
                error: Some("capture_window failed (window may be minimized, size <= 100x100, or PrintWindow failed)".to_string()),
            });
        };

        use base64::Engine;
        let preview_b64 = base64::engine::general_purpose::STANDARD.encode(&capture.jpeg_bytes);
        let image_size = capture.jpeg_bytes.len();
        let engine_mode = state.mode();

        let collector_cfg = manager.config_repo().load_collector_config().await.ok().flatten();
        let (collector_ready, endpoint, token) = match collector_cfg {
            Some(cfg) if cfg.enabled && !cfg.endpoint.is_empty() && !cfg.token.is_empty() => {
                (true, Some(cfg.endpoint), Some(cfg.token))
            }
            Some(cfg) => (false, Some(cfg.endpoint), None),
            None => (false, None, None),
        };

        if !collector_ready {
            return Ok(ChatContextProbeResult {
                is_chat_target: is_target,
                source_app: metadata.source_app,
                window_title: metadata.window_title,
                window_width: capture.width,
                window_height: capture.height,
                captured: true,
                image_size_bytes: image_size,
                image_preview_base64: Some(preview_b64),
                engine: engine_mode,
                collector_ready: false,
                collector_endpoint: endpoint,
                uploaded: false,
                message_count: 0,
                messages_preview: Vec::new(),
                error: Some("Collector not configured or enabled (configure URL and token below)".to_string()),
            });
        }

        let endpoint_str = endpoint.unwrap();
        let token_str = token.unwrap();

        if engine_mode == "local_ocr" {
            let ocr = NativeOcr::new();
            match ocr.recognize_bytes(&capture.jpeg_bytes).await {
                Ok(res) => {
                    let mut messages = Vec::new();
                    let mut previews = Vec::new();
                    for line in res.text.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            messages.push(StoredDialogueMessage {
                                speaker: "chat".to_string(),
                                role: "user".to_string(),
                                content: trimmed.to_string(),
                            });
                            if previews.len() < 5 {
                                previews.push(trimmed.to_string());
                            }
                        }
                    }
                    let count = messages.len();
                    if messages.is_empty() {
                        return Ok(ChatContextProbeResult {
                            is_chat_target: is_target,
                            source_app: metadata.source_app,
                            window_title: metadata.window_title,
                            window_width: capture.width,
                            window_height: capture.height,
                            captured: true,
                            image_size_bytes: image_size,
                            image_preview_base64: Some(preview_b64),
                            engine: engine_mode,
                            collector_ready: true,
                            collector_endpoint: Some(endpoint_str),
                            uploaded: false,
                            message_count: 0,
                            messages_preview: Vec::new(),
                            error: Some("Local OCR extracted 0 text lines from the image".to_string()),
                        });
                    }

                    let payload = ContextSnapshotPayload {
                        captured_at: chrono::Utc::now().to_rfc3339(),
                        source_app: &metadata.source_app,
                        window_title: &metadata.window_title,
                        engine: Some("local_ocr"),
                        image_format: None,
                        image_base64: None,
                        messages: Some(messages),
                    };

                    match send_context_snapshot_to_collector(&endpoint_str, &token_str, &payload).await {
                        Ok(()) => Ok(ChatContextProbeResult {
                            is_chat_target: is_target,
                            source_app: metadata.source_app,
                            window_title: metadata.window_title,
                            window_width: capture.width,
                            window_height: capture.height,
                            captured: true,
                            image_size_bytes: image_size,
                            image_preview_base64: Some(preview_b64),
                            engine: engine_mode,
                            collector_ready: true,
                            collector_endpoint: Some(endpoint_str),
                            uploaded: true,
                            message_count: count,
                            messages_preview: previews,
                            error: None,
                        }),
                        Err(e) => Ok(ChatContextProbeResult {
                            is_chat_target: is_target,
                            source_app: metadata.source_app,
                            window_title: metadata.window_title,
                            window_width: capture.width,
                            window_height: capture.height,
                            captured: true,
                            image_size_bytes: image_size,
                            image_preview_base64: Some(preview_b64),
                            engine: engine_mode,
                            collector_ready: true,
                            collector_endpoint: Some(endpoint_str),
                            uploaded: false,
                            message_count: count,
                            messages_preview: previews,
                            error: Some(format!("Upload to Collector failed: {e}")),
                        }),
                    }
                }
                Err(e) => Ok(ChatContextProbeResult {
                    is_chat_target: is_target,
                    source_app: metadata.source_app,
                    window_title: metadata.window_title,
                    window_width: capture.width,
                    window_height: capture.height,
                    captured: true,
                    image_size_bytes: image_size,
                    image_preview_base64: Some(preview_b64),
                    engine: engine_mode,
                    collector_ready: true,
                    collector_endpoint: Some(endpoint_str),
                    uploaded: false,
                    message_count: 0,
                    messages_preview: Vec::new(),
                    error: Some(format!("Local OCR engine failed: {e}")),
                }),
            }
        } else {
            let payload = ContextSnapshotPayload {
                captured_at: chrono::Utc::now().to_rfc3339(),
                source_app: &metadata.source_app,
                window_title: &metadata.window_title,
                engine: Some("cloud_llm"),
                image_format: Some("jpeg"),
                image_base64: Some(preview_b64.clone()),
                messages: None,
            };

            match send_context_snapshot_to_collector(&endpoint_str, &token_str, &payload).await {
                Ok(()) => Ok(ChatContextProbeResult {
                    is_chat_target: is_target,
                    source_app: metadata.source_app,
                    window_title: metadata.window_title,
                    window_width: capture.width,
                    window_height: capture.height,
                    captured: true,
                    image_size_bytes: image_size,
                    image_preview_base64: Some(preview_b64),
                    engine: engine_mode,
                    collector_ready: true,
                    collector_endpoint: Some(endpoint_str),
                    uploaded: true,
                    message_count: 0,
                    messages_preview: Vec::new(),
                    error: None,
                }),
                Err(e) => Ok(ChatContextProbeResult {
                    is_chat_target: is_target,
                    source_app: metadata.source_app,
                    window_title: metadata.window_title,
                    window_width: capture.width,
                    window_height: capture.height,
                    captured: true,
                    image_size_bytes: image_size,
                    image_preview_base64: Some(preview_b64),
                    engine: engine_mode,
                    collector_ready: true,
                    collector_endpoint: Some(endpoint_str),
                    uploaded: false,
                    message_count: 0,
                    messages_preview: Vec::new(),
                    error: Some(format!("Upload snapshot to Collector failed: {e}")),
                }),
            }
        }
    }
}
