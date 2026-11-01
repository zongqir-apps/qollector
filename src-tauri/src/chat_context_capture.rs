use crate::core::clipboard_manager::ClipboardManager;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MONITOR_POLL_INTERVAL: Duration = Duration::from_millis(1000);
const SETTLE_DELAY: Duration = Duration::from_millis(1500);

pub struct ChatContextCaptureState {
    enabled: AtomicBool,
}

impl ChatContextCaptureState {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextSnapshotPayload<'a> {
    pub captured_at: String,
    pub source_app: &'a str,
    pub window_title: &'a str,
    pub image_format: &'a str,
    pub image_base64: String,
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
            let mut sample = Vec::with_capacity(32 * 32);
            for y in 0..32 {
                let row = (y * height / 32) as usize;
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
                jpeg_bytes: jpeg_buf.into_inner(),
            })
        }
    }
}

async fn send_snapshot_to_collector(
    endpoint: &str,
    token: &str,
    source_app: &str,
    window_title: &str,
    jpeg_bytes: Vec<u8>,
) -> Result<(), String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg_bytes);
    let payload = ContextSnapshotPayload {
        captured_at: chrono::Utc::now().to_rfc3339(),
        source_app,
        window_title,
        image_format: "jpeg",
        image_base64: b64,
    };

    let url = format!("{}/v1/context-snapshots", endpoint.trim_end_matches('/'));
    let response = crate::http_client::client()
        .post(url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Upload snapshot failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Collector returned HTTP {}", response.status()));
    }

    Ok(())
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
            let mut dirty = false;
            let mut changed_at = Instant::now();
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

                if !platform_impl::is_chat_target(&metadata.source_app, &metadata.window_title) {
                    dirty = false;
                    pending_jpeg = None;
                    continue;
                }

                let hwnd_val = match platform_impl::get_foreground_hwnd() {
                    Some(val) => val,
                    None => continue,
                };

                // If switched to another chat window, reset state
                if last_hwnd != Some(hwnd_val) {
                    last_hwnd = Some(hwnd_val);
                    last_hash.clear();
                    dirty = false;
                    pending_jpeg = None;
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
                    last_hash = capture.hash;
                    changed_at = now;
                    dirty = true;
                    pending_jpeg = Some(capture.jpeg_bytes);
                    pending_app = metadata.source_app.clone();
                    pending_title = metadata.window_title.clone();
                } else if dirty && now.duration_since(changed_at) >= SETTLE_DELAY {
                    // Settled after change! Trigger snapshot sync
                    dirty = false;
                    if let Some(jpeg_data) = pending_jpeg.take() {
                        let collector_cfg = manager.config_repo().load_collector_config().await.ok().flatten();
                        if let Some(cfg) = collector_cfg {
                            if cfg.enabled && !cfg.endpoint.is_empty() && !cfg.token.is_empty() {
                                let app_name = pending_app.clone();
                                let title = pending_title.clone();
                                tauri::async_runtime::spawn(async move {
                                    if let Err(e) = send_snapshot_to_collector(
                                        &cfg.endpoint,
                                        &cfg.token,
                                        &app_name,
                                        &title,
                                        jpeg_data,
                                    ).await {
                                        eprintln!("[ChatContextCapture] Upload snapshot error: {e}");
                                    } else {
                                        println!("[ChatContextCapture] Successfully uploaded snapshot for {app_name} - {title}");
                                    }
                                });
                            } else {
                                println!(
                                    "[ChatContextCapture] Settled snapshot captured for {} ({}), size {} KB (Collector not active)",
                                    pending_app,
                                    pending_title,
                                    jpeg_data.len() / 1024
                                );
                            }
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
