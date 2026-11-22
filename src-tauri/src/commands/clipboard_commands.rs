use crate::core::clipboard_manager::ClipboardManager;
use crate::core::models::{ClipboardItem, SearchFilters, Workbench};
#[cfg(target_os = "windows")]
use crate::window_lifecycle::{hide_main_window, MAIN_WINDOW_LABEL};
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
#[cfg(target_os = "windows")]
use tauri::Manager;
use tauri::State;

#[tauri::command]
pub async fn get_recent_items(
    manager: State<'_, Arc<ClipboardManager>>,
    limit: i32,
) -> Result<Vec<ClipboardItem>, String> {
    manager.get_recent(limit).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_recent_items_light(
    manager: State<'_, Arc<ClipboardManager>>,
    limit: i32,
) -> Result<Vec<ClipboardItem>, String> {
    manager
        .get_recent_light(limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_more_items(
    manager: State<'_, Arc<ClipboardManager>>,
    limit: i32,
    offset: i32,
) -> Result<Vec<ClipboardItem>, String> {
    manager
        .get_recent_paged(limit, offset)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_workbench_pinned_items(
    manager: State<'_, Arc<ClipboardManager>>,
    workbench_id: Option<String>,
) -> Result<Vec<ClipboardItem>, String> {
    manager
        .get_workbench_pinned_items(workbench_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reorder_workbench_pinned_items(
    manager: State<'_, Arc<ClipboardManager>>,
    workbench_id: Option<String>,
    ordered_ids: Vec<String>,
) -> Result<(), String> {
    manager
        .reorder_workbench_pinned_items(workbench_id.as_deref(), &ordered_ids)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_workbenches(
    manager: State<'_, Arc<ClipboardManager>>,
) -> Result<Vec<Workbench>, String> {
    manager.get_workbenches().await
}

#[tauri::command]
pub async fn create_workbench(
    manager: State<'_, Arc<ClipboardManager>>,
    name: String,
) -> Result<Workbench, String> {
    manager.create_workbench(&name).await
}

#[tauri::command]
pub async fn rename_workbench(
    manager: State<'_, Arc<ClipboardManager>>,
    id: String,
    name: String,
) -> Result<(), String> {
    manager.rename_workbench(&id, &name).await
}

#[tauri::command]
pub async fn delete_workbench(
    manager: State<'_, Arc<ClipboardManager>>,
    id: String,
) -> Result<String, String> {
    manager.delete_workbench(&id).await
}

#[tauri::command]
pub async fn get_collection_favorite_items(
    manager: State<'_, Arc<ClipboardManager>>,
) -> Result<Vec<ClipboardItem>, String> {
    manager
        .get_collection_favorite_items()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reorder_collection_favorite_items(
    manager: State<'_, Arc<ClipboardManager>>,
    ordered_ids: Vec<String>,
) -> Result<(), String> {
    manager
        .reorder_collection_favorite_items(&ordered_ids)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_item_by_id(
    manager: State<'_, Arc<ClipboardManager>>,
    id: String,
) -> Result<Option<ClipboardItem>, String> {
    manager.get_item_by_id(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn copy_item(
    manager: State<'_, Arc<ClipboardManager>>,
    item: ClipboardItem,
    skip_touch: Option<bool>,
) -> Result<(), String> {
    manager
        .copy_to_clipboard_ext(&item, skip_touch.unwrap_or(false))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn copy_plain_text(
    manager: State<'_, Arc<ClipboardManager>>,
    text: String,
) -> Result<(), String> {
    manager
        .copy_plain_text(&text)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_item(
    manager: State<'_, Arc<ClipboardManager>>,
    item: ClipboardItem,
) -> Result<(), String> {
    manager.update_item(&item).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_item(
    manager: State<'_, Arc<ClipboardManager>>,
    id: String,
) -> Result<(), String> {
    manager.delete_item(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_pin_item(
    manager: State<'_, Arc<ClipboardManager>>,
    id: String,
    max_pinned: Option<usize>,
    workbench_id: Option<String>,
) -> Result<bool, String> {
    manager
        .toggle_pin(&id, max_pinned.unwrap_or(10), workbench_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pin_item(
    manager: State<'_, Arc<ClipboardManager>>,
    id: String,
    max_pinned: Option<usize>,
    workbench_id: Option<String>,
) -> Result<bool, String> {
    manager
        .pin_item(&id, max_pinned.unwrap_or(10), workbench_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_favorite_item(
    manager: State<'_, Arc<ClipboardManager>>,
    id: String,
) -> Result<bool, String> {
    manager
        .toggle_favorite(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_items(
    manager: State<'_, Arc<ClipboardManager>>,
    query: String,
    limit: Option<i32>,
    filters: Option<SearchFilters>,
) -> Result<Vec<ClipboardItem>, String> {
    manager
        .search_items_with_filters(
            &query,
            filters.unwrap_or_default(),
            limit.unwrap_or(200) as usize,
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_user_tags(
    manager: State<'_, Arc<ClipboardManager>>,
    query: Option<String>,
    limit: Option<i32>,
    only_favorite: Option<bool>,
    only_pinned: Option<bool>,
) -> Result<Vec<String>, String> {
    manager
        .get_user_tags(
            query.as_deref(),
            limit.unwrap_or(80),
            only_favorite.unwrap_or(false),
            only_pinned.unwrap_or(false),
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_item(
    manager: State<'_, Arc<ClipboardManager>>,
    id: String,
) -> Result<(), String> {
    let item = manager
        .get_item_by_id(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Item not found".to_string())?;

    let target = match item.type_.as_str() {
        "link" => parse_openable_link(
            item.content
                .as_deref()
                .ok_or_else(|| "Link content is empty".to_string())?,
        )?,
        "file" => first_file_target(
            item.content
                .as_deref()
                .ok_or_else(|| "File path is empty".to_string())?,
        )?,
        _ => return Err("Only link and file types can be opened".to_string()),
    };

    open_with_system(&target)?;

    Ok(())
}

#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    let target = parse_openable_link(&url)?;
    open_with_system(&target)?;
    Ok(())
}

#[tauri::command]
pub async fn try_paste_foreground(app: tauri::AppHandle) -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
        use windows::Win32::UI::WindowsAndMessaging::{
            AllowSetForegroundWindow, BringWindowToTop, GetClassNameW, GetForegroundWindow,
            GetWindowThreadProcessId, IsIconic, IsWindow, SetForegroundWindow, ShowWindow,
            SW_RESTORE, SW_SHOWNOACTIVATE,
        };

        // 1. Capture our own foreground HWND while Qollect is still active (before hiding),
        //    so we hold legal foreground transfer rights and can restore pinned windows without
        //    stealing focus back later.
        let self_fg_hwnd_val: usize = unsafe {
            let fg = GetForegroundWindow();
            if !fg.is_invalid() {
                let mut pid = 0;
                GetWindowThreadProcessId(fg, Some(&mut pid));
                if pid == std::process::id() {
                    fg.0 as usize
                } else {
                    0
                }
            } else {
                0
            }
        };

        // 2. Resolve target external window: prefer continuously tracked last external HWND,
        //    falling back to the nearest visible external top-level window in Z-order.
        let mut target_hwnd_val = crate::window_lifecycle::get_last_external_active_window();
        let recorded_valid = if target_hwnd_val != 0 {
            unsafe { IsWindow(HWND(target_hwnd_val as *mut _)).as_bool() }
        } else {
            false
        };
        if !recorded_valid {
            target_hwnd_val =
                crate::window_lifecycle::find_fallback_external_window(self_fg_hwnd_val);
        }

        if target_hwnd_val == 0 {
            println!("[AutoPaste] No valid external target window found; copied only");
            return Ok(false);
        }

        let mut pinned_always_on_top = false;
        if let Some(main_window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
            pinned_always_on_top = main_window.is_always_on_top().unwrap_or(false);
        }

        // 3. Transfer foreground authority BEFORE hiding Qollect while our process still owns
        //    Windows foreground rights. Keep `HWND` strictly scoped to non-await blocks so the
        //    async Future remains `Send`.
        unsafe {
            let target_hwnd = HWND(target_hwnd_val as *mut _);
            let mut target_pid = 0;
            GetWindowThreadProcessId(target_hwnd, Some(&mut target_pid));
            if target_pid != 0 {
                let _ = AllowSetForegroundWindow(target_pid);
            }
            if IsIconic(target_hwnd).as_bool() {
                let _ = ShowWindow(target_hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(target_hwnd);
        }

        // 4. Hide Qollect (temporarily if pinned) so it cannot intercept focus or obscure caret.
        let mut should_restore_pinned_window = false;
        if pinned_always_on_top {
            if let Some(main_window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                let _ = main_window.hide();
                should_restore_pinned_window = true;
            }
        } else {
            hide_main_window(&app);
        }

        // 5. Reinforce activation if Windows did not switch foreground immediately.
        //    Attach the ACTUAL current foreground thread + our thread + target thread briefly,
        //    then ALWAYS detach before sending any keyboard input.
        unsafe {
            let target_hwnd = HWND(target_hwnd_val as *mut _);
            if GetForegroundWindow() != target_hwnd && IsWindow(target_hwnd).as_bool() {
                let our_tid = GetCurrentThreadId();
                let current_fg = GetForegroundWindow();
                let fg_tid = if !current_fg.is_invalid() {
                    GetWindowThreadProcessId(current_fg, None)
                } else {
                    0
                };
                let target_tid = GetWindowThreadProcessId(target_hwnd, None);

                let attached_fg = if fg_tid != 0 && fg_tid != our_tid {
                    AttachThreadInput(our_tid, fg_tid, true).as_bool()
                } else {
                    false
                };
                let attached_target =
                    if target_tid != 0 && target_tid != our_tid && target_tid != fg_tid {
                        AttachThreadInput(our_tid, target_tid, true).as_bool()
                    } else {
                        false
                    };

                let _ = BringWindowToTop(target_hwnd);
                let _ = SetForegroundWindow(target_hwnd);

                if attached_target {
                    let _ = AttachThreadInput(our_tid, target_tid, false);
                }
                if attached_fg {
                    let _ = AttachThreadInput(our_tid, fg_tid, false);
                }
            }
        }

        // 6. Wait for foreground transition to settle and verify focus actually left Qollect.
        let mut focus_ready = false;
        for _ in 0..16 {
            tokio::time::sleep(std::time::Duration::from_millis(15)).await;
            let reached_target = unsafe {
                let current_fg = GetForegroundWindow();
                !current_fg.is_invalid() && (current_fg.0 as usize) == target_hwnd_val
            };
            if reached_target {
                focus_ready = true;
                break;
            }
        }

        let foreground_is_external = unsafe {
            let current_fg = GetForegroundWindow();
            if current_fg.is_invalid() {
                false
            } else {
                let mut fg_pid = 0;
                GetWindowThreadProcessId(current_fg, Some(&mut fg_pid));
                if fg_pid == 0 || fg_pid == std::process::id() {
                    false
                } else {
                    let mut class_name = [0u16; 64];
                    let len = GetClassNameW(current_fg, &mut class_name);
                    if len > 0 {
                        let name = String::from_utf16_lossy(&class_name[..len as usize]);
                        !crate::window_lifecycle::is_ignored_shell_class(&name)
                    } else {
                        true
                    }
                }
            }
        };

        if !foreground_is_external {
            println!("[AutoPaste] Foreground did not switch to an external window; aborting paste");
            if should_restore_pinned_window {
                if self_fg_hwnd_val != 0 {
                    unsafe {
                        let _ = ShowWindow(HWND(self_fg_hwnd_val as *mut _), SW_SHOWNOACTIVATE);
                    }
                } else if let Some(main_window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                    let _ = main_window.show();
                    let _ = main_window.set_always_on_top(true);
                }
            }
            return Ok(false);
        }

        // Give target app event loop time to process WM_ACTIVATE and restore text caret.
        let settle_ms = if focus_ready { 45 } else { 60 };
        tokio::time::sleep(std::time::Duration::from_millis(settle_ms)).await;

        send_ctrl_v_input().map_err(|e| format!("Failed to simulate paste: {}", e))?;

        if should_restore_pinned_window {
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            if self_fg_hwnd_val != 0 {
                unsafe {
                    let _ = ShowWindow(HWND(self_fg_hwnd_val as *mut _), SW_SHOWNOACTIVATE);
                }
            } else if let Some(main_window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                let _ = main_window.show();
                let _ = main_window.set_always_on_top(true);
            }
        }
        return Ok(true);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        // Keep behavior extensible: other platforms currently fallback to copy-only.
        Ok(false)
    }
}

#[tauri::command]
pub async fn save_image_as(
    manager: State<'_, Arc<ClipboardManager>>,
    id: String,
) -> Result<String, String> {
    let item = manager
        .get_item_by_id(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Item not found".to_string())?;

    if item.type_ != "image" {
        return Err("Only image types can be saved".to_string());
    }

    let source_path = image_source_path(&item)?;
    let source = Path::new(&source_path);
    if !source.exists() {
        return Err("Image file not found".to_string());
    }

    let suggested_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .map(ToString::to_string)
        .unwrap_or_else(|| "image.png".to_string());

    let extension = source
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("png");

    let save_path = rfd::FileDialog::new()
        .set_file_name(&suggested_name)
        .add_filter("Image", &[extension])
        .save_file()
        .ok_or_else(|| "CANCELLED".to_string())?;

    std::fs::copy(source, &save_path).map_err(|e| format!("Failed to save image: {}", e))?;

    Ok(save_path.to_string_lossy().to_string())
}

fn first_file_target(content: &str) -> Result<String, String> {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| "File path is empty".to_string())
}

fn parse_openable_link(content: &str) -> Result<String, String> {
    let trimmed = content.trim();
    let parsed = url::Url::parse(trimmed).map_err(|_| "Invalid link URL".to_string())?;
    let scheme = parsed.scheme();
    let supported = matches!(scheme, "http" | "https" | "ftp" | "ftps" | "ws" | "wss");
    if !supported {
        return Err("Unsupported link scheme".to_string());
    }
    Ok(trimmed.to_string())
}

fn image_source_path(item: &ClipboardItem) -> Result<String, String> {
    let image_paths = item
        .image_paths
        .as_deref()
        .ok_or_else(|| "Image path is empty".to_string())?;

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(image_paths) {
        if let Some(path) = value.get("original").and_then(|v| v.as_str()) {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }

        if let Some(path) = value.get("raw").and_then(|v| v.as_str()) {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
    }

    let trimmed = image_paths.trim();
    if trimmed.is_empty() {
        return Err("Image path is empty".to_string());
    }

    Ok(trimmed.to_string())
}

#[cfg(target_os = "windows")]
fn send_ctrl_v_input() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::time::Duration;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
        KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, VIRTUAL_KEY, VK_LBUTTON, VK_LCONTROL, VK_LMENU,
        VK_LSHIFT, VK_LWIN, VK_MENU, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
    };

    const VK_V_KEY: u16 = 0x56;
    const FALLBACK_SCAN_LCTRL: u16 = 0x1D;
    const FALLBACK_SCAN_V: u16 = 0x2F;
    const QOLLECT_PASTE_EXTRA_INFO: usize = 0x514F_4C56; // "QOLV"

    fn send_key_event(
        vk: u16,
        scan: u16,
        key_up: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let flags = if key_up {
            KEYEVENTF_KEYUP
        } else {
            Default::default()
        };

        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk),
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: QOLLECT_PASTE_EXTRA_INFO,
                },
            },
        };

        let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
        if sent != 1 {
            return Err(format!("SendInput failed for vk {}, sent {}", vk, sent).into());
        }
        Ok(())
    }

    struct LeftCtrlGuard {
        scan: u16,
        active: bool,
    }

    impl Drop for LeftCtrlGuard {
        fn drop(&mut self) {
            if self.active {
                let _ = send_key_event(VK_LCONTROL.0, self.scan, true);
                self.active = false;
            }
        }
    }

    // Wait until the physical left mouse button from double-clicking is fully released
    // (apps ignore Ctrl+V while mouse-down state is still active).
    unsafe {
        for _ in 0..16 {
            if (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) == 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        // Release any lingering physical non-Ctrl modifiers to avoid distorting Ctrl+V.
        for modifier in [
            VK_MENU, VK_LMENU, VK_RMENU, VK_SHIFT, VK_LSHIFT, VK_RSHIFT, VK_LWIN, VK_RWIN,
        ] {
            if (GetAsyncKeyState(modifier.0 as i32) as u16 & 0x8000) != 0 {
                let _ = send_key_event(modifier.0, 0, true);
            }
        }
    }

    let scan_ctrl = unsafe {
        let mapped = MapVirtualKeyW(VK_LCONTROL.0 as u32, MAPVK_VK_TO_VSC) as u16;
        if mapped != 0 {
            mapped
        } else {
            FALLBACK_SCAN_LCTRL
        }
    };
    let scan_v = unsafe {
        let mapped = MapVirtualKeyW(VK_V_KEY as u32, MAPVK_VK_TO_VSC) as u16;
        if mapped != 0 {
            mapped
        } else {
            FALLBACK_SCAN_V
        }
    };

    // 1. Press concrete Left Ctrl down and allow target window & IME to register modifier state.
    send_key_event(VK_LCONTROL.0, scan_ctrl, false)?;
    let ctrl_guard = LeftCtrlGuard {
        scan: scan_ctrl,
        active: true,
    };
    std::thread::sleep(Duration::from_millis(20));

    // 2. Press V down -> brief hold -> release V up.
    send_key_event(VK_V_KEY, scan_v, false)?;
    std::thread::sleep(Duration::from_millis(15));
    send_key_event(VK_V_KEY, scan_v, true)?;

    // 3. Trailing hold before releasing Left Ctrl so async apps/IME never read Ctrl=up on V.
    std::thread::sleep(Duration::from_millis(30));
    drop(ctrl_guard);

    Ok(())
}

#[tauri::command]
pub async fn reveal_in_folder(
    manager: State<'_, Arc<ClipboardManager>>,
    id: String,
) -> Result<(), String> {
    let item = manager
        .get_item_by_id(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Item not found".to_string())?;

    let target_path = match item.type_.as_str() {
        "file" => first_file_target(
            item.content
                .as_deref()
                .ok_or_else(|| "File path is empty".to_string())?,
        )?,
        "image" => image_source_path(&item)?,
        _ => return Err("Only file and image types can be revealed".to_string()),
    };

    let path = Path::new(&target_path);
    if !path.exists() {
        return Err("Target file does not exist".to_string());
    }

    reveal_path_in_explorer(path)?;
    Ok(())
}

fn reveal_path_in_explorer(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg("/select,")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to reveal in folder: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to reveal in folder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = path.parent() {
            std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| format!("Failed to reveal in folder: {}", e))?;
        } else {
            return Err("Cannot determine parent folder".to_string());
        }
    }

    Ok(())
}

#[derive(Serialize)]
pub struct DirectoryItem {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub size: u64,
    pub extension: String,
}

#[derive(Serialize)]
pub struct FilePreviewData {
    pub preview_type: String,
    pub file_path: String,
    pub file_name: String,
    pub file_size: u64,
    pub text_content: Option<String>,
    pub text_truncated: bool,
    pub is_directory: bool,
    pub directory_items: Option<Vec<DirectoryItem>>,
}

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "ico"];
const TEXT_EXTS: &[&str] = &[
    "txt", "md", "json", "csv", "log", "xml", "yaml", "yml", "toml", "ini", "conf", "js", "ts",
    "jsx", "tsx", "py", "rs", "go", "java", "c", "cpp", "h", "hpp", "sh", "bat", "ps1", "html",
    "css", "sql",
];
const AUDIO_EXTS: &[&str] = &["mp3", "wav", "ogg", "flac", "aac", "m4a", "wma"];
const VIDEO_EXTS: &[&str] = &["mp4", "webm", "mkv", "avi", "mov"];
const PDF_EXTS: &[&str] = &["pdf"];

fn classify_extension(ext: &str) -> &'static str {
    let lower = ext.to_ascii_lowercase();
    let lower = lower.as_str();
    if IMAGE_EXTS.contains(&lower) {
        "image"
    } else if TEXT_EXTS.contains(&lower) {
        "text"
    } else if AUDIO_EXTS.contains(&lower) {
        "audio"
    } else if VIDEO_EXTS.contains(&lower) {
        "video"
    } else if PDF_EXTS.contains(&lower) {
        "pdf"
    } else {
        "binary"
    }
}

const MAX_TEXT_READ: u64 = 5 * 1024 * 1024; // 5MB
const TEXT_PREVIEW_BYTES: usize = 10240; // 10KB

#[tauri::command]
pub async fn preview_file(file_path: String) -> Result<FilePreviewData, String> {
    let path = Path::new(&file_path);

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let metadata = std::fs::metadata(path).map_err(|e| format!("Cannot read file: {}", e))?;
    let file_size = metadata.len();
    let is_directory = metadata.is_dir();

    let (preview_type, text_content, text_truncated, directory_items) = if is_directory {
        let mut items = Vec::new();
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let path_str = entry.path().to_string_lossy().to_string();
                    let is_dir = meta.is_dir();
                    let size = meta.len();
                    let extension = entry
                        .path()
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_string();

                    items.push(DirectoryItem {
                        name,
                        path: path_str,
                        is_directory: is_dir,
                        size,
                        extension,
                    });
                }
            }
            items.sort_by(|a, b| match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            });
        }
        ("directory".to_string(), None, false, Some(items))
    } else {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let preview_type = classify_extension(ext).to_string();

        let (text_content, text_truncated) = if preview_type == "text" && file_size <= MAX_TEXT_READ
        {
            match std::fs::read(path) {
                Ok(bytes) => {
                    let truncated = bytes.len() > TEXT_PREVIEW_BYTES;
                    let slice = if truncated {
                        &bytes[..TEXT_PREVIEW_BYTES]
                    } else {
                        &bytes[..]
                    };
                    let text = String::from_utf8_lossy(slice).to_string();
                    (Some(text), truncated)
                }
                Err(_) => (None, false),
            }
        } else {
            (None, false)
        };
        (preview_type, text_content, text_truncated, None)
    };

    Ok(FilePreviewData {
        preview_type,
        file_path: file_path.clone(),
        file_name,
        file_size,
        text_content,
        text_truncated,
        is_directory,
        directory_items,
    })
}

fn open_with_system(target: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn open_file_path(path: String) -> Result<(), String> {
    open_with_system(&path)
}

#[tauri::command]
pub async fn reveal_file_path(path: String) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("File path is empty".to_string());
    }

    let path = Path::new(trimmed);
    if !path.exists() {
        return Err("File does not exist".to_string());
    }

    reveal_path_in_explorer(path)
}

#[tauri::command]
pub async fn get_items_by_date_range(
    manager: State<'_, Arc<ClipboardManager>>,
    start_timestamp: i64,
    end_timestamp: i64,
) -> Result<Vec<ClipboardItem>, String> {
    manager
        .get_items_by_date_range(start_timestamp, end_timestamp)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct HistoryGroupCounts {
    pub today: i64,
    pub yesterday: i64,
    pub last7: i64,
    pub older: i64,
}

#[tauri::command]
pub async fn get_history_group_counts(
    manager: State<'_, Arc<ClipboardManager>>,
    start_today: i64,
    start_yesterday: i64,
    start_last7: i64,
) -> Result<HistoryGroupCounts, String> {
    if !(start_last7 <= start_yesterday && start_yesterday <= start_today) {
        return Err("Invalid day boundaries".to_string());
    }

    let (today, yesterday, last7, older) = manager
        .get_history_group_counts(start_today, start_yesterday, start_last7)
        .await
        .map_err(|e| e.to_string())?;

    Ok(HistoryGroupCounts {
        today,
        yesterday,
        last7,
        older,
    })
}
