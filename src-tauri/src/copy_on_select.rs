use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct CopyOnSelectState {
    enabled: AtomicBool,
    double_click_enabled: AtomicBool,
    smart_replace_enabled: AtomicBool,
}

impl CopyOnSelectState {
    pub fn new(enabled: bool, double_click_enabled: bool, smart_replace_enabled: bool) -> Self {
        Self {
            enabled: AtomicBool::new(enabled),
            double_click_enabled: AtomicBool::new(double_click_enabled),
            smart_replace_enabled: AtomicBool::new(smart_replace_enabled),
        }
    }

    pub fn update(&self, enabled: bool, double_click_enabled: bool, smart_replace_enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
        self.double_click_enabled.store(double_click_enabled, Ordering::Relaxed);
        self.smart_replace_enabled.store(smart_replace_enabled, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn is_double_click_enabled(&self) -> bool {
        self.double_click_enabled.load(Ordering::Relaxed)
    }

    pub fn is_smart_replace_enabled(&self) -> bool {
        self.smart_replace_enabled.load(Ordering::Relaxed)
    }
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use parking_lot::Mutex;
    use std::sync::LazyLock;
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::{CloseHandle, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Threading::{
        AttachThreadInput, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW,
        PROCESS_NAME_WIN32, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, GetDoubleClickTime, GetFocus, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD,
        KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetClassNameW, GetForegroundWindow, GetMessageW,
        GetSystemMetrics, GetWindowThreadProcessId, SendMessageW, SetWindowsHookExW,
        TranslateMessage, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
        SM_CXDOUBLECLK, SM_CYDOUBLECLK, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_COPY, WM_KEYDOWN,
        WM_LBUTTONDOWN, WM_LBUTTONUP, WM_SYSKEYDOWN,
    };

    const EXCLUDED_PROCESS_NAMES: &[&str] = &[
        // Windows Console & Terminals (already have native copy-on-select; avoid SIGINT)
        "windowsterminal.exe",
        "cmd.exe",
        "powershell.exe",
        "pwsh.exe",
        "mintty.exe",
        "alacritty.exe",
        "wezterm-gui.exe",
        "conhost.exe",
        "putty.exe",
        "kitty.exe",
        "mobaxterm.exe",
        "xshell.exe",
        "securecrt.exe",
        "tabby.exe",
        "warp.exe",
        "hyper.exe",
        "terminus.exe",
        // Remote desktop & virtualization tools
        "mstsc.exe",
        "teamviewer.exe",
        "anydesk.exe",
        "vncviewer.exe",
    ];

    static COPY_ON_SELECT_STATE: LazyLock<Mutex<Option<Arc<CopyOnSelectState>>>> =
        LazyLock::new(|| Mutex::new(None));

    struct MouseClickState {
        last_down_pt: (i32, i32),
        last_down_time: Instant,
        last_up_pt: (i32, i32),
        last_up_time: Instant,
        last_trigger_time: Instant,
    }

    static CLICK_TRACKER: LazyLock<Mutex<MouseClickState>> = LazyLock::new(|| {
        let now = Instant::now();
        Mutex::new(MouseClickState {
            last_down_pt: (0, 0),
            last_down_time: now,
            last_up_pt: (0, 0),
            last_up_time: now,
            last_trigger_time: now,
        })
    });

    struct ReplaceTracker {
        armed: bool,
        target_hwnd: isize,
        preserved_text: Option<String>,
        trigger_time: Instant,
    }

    static REPLACE_TRACKER: LazyLock<Mutex<ReplaceTracker>> = LazyLock::new(|| {
        Mutex::new(ReplaceTracker {
            armed: false,
            target_hwnd: 0,
            preserved_text: None,
            trigger_time: Instant::now(),
        })
    });

    fn get_process_exe_name(process_id: u32) -> Option<String> {
        use windows::core::PWSTR;

        unsafe {
            let handle = OpenProcess(
                PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                false,
                process_id,
            )
            .ok()?;

            let mut name = vec![0u16; 260];
            let mut size = name.len() as u32;
            let pwstr = PWSTR(name.as_mut_ptr());

            let res = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, pwstr, &mut size);
            let _ = CloseHandle(handle);

            if res.is_ok() {
                let full_path = String::from_utf16_lossy(&name[..size as usize]);
                let exe_name = full_path
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or(&full_path)
                    .to_lowercase();
                Some(exe_name)
            } else {
                None
            }
        }
    }

    fn is_excluded_context() -> bool {
        unsafe {
            // 1. Check modifier keys (Ctrl, Alt, Win)
            let is_ctrl = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
            let is_alt = (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0;
            let is_win = ((GetAsyncKeyState(VK_LWIN.0 as i32) as u16 & 0x8000) != 0)
                || ((GetAsyncKeyState(VK_RWIN.0 as i32) as u16 & 0x8000) != 0);

            if is_ctrl || is_alt || is_win {
                return true;
            }

            // 2. Check foreground window
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                return true;
            }

            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 || pid == std::process::id() {
                return true;
            }

            // 3. Check excluded process blacklist (terminals, remote desktops)
            if let Some(exe_name) = get_process_exe_name(pid) {
                if EXCLUDED_PROCESS_NAMES.iter().any(|&item| item == exe_name) {
                    return true;
                }
            }

            false
        }
    }

    fn send_atomic_ctrl_c() {
        const VK_C_KEY: u16 = 0x43;
        const SCAN_CTRL: u16 = 0x1D;
        const SCAN_C: u16 = 0x2E;

        fn key_input(vk: u16, scan: u16, key_up: bool) -> INPUT {
            let flags = if key_up {
                KEYEVENTF_KEYUP
            } else {
                Default::default()
            };

            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(vk),
                        wScan: scan,
                        dwFlags: flags,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            }
        }

        // Submits all 4 events atomically in a single system call
        // No thread sleep between Ctrl-Down and C-Down, eliminating IME/queue modifier drop!
        let inputs = [
            key_input(VK_CONTROL.0, SCAN_CTRL, false),
            key_input(VK_C_KEY, SCAN_C, false),
            key_input(VK_C_KEY, SCAN_C, true),
            key_input(VK_CONTROL.0, SCAN_CTRL, true),
        ];

        unsafe {
            let _ = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }

    fn execute_copy(target_hwnd: HWND) {
        unsafe {
            let target_tid = GetWindowThreadProcessId(target_hwnd, None);
            let our_tid = GetCurrentThreadId();

            let mut attached = false;
            let mut focused_hwnd = target_hwnd;

            if our_tid != target_tid && target_tid != 0 {
                if AttachThreadInput(our_tid, target_tid, true).as_bool() {
                    attached = true;
                    let focus = GetFocus();
                    if !focus.is_invalid() {
                        focused_hwnd = focus;
                    }
                }
            }

            let mut class_name = [0u16; 64];
            let len = GetClassNameW(focused_hwnd, &mut class_name);
            let name = if len > 0 {
                String::from_utf16_lossy(&class_name[..len as usize])
            } else {
                String::new()
            };
            let class_lower = name.to_lowercase();

            // Native Win32 edit controls (Scintilla in Notepad++, standard Edit, RichEdit)
            // support WM_COPY directly without emitting ANY synthetic keystrokes.
            // This 100% guarantees no 'c' character injection into the active document.
            let is_native_editor_control = class_lower.contains("scintilla")
                || class_lower == "edit"
                || class_lower.contains("richedit");

            if is_native_editor_control {
                let _ = SendMessageW(focused_hwnd, WM_COPY, WPARAM(0), LPARAM(0));
            } else {
                send_atomic_ctrl_c();
            }

            if attached {
                let _ = AttachThreadInput(our_tid, target_tid, false);
            }
        }
    }

    unsafe extern "system" fn mouse_hook_proc(
        n_code: i32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if n_code >= 0 && l_param.0 != 0 {
            let state_opt = COPY_ON_SELECT_STATE.lock().clone();
            if let Some(state) = state_opt {
                if state.is_enabled() {
                    let msg = w_param.0 as u32;
                    let info = &*(l_param.0 as *const MSLLHOOKSTRUCT);
                    let pt = (info.pt.x, info.pt.y);

                    // Skip synthetic/injected events (bit 0 of flags is LLMHF_INJECTED)
                    let flags_raw: u32 = std::mem::transmute_copy(&info.flags);
                    let is_injected = (flags_raw & 1) != 0;

                    if !is_injected {
                        match msg {
                            WM_LBUTTONDOWN => {
                                let now = Instant::now();
                                let mut tracker = CLICK_TRACKER.lock();
                                tracker.last_down_pt = pt;
                                tracker.last_down_time = now;
                            }
                            WM_LBUTTONUP => {
                                let now = Instant::now();
                                let mut tracker = CLICK_TRACKER.lock();
                                let down_pt = tracker.last_down_pt;
                                let last_up_pt = tracker.last_up_pt;
                                let last_up_time = tracker.last_up_time;
                                let last_trigger = tracker.last_trigger_time;

                                tracker.last_up_pt = pt;
                                tracker.last_up_time = now;

                                // 1. Check drag distance: >= 12px (144 distance squared to filter out click jitter)
                                let dx = pt.0 - down_pt.0;
                                let dy = pt.1 - down_pt.1;
                                let drag_dist_sq = dx * dx + dy * dy;
                                let is_drag = drag_dist_sq >= 144; // 12px threshold

                                // 2. Check double click (word selection)
                                let dbl_click_enabled = state.is_double_click_enabled();
                                let dbl_time_limit = GetDoubleClickTime().max(200) as u128;
                                let cx_dbl = GetSystemMetrics(SM_CXDOUBLECLK).max(4);
                                let cy_dbl = GetSystemMetrics(SM_CYDOUBLECLK).max(4);

                                let is_double_click = if dbl_click_enabled {
                                    let up_interval = now.duration_since(last_up_time).as_millis();
                                    let click_dx = (pt.0 - last_up_pt.0).abs();
                                    let click_dy = (pt.1 - last_up_pt.1).abs();
                                    up_interval <= dbl_time_limit
                                        && click_dx <= cx_dbl
                                        && click_dy <= cy_dbl
                                } else {
                                    false
                                };

                                let should_trigger = is_drag || is_double_click;

                                // Debounce: avoid duplicate triggers within 150ms
                                if should_trigger
                                    && now.duration_since(last_trigger).as_millis() > 150
                                {
                                    if !is_excluded_context() {
                                        tracker.last_trigger_time = now;
                                        let is_smart_replace = state.is_smart_replace_enabled();
                                        // Spawn a lightweight task to delay-send copy so host app settles selection
                                        std::thread::spawn(move || {
                                            let target_hwnd = unsafe { GetForegroundWindow() };
                                            if target_hwnd.is_invalid() {
                                                return;
                                            }
                                            // Capture previous clipboard text before copy
                                            let preserved_text = if is_smart_replace {
                                                arboard::Clipboard::new()
                                                    .ok()
                                                    .and_then(|mut cb| cb.get_text().ok())
                                            } else {
                                                None
                                            };

                                            std::thread::sleep(Duration::from_millis(80));
                                            let current_fg = unsafe { GetForegroundWindow() };
                                            if current_fg == target_hwnd && !is_excluded_context() {
                                                execute_copy(target_hwnd);

                                                if is_smart_replace {
                                                    if let Some(old_text) = preserved_text {
                                                        if !old_text.trim().is_empty() {
                                                            // Wait briefly for host app to populate clipboard with selected text
                                                            std::thread::sleep(Duration::from_millis(60));
                                                            let new_text = arboard::Clipboard::new()
                                                                .ok()
                                                                .and_then(|mut cb| cb.get_text().ok());
                                                            // Arm only if new selection actually changed clipboard content
                                                            if let Some(new_t) = new_text {
                                                                if new_t != old_text && !new_t.trim().is_empty() {
                                                                    let mut r_tracker = REPLACE_TRACKER.lock();
                                                                    r_tracker.armed = true;
                                                                    r_tracker.target_hwnd = target_hwnd.0 as isize;
                                                                    r_tracker.preserved_text = Some(old_text);
                                                                    r_tracker.trigger_time = Instant::now();
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        });
                                    }
                                } else if !should_trigger {
                                    // User clicked elsewhere to move cursor or deselect
                                    let mut r_tracker = REPLACE_TRACKER.lock();
                                    if r_tracker.armed {
                                        r_tracker.armed = false;
                                        r_tracker.preserved_text = None;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
    }

    unsafe extern "system" fn keyboard_hook_proc(
        n_code: i32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if n_code >= 0 && l_param.0 != 0 {
            let state_opt = COPY_ON_SELECT_STATE.lock().clone();
            if let Some(state) = state_opt {
                if state.is_enabled() && state.is_smart_replace_enabled() {
                    let msg = w_param.0 as u32;
                    if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                        let kbd = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
                        let flags_raw: u32 = std::mem::transmute_copy(&kbd.flags);
                        let is_injected = (flags_raw & 1) != 0;

                        if !is_injected {
                            let vk = kbd.vkCode;
                            let is_ctrl = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
                            let is_alt = (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0;
                            let is_win = ((GetAsyncKeyState(VK_LWIN.0 as i32) as u16 & 0x8000) != 0)
                                || ((GetAsyncKeyState(VK_RWIN.0 as i32) as u16 & 0x8000) != 0);

                            // Detect Ctrl+V (0x56 is virtual key code for 'V')
                            if vk == 0x56 && is_ctrl && !is_alt && !is_win {
                                let mut r_tracker = REPLACE_TRACKER.lock();
                                if r_tracker.armed {
                                    let now = Instant::now();
                                    let cur_hwnd = GetForegroundWindow();
                                    if now.duration_since(r_tracker.trigger_time).as_secs() <= 5
                                        && cur_hwnd.0 as isize == r_tracker.target_hwnd
                                    {
                                        if let Some(ref text_to_restore) = r_tracker.preserved_text {
                                            if let Ok(mut cb) = arboard::Clipboard::new() {
                                                let _ = cb.set_text(text_to_restore.clone());
                                                println!("[CopyOnSelect] In-place replace detected: restored preserved text before paste");
                                            }
                                        }
                                    }
                                    r_tracker.armed = false;
                                    r_tracker.preserved_text = None;
                                }
                            } else {
                                // For non-modifier keys, if it's not backspace/delete/shift, disarm replace tracker
                                const VK_BACK_CODE: u32 = 0x08;
                                const VK_DELETE_CODE: u32 = 0x2E;
                                const VK_SHIFT_CODE: u32 = 0x10;
                                let is_modifier = vk == VK_CONTROL.0 as u32
                                    || vk == VK_MENU.0 as u32
                                    || vk == VK_LWIN.0 as u32
                                    || vk == VK_RWIN.0 as u32
                                    || vk == VK_SHIFT_CODE;
                                let is_del = vk == VK_BACK_CODE || vk == VK_DELETE_CODE;

                                if !is_modifier && !is_del {
                                    let mut r_tracker = REPLACE_TRACKER.lock();
                                    if r_tracker.armed {
                                        r_tracker.armed = false;
                                        r_tracker.preserved_text = None;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
    }

    pub fn start_monitor(state: Arc<CopyOnSelectState>) {
        *COPY_ON_SELECT_STATE.lock() = Some(state);

        std::thread::spawn(|| unsafe {
            let mouse_hook =
                SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), HINSTANCE::default(), 0);
            let kbd_hook =
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), HINSTANCE::default(), 0);

            let mouse_ok = mouse_hook.is_ok();
            let kbd_ok = kbd_hook.is_ok();

            if mouse_ok || kbd_ok {
                println!(
                    "[CopyOnSelect] Low-level hooks installed (mouse: {}, keyboard: {})",
                    mouse_ok, kbd_ok
                );
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                if let Ok(h) = mouse_hook {
                    let _ = UnhookWindowsHookEx(h);
                }
                if let Ok(h) = kbd_hook {
                    let _ = UnhookWindowsHookEx(h);
                }
            } else {
                eprintln!("[CopyOnSelect] Failed to install low-level hooks");
            }
        });
    }
}

pub fn start_copy_on_select_monitor(state: Arc<CopyOnSelectState>) {
    #[cfg(target_os = "windows")]
    {
        windows_impl::start_monitor(state);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = state;
    }
}
