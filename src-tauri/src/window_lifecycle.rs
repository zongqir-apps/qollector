use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, WebviewWindow, WebviewWindowBuilder};

pub const MAIN_WINDOW_LABEL: &str = "main";

#[derive(Default)]
pub struct AppExitState {
    user_requested_exit: AtomicBool,
}

impl AppExitState {
    pub fn mark_user_requested_exit(&self) {
        self.user_requested_exit.store(true, Ordering::SeqCst);
    }

    pub fn is_user_requested_exit(&self) -> bool {
        self.user_requested_exit.load(Ordering::SeqCst)
    }
}

fn find_main_window_config(app: &AppHandle) -> Option<tauri::utils::config::WindowConfig> {
    app.config()
        .app
        .windows
        .iter()
        .find(|cfg| cfg.label == MAIN_WINDOW_LABEL)
        .cloned()
}

pub fn ensure_main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        return Ok(window);
    }

    let config = find_main_window_config(app)
        .ok_or_else(|| format!("main window config '{}' not found", MAIN_WINDOW_LABEL))?;

    WebviewWindowBuilder::from_config(app, &config)
        .map_err(|e| format!("failed to create main window builder: {}", e))?
        .build()
        .map_err(|e| format!("failed to build main window: {}", e))
}

pub fn hide_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.hide();
    }
}

static LAST_EXTERNAL_ACTIVE_HWND: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[cfg(target_os = "windows")]
pub fn is_ignored_shell_class(name: &str) -> bool {
    matches!(
        name,
        "Progman"
            | "WorkerW"
            | "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "NotifyIconOverflowWindow"
            | "TopLevelWindowForOverflowXamlIsland"
            | "TaskListThumbnailWnd"
    )
}

#[cfg(target_os = "windows")]
fn is_valid_external_target(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetWindowThreadProcessId, IsWindow, IsWindowVisible,
    };

    unsafe {
        if hwnd.is_invalid() || !IsWindow(hwnd).as_bool() || !IsWindowVisible(hwnd).as_bool() {
            return false;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 || pid == std::process::id() {
            return false;
        }
        let mut class_name = [0u16; 64];
        let len = GetClassNameW(hwnd, &mut class_name);
        if len > 0 {
            let name = String::from_utf16_lossy(&class_name[..len as usize]);
            if is_ignored_shell_class(&name) {
                return false;
            }
        }
        true
    }
}

#[cfg(target_os = "windows")]
fn record_external_hwnd_if_valid(hwnd: windows::Win32::Foundation::HWND) {
    if is_valid_external_target(hwnd) {
        LAST_EXTERNAL_ACTIVE_HWND.store(hwnd.0 as usize, Ordering::SeqCst);
    }
}

pub fn record_external_active_window() {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        let hwnd = GetForegroundWindow();
        record_external_hwnd_if_valid(hwnd);
    }
}

pub fn get_last_external_active_window() -> usize {
    LAST_EXTERNAL_ACTIVE_HWND.load(Ordering::SeqCst)
}

/// Finds a fallback visible external top-level window below `start_hwnd` in Windows Z-order.
/// Used when no prior foreground event was captured or the previously recorded window was closed.
pub fn find_fallback_external_window(start_hwnd_val: usize) -> usize {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindow, GW_HWNDNEXT,
        };

        let start = if start_hwnd_val != 0 {
            HWND(start_hwnd_val as *mut _)
        } else {
            GetForegroundWindow()
        };
        if start.is_invalid() {
            return 0;
        }

        let mut curr = start;
        for _ in 0..64 {
            match GetWindow(curr, GW_HWNDNEXT) {
                Ok(next) => {
                    if is_valid_external_target(next) {
                        return next.0 as usize;
                    }
                    curr = next;
                }
                Err(_) => break,
            }
        }
        0
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = start_hwnd_val;
        0
    }
}

/// Continuously tracks the most recently focused external window via Windows
/// `EVENT_SYSTEM_FOREGROUND` so double-click auto-paste works reliably regardless
/// of how Qollect was opened (shortcut, tray, taskbar, side-by-side, or always-on-top).
pub fn start_external_window_tracker() {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::{HMODULE, HWND};
        use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, TranslateMessage, EVENT_SYSTEM_FOREGROUND, MSG,
            WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
        };

        unsafe extern "system" fn foreground_event_proc(
            _hook: HWINEVENTHOOK,
            _event: u32,
            hwnd: HWND,
            _id_object: i32,
            _id_child: i32,
            _id_event_thread: u32,
            _dwms_event_time: u32,
        ) {
            record_external_hwnd_if_valid(hwnd);
        }

        record_external_active_window();

        std::thread::spawn(|| unsafe {
            let hook = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                HMODULE::default(),
                Some(foreground_event_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
            if hook.is_invalid() {
                eprintln!("[WindowLifecycle] Failed to install foreground window tracker");
                return;
            }

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = UnhookWinEvent(hook);
        });
    }
}
