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
        self.double_click_enabled
            .store(double_click_enabled, Ordering::Relaxed);
        self.smart_replace_enabled
            .store(smart_replace_enabled, Ordering::Relaxed);

        #[cfg(target_os = "windows")]
        {
            windows_impl::sync_hook_lifecycle(enabled);
        }
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
    use std::sync::atomic::AtomicU64;
    use std::sync::LazyLock;
    use std::time::{Duration, Instant};
    use uiautomation::patterns::UITextPattern;
    use uiautomation::types::{ControlType, Point, SupportedTextSelection};
    use uiautomation::{UIAutomation, UIElement};
    use windows::Win32::Foundation::{
        CloseHandle, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, RPC_E_CHANGED_MODE, WPARAM,
    };
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
    use windows::Win32::System::Threading::{
        GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, GetDoubleClickTime, MapVirtualKeyW, SendInput, INPUT, INPUT_0,
        INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, VIRTUAL_KEY, VK_CONTROL,
        VK_LBUTTON, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MBUTTON, VK_MENU, VK_RBUTTON,
        VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetClassNameW, GetClientRect, GetCursorInfo,
        GetForegroundWindow, GetGUIThreadInfo, GetMessageW, GetSystemMetrics, GetWindowRect,
        GetWindowThreadProcessId, LoadCursorW, PeekMessageW, PostThreadMessageW,
        SendMessageTimeoutW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, CURSORINFO,
        GUITHREADINFO, GUI_INMENUMODE, GUI_INMOVESIZE, GUI_POPUPMENUMODE, GUI_SYSTEMMENUMODE,
        HHOOK, IDC_APPSTARTING, IDC_IBEAM, IDC_NO, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS,
        IDC_SIZENWSE, IDC_SIZEWE, IDC_WAIT, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE,
        SMTO_ABORTIFHUNG, SM_CXDOUBLECLK, SM_CYDOUBLECLK, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_COPY,
        WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MOUSEWHEEL, WM_QUIT,
        WM_RBUTTONDOWN, WM_SYSKEYDOWN,
    };

    /// Unique marker tag placed in KEYBDINPUT.dwExtraInfo so our own hooks can
    /// reliably distinguish synthetic keys emitted by Qollect from user input.
    const QOLLECT_INJECTED_EXTRA_INFO: usize = 0x514F_4C43; // "QOLC"

    /// KBDLLHOOKSTRUCT.flags bit masks (note: bit 0 is LLKHF_EXTENDED, NOT injected!).
    const LLKHF_LOWER_IL_INJECTED: u32 = 0x0000_0002;
    const LLKHF_INJECTED: u32 = 0x0000_0010;

    /// Win32 edit & Scintilla messages used for zero-keystroke selection probing.
    const EM_GETSEL: u32 = 0x00B0;
    const SCI_GETSELECTIONEMPTY: u32 = 2650;

    /// Upper bound for direct UIA text read to avoid stalling on multi-megabyte documents.
    const MAX_UIA_SELECTION_CHARS: i32 = 500_000;

    /// Gesture thresholds tuned to eliminate accidental triggers while keeping text selection responsive.
    const DRAG_MIN_DIST_SQ: i32 = 196; // 14px squared
    const DRAG_MIN_HOLD_MS: u128 = 45;
    const DRAG_MAX_HOLD_MS: u128 = 15_000;
    const DOUBLE_CLICK_MAX_HOLD_MS: u128 = 250;
    const DOUBLE_CLICK_MAX_DRIFT_SQ: i32 = 36; // 6px squared
    const TRIGGER_DEBOUNCE_MS: u128 = 180;
    const DOUBLE_CLICK_COOLDOWN_MS: u128 = 450;
    const TYPING_QUIET_WINDOW_MS: u128 = 150;
    const SELECTION_SETTLE_DELAY_MS: u64 = 100;

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
        // Remote desktop & virtualization tools (modifier desync over remote protocols causes stray 'c')
        "mstsc.exe",
        "teamviewer.exe",
        "anydesk.exe",
        "vncviewer.exe",
        "rustdesk.exe",
        "todesk.exe",
        "sunloginclient.exe",
        "parsec.exe",
        "moonlight.exe",
        "vmware.exe",
        "vmware-vmx.exe",
        "virtualboxvm.exe",
        "vmconnect.exe",
    ];

    static COPY_ON_SELECT_STATE: LazyLock<Mutex<Option<Arc<CopyOnSelectState>>>> =
        LazyLock::new(|| Mutex::new(None));

    /// Monotonic counter bumped on ANY real user keyboard event or new mouse action.
    /// Any pending copy task compares its captured generation against this counter
    /// and aborts immediately if the user started typing or clicking again.
    static INPUT_GENERATION: AtomicU64 = AtomicU64::new(0);

    /// Serializes copy execution so two synthetic Ctrl+C sequences can never interleave
    /// (interleaving Thread A's Ctrl-Up with Thread B's C-Down guarantees a stray 'c').
    static COPY_EXEC_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    struct TypingTracker {
        last_non_modifier_key_time: Instant,
    }

    static TYPING_TRACKER: LazyLock<Mutex<TypingTracker>> = LazyLock::new(|| {
        let past = Instant::now()
            .checked_sub(Duration::from_secs(10))
            .unwrap_or_else(Instant::now);
        Mutex::new(TypingTracker {
            last_non_modifier_key_time: past,
        })
    });

    struct SystemCursors {
        ibeam: isize,
        excluded: [isize; 8],
    }

    static SYSTEM_CURSORS: LazyLock<SystemCursors> = LazyLock::new(|| unsafe {
        let load = |id| {
            LoadCursorW(HINSTANCE::default(), id)
                .map(|c| c.0 as isize)
                .unwrap_or(0)
        };
        SystemCursors {
            ibeam: load(IDC_IBEAM),
            excluded: [
                load(IDC_SIZEALL),
                load(IDC_SIZENS),
                load(IDC_SIZEWE),
                load(IDC_SIZENWSE),
                load(IDC_SIZENESW),
                load(IDC_NO),
                load(IDC_WAIT),
                load(IDC_APPSTARTING),
            ],
        }
    });

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum TriggerKind {
        Drag,
        DoubleClick,
    }

    struct MouseClickState {
        down_valid: bool,
        down_pt: (i32, i32),
        down_time: Instant,
        down_hwnd: isize,
        down_win_rect: Option<RECT>,
        down_cursor: isize,
        down_in_client: bool,
        down_generation: u64,
        last_clean_click_pt: (i32, i32),
        last_clean_click_time: Instant,
        last_clean_click_hwnd: isize,
        last_trigger_time: Instant,
        last_double_click_trigger_time: Instant,
    }

    static CLICK_TRACKER: LazyLock<Mutex<MouseClickState>> = LazyLock::new(|| {
        let past = Instant::now()
            .checked_sub(Duration::from_secs(10))
            .unwrap_or_else(Instant::now);
        Mutex::new(MouseClickState {
            down_valid: false,
            down_pt: (0, 0),
            down_time: past,
            down_hwnd: 0,
            down_win_rect: None,
            down_cursor: 0,
            down_in_client: false,
            down_generation: 0,
            last_clean_click_pt: (0, 0),
            last_clean_click_time: past,
            last_clean_click_hwnd: 0,
            last_trigger_time: past,
            last_double_click_trigger_time: past,
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

    struct CopyRequest {
        target_hwnd: isize,
        down_pt: (i32, i32),
        up_pt: (i32, i32),
        kind: TriggerKind,
        was_ibeam: bool,
        expected_gen: u64,
        is_smart_replace: bool,
    }

    enum NativeFocusState {
        /// Window is currently moving/resizing or in a menu -> must abort.
        BusyAbort,
        /// Native Edit/RichEdit/Scintilla with confirmed empty selection -> must abort.
        NativeEmptyAbort,
        /// Native Edit/RichEdit/Scintilla ready for zero-keystroke WM_COPY.
        NativeCopyable(HWND),
        /// Modern/custom window (Chromium, Electron, Office, Qt, etc.).
        NonNative,
    }

    enum TextPatternCheck {
        Selected(String),
        ConfirmedEmpty,
        Unsupported,
    }

    enum UiaProbeOutcome {
        /// Retrieved selected text directly via UI Automation (0 simulated keystrokes).
        SelectedText(String),
        /// Confirmed non-text chrome control, password box, or empty selection -> must abort.
        Abort,
        /// UIA did not expose an active text selection bridge -> cautious fallback allowed.
        Unavailable,
    }

    enum CopyOutcome {
        Aborted,
        CopiedDirectly(String),
        TriggeredViaMessageOrKey,
    }

    struct ComGuard {
        should_uninit: bool,
    }

    impl ComGuard {
        fn init_multithreaded() -> Option<Self> {
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if hr.is_ok() {
                Some(Self {
                    should_uninit: true,
                })
            } else if hr == RPC_E_CHANGED_MODE {
                Some(Self {
                    should_uninit: false,
                })
            } else {
                None
            }
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.should_uninit {
                unsafe {
                    CoUninitialize();
                }
            }
        }
    }

    #[inline]
    fn is_vk_down(vk: VIRTUAL_KEY) -> bool {
        unsafe { (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0 }
    }

    #[inline]
    fn any_modifier_down() -> bool {
        is_vk_down(VK_CONTROL)
            || is_vk_down(VK_LCONTROL)
            || is_vk_down(VK_RCONTROL)
            || is_vk_down(VK_MENU)
            || is_vk_down(VK_LMENU)
            || is_vk_down(VK_RMENU)
            || is_vk_down(VK_SHIFT)
            || is_vk_down(VK_LSHIFT)
            || is_vk_down(VK_RSHIFT)
            || is_vk_down(VK_LWIN)
            || is_vk_down(VK_RWIN)
    }

    #[inline]
    fn any_mouse_button_down() -> bool {
        is_vk_down(VK_LBUTTON) || is_vk_down(VK_RBUTTON) || is_vk_down(VK_MBUTTON)
    }

    #[inline]
    fn is_non_ctrl_modifier_down() -> bool {
        is_vk_down(VK_MENU)
            || is_vk_down(VK_LMENU)
            || is_vk_down(VK_RMENU)
            || is_vk_down(VK_SHIFT)
            || is_vk_down(VK_LSHIFT)
            || is_vk_down(VK_RSHIFT)
            || is_vk_down(VK_LWIN)
            || is_vk_down(VK_RWIN)
    }

    #[inline]
    fn current_cursor_handle() -> isize {
        unsafe {
            let mut ci = CURSORINFO {
                cbSize: std::mem::size_of::<CURSORINFO>() as u32,
                ..Default::default()
            };
            if GetCursorInfo(&mut ci).is_ok() {
                ci.hCursor.0 as isize
            } else {
                0
            }
        }
    }

    #[inline]
    fn is_excluded_cursor(handle: isize) -> bool {
        if handle == 0 {
            return false;
        }
        SYSTEM_CURSORS
            .excluded
            .iter()
            .any(|&ex| ex != 0 && ex == handle)
    }

    #[inline]
    fn is_ibeam_cursor(handle: isize) -> bool {
        handle != 0 && SYSTEM_CURSORS.ibeam != 0 && handle == SYSTEM_CURSORS.ibeam
    }

    fn get_window_rect(hwnd: HWND) -> Option<RECT> {
        unsafe {
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_ok() {
                Some(rect)
            } else {
                None
            }
        }
    }

    fn is_point_in_client_area(hwnd: HWND, pt: (i32, i32)) -> bool {
        unsafe {
            let mut client = RECT::default();
            if GetClientRect(hwnd, &mut client).is_err() {
                return false;
            }
            let mut top_left = POINT {
                x: client.left,
                y: client.top,
            };
            let mut bottom_right = POINT {
                x: client.right,
                y: client.bottom,
            };
            if !ClientToScreen(hwnd, &mut top_left).as_bool()
                || !ClientToScreen(hwnd, &mut bottom_right).as_bool()
            {
                return false;
            }
            pt.0 >= top_left.x
                && pt.0 < bottom_right.x
                && pt.1 >= top_left.y
                && pt.1 < bottom_right.y
        }
    }

    fn rects_equal(a: Option<RECT>, b: Option<RECT>) -> bool {
        match (a, b) {
            (Some(r1), Some(r2)) => {
                r1.left == r2.left
                    && r1.top == r2.top
                    && r1.right == r2.right
                    && r1.bottom == r2.bottom
            }
            _ => false,
        }
    }

    fn get_process_exe_name(process_id: u32) -> Option<String> {
        use windows::core::PWSTR;

        unsafe {
            // Use PROCESS_QUERY_LIMITED_INFORMATION (not PROCESS_VM_READ) so elevated
            // terminals/processes can still be identified and matched against the blacklist.
            let handle =
                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()?;

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

    fn is_excluded_context(check_left_mouse_button: bool) -> bool {
        unsafe {
            // 1. Exclude if ANY modifier key (Ctrl/Alt/Shift/Win) or mouse button is physically held.
            // Note: Inside WH_MOUSE_LL's WM_LBUTTONUP callback, GetAsyncKeyState(VK_LBUTTON) may still
            // report down until CallNextHookEx completes, so callers pass check_left_mouse_button=false there.
            let mouse_held = if check_left_mouse_button {
                any_mouse_button_down()
            } else {
                is_vk_down(VK_RBUTTON) || is_vk_down(VK_MBUTTON)
            };
            if any_modifier_down() || mouse_held {
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

            // 3. Check excluded process blacklist (terminals, remote desktops, VMs).
            // If OpenProcess fails (e.g. protected/elevated process), conservatively exclude.
            match get_process_exe_name(pid) {
                Some(exe_name) => {
                    if EXCLUDED_PROCESS_NAMES.iter().any(|&item| item == exe_name) {
                        return true;
                    }
                }
                None => return true,
            }

            false
        }
    }

    /// Inspects the foreground window's GUI thread state WITHOUT calling AttachThreadInput,
    /// avoiding any corruption or reset of the target thread's keyboard modifier state.
    fn inspect_native_focus(target_hwnd: HWND) -> NativeFocusState {
        unsafe {
            let target_tid = GetWindowThreadProcessId(target_hwnd, None);
            let mut focused_hwnd = target_hwnd;

            if target_tid != 0 {
                let mut gui = GUITHREADINFO {
                    cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                    ..Default::default()
                };
                if GetGUIThreadInfo(target_tid, &mut gui).is_ok() {
                    if gui.flags.contains(GUI_INMOVESIZE)
                        || gui.flags.contains(GUI_INMENUMODE)
                        || gui.flags.contains(GUI_POPUPMENUMODE)
                        || gui.flags.contains(GUI_SYSTEMMENUMODE)
                    {
                        return NativeFocusState::BusyAbort;
                    }
                    if !gui.hwndFocus.is_invalid() {
                        focused_hwnd = gui.hwndFocus;
                    }
                }
            }

            let mut class_name = [0u16; 64];
            let len = GetClassNameW(focused_hwnd, &mut class_name);
            let class_lower = if len > 0 {
                String::from_utf16_lossy(&class_name[..len as usize]).to_lowercase()
            } else {
                String::new()
            };

            if class_lower == "edit" || class_lower.contains("richedit") {
                // Passing (0, 0) safely returns MAKELONG(start, end) without cross-process pointers.
                let mut result: usize = 0;
                let ok = SendMessageTimeoutW(
                    focused_hwnd,
                    EM_GETSEL,
                    WPARAM(0),
                    LPARAM(0),
                    SMTO_ABORTIFHUNG,
                    50,
                    Some(&mut result),
                );
                if ok.0 != 0 {
                    let start = result & 0xFFFF;
                    let end = (result >> 16) & 0xFFFF;
                    if start == end && start < 0xFFFF {
                        return NativeFocusState::NativeEmptyAbort;
                    }
                }
                return NativeFocusState::NativeCopyable(focused_hwnd);
            }

            if class_lower.contains("scintilla") {
                let mut empty: usize = 0;
                let ok = SendMessageTimeoutW(
                    focused_hwnd,
                    SCI_GETSELECTIONEMPTY,
                    WPARAM(0),
                    LPARAM(0),
                    SMTO_ABORTIFHUNG,
                    50,
                    Some(&mut empty),
                );
                if ok.0 != 0 && empty != 0 {
                    return NativeFocusState::NativeEmptyAbort;
                }
                return NativeFocusState::NativeCopyable(focused_hwnd);
            }

            NativeFocusState::NonNative
        }
    }

    fn is_non_text_chrome(ct: ControlType) -> bool {
        matches!(
            ct,
            ControlType::ScrollBar
                | ControlType::Thumb
                | ControlType::TitleBar
                | ControlType::Slider
                | ControlType::Spinner
                | ControlType::ProgressBar
                | ControlType::MenuBar
                | ControlType::Menu
                | ControlType::MenuItem
                | ControlType::Tab
                | ControlType::TabItem
                | ControlType::Header
                | ControlType::HeaderItem
                | ControlType::Separator
                | ControlType::CheckBox
                | ControlType::RadioButton
                | ControlType::SplitButton
                | ControlType::ToolBar
                | ControlType::StatusBar
                | ControlType::AppBar
        )
    }

    fn same_uia_element(a: &UIElement, b: &UIElement) -> bool {
        match (a.get_runtime_id(), b.get_runtime_id()) {
            (Ok(id_a), Ok(id_b)) => !id_a.is_empty() && id_a == id_b,
            _ => false,
        }
    }

    fn try_read_text_pattern(elem: &UIElement) -> TextPatternCheck {
        let Ok(pattern) = elem.get_pattern::<UITextPattern>() else {
            return TextPatternCheck::Unsupported;
        };

        if let Ok(SupportedTextSelection::None) = pattern.get_supported_text_selection() {
            return TextPatternCheck::Unsupported;
        }

        let Ok(ranges) = pattern.get_selection() else {
            return TextPatternCheck::Unsupported;
        };

        let mut combined = String::new();
        for range in ranges {
            if let Ok(chunk) = range.get_text(MAX_UIA_SELECTION_CHARS) {
                if !chunk.is_empty() {
                    combined.push_str(&chunk);
                }
            }
        }

        // Ignore object replacement placeholders (\u{FFFC}) with no real text.
        let has_real_text = combined.chars().any(|c| c != '\u{FFFC}');
        if has_real_text && !combined.is_empty() {
            // If selection hit the safety truncation cap, let fallback handle full copy.
            if combined.chars().count() as i32 >= MAX_UIA_SELECTION_CHARS {
                return TextPatternCheck::Unsupported;
            }
            return TextPatternCheck::Selected(combined);
        }

        // Selection is empty. Verify that this element's UIA text bridge is genuinely alive
        // (not a stub returning empty strings) before trusting "empty selection" as authoritative.
        if let Ok(doc_head) = pattern
            .get_document_range()
            .and_then(|doc| doc.get_text(1))
        {
            if !doc_head.is_empty() {
                return TextPatternCheck::ConfirmedEmpty;
            }
            if matches!(elem.get_control_type(), Ok(ControlType::Edit)) {
                // Live UIA Edit control with completely empty document -> impossible to have selection.
                return TextPatternCheck::ConfirmedEmpty;
            }
        }

        TextPatternCheck::Unsupported
    }

    fn probe_uia_selection(
        down_pt: (i32, i32),
        up_pt: (i32, i32),
        kind: TriggerKind,
        was_ibeam: bool,
    ) -> UiaProbeOutcome {
        let Some(_com_guard) = ComGuard::init_multithreaded() else {
            return UiaProbeOutcome::Unavailable;
        };

        let Ok(automation) = UIAutomation::new_direct() else {
            return UiaProbeOutcome::Unavailable;
        };

        let hit_up = automation
            .element_from_point(Point::new(up_pt.0, up_pt.1))
            .ok();
        let hit_down = if down_pt == up_pt {
            None
        } else {
            automation
                .element_from_point(Point::new(down_pt.0, down_pt.1))
                .ok()
        };
        let focused = automation.get_focused_element().ok();

        // 1. Never copy from password fields.
        for elem in [&focused, &hit_up, &hit_down].into_iter().flatten() {
            if elem.is_password().unwrap_or(false) {
                return UiaProbeOutcome::Abort;
            }
        }

        // 2. Reject non-text UI chrome (scrollbars, thumbs, titlebars, sliders, tabs, menus, etc.).
        let up_ct = hit_up.as_ref().and_then(|e| e.get_control_type().ok());
        let down_ct = hit_down.as_ref().and_then(|e| e.get_control_type().ok());

        if up_ct.is_some_and(is_non_text_chrome) || down_ct.is_some_and(is_non_text_chrome) {
            return UiaProbeOutcome::Abort;
        }
        if up_ct == Some(ControlType::Button)
            && (down_ct == Some(ControlType::Button) || down_pt == up_pt)
        {
            return UiaProbeOutcome::Abort;
        }
        if kind == TriggerKind::DoubleClick
            && matches!(
                up_ct,
                Some(ControlType::Image | ControlType::TreeItem | ControlType::Window)
            )
        {
            return UiaProbeOutcome::Abort;
        }

        // 3. Build ancestor chain from hit_up (up to 4 levels, stopping at Window) because in
        // Chromium/Edge/Electron, element_from_point hits a leaf Text node while UITextPattern
        // is exposed on its enclosing Document/Edit ancestor.
        let mut hit_chain: Vec<UIElement> = Vec::with_capacity(5);
        if let Some(up_elem) = hit_up {
            hit_chain.push(up_elem);
            if let Ok(walker) = automation.get_control_view_walker() {
                for _ in 0..4 {
                    let next_parent = hit_chain.last().and_then(|e| walker.get_parent(e).ok());
                    let Some(parent) = next_parent else {
                        break;
                    };
                    if matches!(parent.get_control_type(), Ok(ControlType::Window)) {
                        break;
                    }
                    if hit_chain.iter().any(|existing| same_uia_element(existing, &parent)) {
                        break;
                    }
                    hit_chain.push(parent);
                }
            }
        }

        let mut hit_confirmed_empty = false;
        for elem in &hit_chain {
            match try_read_text_pattern(elem) {
                TextPatternCheck::Selected(text) => {
                    return UiaProbeOutcome::SelectedText(text);
                }
                TextPatternCheck::ConfirmedEmpty => {
                    hit_confirmed_empty = true;
                }
                TextPatternCheck::Unsupported => {}
            }
        }

        let mut focused_confirmed_empty = false;
        if let Some(ref foc) = focused {
            let already_checked = hit_chain.iter().any(|e| same_uia_element(e, foc));
            if !already_checked {
                match try_read_text_pattern(foc) {
                    TextPatternCheck::Selected(text) => {
                        return UiaProbeOutcome::SelectedText(text);
                    }
                    TextPatternCheck::ConfirmedEmpty => {
                        focused_confirmed_empty = true;
                    }
                    TextPatternCheck::Unsupported => {}
                }
            }
        }

        if let Some(ref down_elem) = hit_down {
            let already_checked = hit_chain.iter().any(|e| same_uia_element(e, down_elem))
                || focused
                    .as_ref()
                    .is_some_and(|f| same_uia_element(f, down_elem));
            if !already_checked {
                if let TextPatternCheck::Selected(text) = try_read_text_pattern(down_elem) {
                    return UiaProbeOutcome::SelectedText(text);
                }
            }
        }

        // Abort if the clicked/dragged region itself confirmed an empty selection, or if the
        // focused control confirmed an empty selection while the cursor wasn't even an I-beam.
        if hit_confirmed_empty || (focused_confirmed_empty && !was_ibeam) {
            return UiaProbeOutcome::Abort;
        }

        UiaProbeOutcome::Unavailable
    }

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
                    dwExtraInfo: QOLLECT_INJECTED_EXTRA_INFO,
                },
            },
        }
    }

    fn send_single_key(vk: u16, scan: u16, key_up: bool) -> bool {
        let input = key_input(vk, scan, key_up);
        let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
        sent == 1
    }

    /// RAII guard guaranteeing VK_LCONTROL is always released even on early abort.
    struct LeftCtrlGuard {
        scan: u16,
        active: bool,
    }

    impl Drop for LeftCtrlGuard {
        fn drop(&mut self) {
            if self.active {
                let _ = send_single_key(VK_LCONTROL.0, self.scan, true);
                self.active = false;
            }
        }
    }

    /// Emits a verified, staged Left-Ctrl + C chord:
    /// 1. Presses concrete VK_LCONTROL (not generic VK_CONTROL) and waits 20ms.
    /// 2. Re-verifies VK_LCONTROL is actually held in OS async key state, no other modifier
    ///    is held, focus has not changed, and no user input arrived before emitting 'C'.
    ///    If Ctrl did not stick (e.g. UIPI / hook interference), aborts WITHOUT sending 'C'.
    /// 3. Emits C-Down -> 15ms -> C-Up -> 30ms trailing Ctrl hold -> VK_LCONTROL-Up so
    ///    async consumers (Chromium/Electron/IME/TSF) never observe Ctrl already released
    ///    when translating WM_KEYDOWN('C').
    fn send_safe_ctrl_c(expected_gen: u64, target_hwnd: HWND) -> bool {
        const VK_C_KEY: u16 = 0x43;
        const FALLBACK_SCAN_LCTRL: u16 = 0x1D;
        const FALLBACK_SCAN_C: u16 = 0x2E;

        let scan_ctrl = unsafe {
            let mapped = MapVirtualKeyW(VK_LCONTROL.0 as u32, MAPVK_VK_TO_VSC) as u16;
            if mapped != 0 {
                mapped
            } else {
                FALLBACK_SCAN_LCTRL
            }
        };
        let scan_c = unsafe {
            let mapped = MapVirtualKeyW(VK_C_KEY as u32, MAPVK_VK_TO_VSC) as u16;
            if mapped != 0 {
                mapped
            } else {
                FALLBACK_SCAN_C
            }
        };

        if !send_single_key(VK_LCONTROL.0, scan_ctrl, false) {
            return false;
        }
        let ctrl_guard = LeftCtrlGuard {
            scan: scan_ctrl,
            active: true,
        };

        // Allow OS input queue, TSF/IME, and target message pump to register Ctrl-Down.
        std::thread::sleep(Duration::from_millis(20));

        // Final pre-C verification: abort immediately (releasing Ctrl) if anything is off.
        let ctrl_is_down = is_vk_down(VK_LCONTROL) || is_vk_down(VK_CONTROL);
        let fg_matches = unsafe { GetForegroundWindow() == target_hwnd };
        let gen_matches = INPUT_GENERATION.load(Ordering::SeqCst) == expected_gen;

        if !ctrl_is_down
            || !fg_matches
            || !gen_matches
            || is_non_ctrl_modifier_down()
            || any_mouse_button_down()
        {
            // Dropping ctrl_guard releases VK_LCONTROL cleanly without ever emitting 'C'.
            return false;
        }

        if !send_single_key(VK_C_KEY, scan_c, false) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(15));
        let _ = send_single_key(VK_C_KEY, scan_c, true);

        // Trailing hold before releasing Ctrl prevents async GetAsyncKeyState(VK_CONTROL)
        // readers in Chromium/Electron/IME from seeing Ctrl=up while handling WM_KEYDOWN('C').
        std::thread::sleep(Duration::from_millis(30));

        // Explicitly release Ctrl via guard drop.
        drop(ctrl_guard);
        true
    }

    fn execute_copy(req: &CopyRequest) -> CopyOutcome {
        let target_hwnd = HWND(req.target_hwnd as *mut std::ffi::c_void);

        // Layer 3A: Native Win32 Edit/RichEdit/Scintilla check via GetGUIThreadInfo (no AttachThreadInput!).
        match inspect_native_focus(target_hwnd) {
            NativeFocusState::BusyAbort | NativeFocusState::NativeEmptyAbort => {
                return CopyOutcome::Aborted;
            }
            NativeFocusState::NativeCopyable(focused_hwnd) => {
                if INPUT_GENERATION.load(Ordering::SeqCst) != req.expected_gen {
                    return CopyOutcome::Aborted;
                }
                let ok = unsafe {
                    SendMessageTimeoutW(
                        focused_hwnd,
                        WM_COPY,
                        WPARAM(0),
                        LPARAM(0),
                        SMTO_ABORTIFHUNG,
                        100,
                        None,
                    )
                };
                return if ok.0 != 0 {
                    CopyOutcome::TriggeredViaMessageOrKey
                } else {
                    CopyOutcome::Aborted
                };
            }
            NativeFocusState::NonNative => {}
        }

        // Layer 3B: Windows UI Automation zero-keystroke selection read & non-text chrome filter.
        match probe_uia_selection(req.down_pt, req.up_pt, req.kind, req.was_ibeam) {
            UiaProbeOutcome::SelectedText(selected) => {
                if INPUT_GENERATION.load(Ordering::SeqCst) != req.expected_gen {
                    return CopyOutcome::Aborted;
                }
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    if cb.set_text(selected.clone()).is_ok() {
                        return CopyOutcome::CopiedDirectly(selected);
                    }
                }
                return CopyOutcome::Aborted;
            }
            UiaProbeOutcome::Abort => {
                return CopyOutcome::Aborted;
            }
            UiaProbeOutcome::Unavailable => {}
        }

        // Layer 4: Conservative admission gate for legacy/custom apps without UIA TextPattern.
        if !req.was_ibeam {
            match req.kind {
                // Double-clicking on non-I-beam cursors in non-UIA apps is overwhelmingly
                // opening files/items/buttons rather than selecting words.
                TriggerKind::DoubleClick => return CopyOutcome::Aborted,
                // Non-I-beam drag requires a clearer horizontal or larger sweep to avoid
                // accidental scrollbar/canvas/micro-drag triggers.
                TriggerKind::Drag => {
                    let dx = (req.up_pt.0 - req.down_pt.0).abs();
                    let dy = (req.up_pt.1 - req.down_pt.1).abs();
                    let dist_sq = dx * dx + dy * dy;
                    if dx < 18 && dist_sq < 784 {
                        return CopyOutcome::Aborted;
                    }
                }
            }
        }

        // Layer 5: Serialized, verified staged Ctrl+C fallback.
        let Some(_exec_guard) = COPY_EXEC_LOCK.try_lock() else {
            return CopyOutcome::Aborted;
        };

        let current_fg = unsafe { GetForegroundWindow() };
        if INPUT_GENERATION.load(Ordering::SeqCst) != req.expected_gen
            || current_fg != target_hwnd
            || is_excluded_context(true)
        {
            return CopyOutcome::Aborted;
        }

        if send_safe_ctrl_c(req.expected_gen, target_hwnd) {
            CopyOutcome::TriggeredViaMessageOrKey
        } else {
            CopyOutcome::Aborted
        }
    }

    fn run_copy_task(req: CopyRequest) {
        let target_hwnd = HWND(req.target_hwnd as *mut std::ffi::c_void);
        if target_hwnd.is_invalid() {
            return;
        }

        // Capture previous clipboard text before copy for smart-replace support.
        let preserved_text = if req.is_smart_replace {
            arboard::Clipboard::new()
                .ok()
                .and_then(|mut cb| cb.get_text().ok())
        } else {
            None
        };

        std::thread::sleep(Duration::from_millis(SELECTION_SETTLE_DELAY_MS));

        let current_fg = unsafe { GetForegroundWindow() };
        if INPUT_GENERATION.load(Ordering::SeqCst) != req.expected_gen
            || current_fg != target_hwnd
            || is_excluded_context(true)
        {
            return;
        }

        let outcome = execute_copy(&req);
        if !req.is_smart_replace {
            return;
        }

        let Some(old_text) = preserved_text else {
            return;
        };
        if old_text.trim().is_empty() {
            return;
        }

        let new_text = match outcome {
            CopyOutcome::Aborted => return,
            CopyOutcome::CopiedDirectly(text) => Some(text),
            CopyOutcome::TriggeredViaMessageOrKey => {
                // Wait briefly for host app to populate clipboard with selected text.
                std::thread::sleep(Duration::from_millis(60));
                arboard::Clipboard::new()
                    .ok()
                    .and_then(|mut cb| cb.get_text().ok())
            }
        };

        // Arm smart-replace only if new selection actually changed clipboard content
        // and user did not already start typing during the copy.
        if INPUT_GENERATION.load(Ordering::SeqCst) != req.expected_gen {
            return;
        }
        if let Some(new_t) = new_text {
            if new_t != old_text && !new_t.trim().is_empty() {
                let mut r_tracker = REPLACE_TRACKER.lock();
                r_tracker.armed = true;
                r_tracker.target_hwnd = req.target_hwnd;
                r_tracker.preserved_text = Some(old_text);
                r_tracker.trigger_time = Instant::now();
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

                    // Skip synthetic/injected events (LLMHF_INJECTED = 0x1, LLMHF_LOWER_IL_INJECTED = 0x2)
                    let flags_raw: u32 = std::mem::transmute_copy(&info.flags);
                    let is_injected = (flags_raw & 0x3) != 0;

                    if !is_injected {
                        match msg {
                            WM_RBUTTONDOWN | WM_MBUTTONDOWN => {
                                INPUT_GENERATION.fetch_add(1, Ordering::SeqCst);
                                let mut tracker = CLICK_TRACKER.lock();
                                tracker.down_valid = false;
                            }
                            WM_MOUSEWHEEL => {
                                // Only cancel pending copy when not mid-drag (preserve drag-while-scrolling).
                                let tracker = CLICK_TRACKER.lock();
                                if !tracker.down_valid {
                                    INPUT_GENERATION.fetch_add(1, Ordering::SeqCst);
                                }
                            }
                            WM_LBUTTONDOWN => {
                                let now = Instant::now();
                                let new_gen = INPUT_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
                                let fg = GetForegroundWindow();
                                let win_rect = if !fg.is_invalid() {
                                    get_window_rect(fg)
                                } else {
                                    None
                                };
                                let in_client =
                                    !fg.is_invalid() && is_point_in_client_area(fg, pt);
                                let cursor = current_cursor_handle();

                                let mut tracker = CLICK_TRACKER.lock();
                                tracker.down_valid = true;
                                tracker.down_pt = pt;
                                tracker.down_time = now;
                                tracker.down_hwnd = fg.0 as isize;
                                tracker.down_win_rect = win_rect;
                                tracker.down_cursor = cursor;
                                tracker.down_in_client = in_client;
                                tracker.down_generation = new_gen;
                            }
                            WM_LBUTTONUP => {
                                let now = Instant::now();
                                let up_fg = GetForegroundWindow();
                                let up_hwnd = up_fg.0 as isize;
                                let up_cursor = current_cursor_handle();
                                let up_win_rect = if !up_fg.is_invalid() {
                                    get_window_rect(up_fg)
                                } else {
                                    None
                                };

                                let mut tracker = CLICK_TRACKER.lock();
                                let was_valid_down = tracker.down_valid;
                                tracker.down_valid = false;

                                let down_pt = tracker.down_pt;
                                let down_time = tracker.down_time;
                                let down_hwnd = tracker.down_hwnd;
                                let down_win_rect = tracker.down_win_rect;
                                let down_cursor = tracker.down_cursor;
                                let down_in_client = tracker.down_in_client;
                                let down_gen = tracker.down_generation;
                                let last_clean_pt = tracker.last_clean_click_pt;
                                let last_clean_time = tracker.last_clean_click_time;
                                let last_clean_hwnd = tracker.last_clean_click_hwnd;
                                let last_trigger = tracker.last_trigger_time;
                                let last_dbl_trigger = tracker.last_double_click_trigger_time;

                                let dx = pt.0 - down_pt.0;
                                let dy = pt.1 - down_pt.1;
                                let drag_dist_sq = dx * dx + dy * dy;
                                let hold_ms = now.duration_since(down_time).as_millis();

                                let current_gen = INPUT_GENERATION.load(Ordering::SeqCst);
                                let no_intervening_input = current_gen == down_gen;
                                let recently_typed = now
                                    .duration_since(TYPING_TRACKER.lock().last_non_modifier_key_time)
                                    .as_millis()
                                    < TYPING_QUIET_WINDOW_MS;

                                // Validate geometric/window integrity before classifying gesture.
                                let same_window = up_hwnd != 0 && up_hwnd == down_hwnd;
                                let window_unmoved = rects_equal(down_win_rect, up_win_rect);
                                let cursor_allowed = !is_excluded_cursor(down_cursor)
                                    && !is_excluded_cursor(up_cursor);
                                let was_ibeam =
                                    is_ibeam_cursor(down_cursor) || is_ibeam_cursor(up_cursor);

                                // Track clean single clicks for accurate double-click detection
                                // (prevents drag-then-click or long-press-then-click false positives).
                                let is_clean_click = was_valid_down
                                    && drag_dist_sq <= DOUBLE_CLICK_MAX_DRIFT_SQ
                                    && hold_ms <= DOUBLE_CLICK_MAX_HOLD_MS
                                    && same_window
                                    && down_in_client;

                                // 1. Drag selection check
                                let is_drag = was_valid_down
                                    && same_window
                                    && window_unmoved
                                    && down_in_client
                                    && cursor_allowed
                                    && no_intervening_input
                                    && !recently_typed
                                    && drag_dist_sq >= DRAG_MIN_DIST_SQ
                                    && hold_ms >= DRAG_MIN_HOLD_MS
                                    && hold_ms <= DRAG_MAX_HOLD_MS;

                                // 2. Double-click word selection check
                                let is_double_click = if state.is_double_click_enabled()
                                    && is_clean_click
                                    && window_unmoved
                                    && cursor_allowed
                                    && no_intervening_input
                                    && !recently_typed
                                    && last_clean_hwnd == up_hwnd
                                    && now.duration_since(last_dbl_trigger).as_millis()
                                        > DOUBLE_CLICK_COOLDOWN_MS
                                {
                                    let dbl_time_limit = GetDoubleClickTime().max(200) as u128;
                                    let cx_dbl = GetSystemMetrics(SM_CXDOUBLECLK).max(4);
                                    let cy_dbl = GetSystemMetrics(SM_CYDOUBLECLK).max(4);
                                    let interval = now.duration_since(last_clean_time).as_millis();
                                    let click_dx = (pt.0 - last_clean_pt.0).abs();
                                    let click_dy = (pt.1 - last_clean_pt.1).abs();
                                    interval <= dbl_time_limit
                                        && click_dx <= cx_dbl
                                        && click_dy <= cy_dbl
                                } else {
                                    false
                                };

                                if is_clean_click {
                                    tracker.last_clean_click_pt = pt;
                                    tracker.last_clean_click_time = now;
                                    tracker.last_clean_click_hwnd = up_hwnd;
                                } else {
                                    tracker.last_clean_click_hwnd = 0;
                                }

                                let trigger_kind = if is_drag {
                                    Some(TriggerKind::Drag)
                                } else if is_double_click {
                                    Some(TriggerKind::DoubleClick)
                                } else {
                                    None
                                };

                                if let Some(kind) = trigger_kind {
                                    if now.duration_since(last_trigger).as_millis()
                                        > TRIGGER_DEBOUNCE_MS
                                        && !is_excluded_context(false)
                                    {
                                        tracker.last_trigger_time = now;
                                        if kind == TriggerKind::DoubleClick {
                                            tracker.last_double_click_trigger_time = now;
                                        }
                                        let req = CopyRequest {
                                            target_hwnd: up_hwnd,
                                            down_pt,
                                            up_pt: pt,
                                            kind,
                                            was_ibeam,
                                            expected_gen: current_gen,
                                            is_smart_replace: state.is_smart_replace_enabled(),
                                        };
                                        drop(tracker);
                                        std::thread::spawn(move || run_copy_task(req));
                                    }
                                } else {
                                    drop(tracker);
                                    // User clicked elsewhere to move cursor or deselect -> disarm smart replace.
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
                if state.is_enabled() {
                    let msg = w_param.0 as u32;
                    if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                        let kbd = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
                        let flags_raw: u32 = std::mem::transmute_copy(&kbd.flags);
                        let is_injected = (flags_raw & (LLKHF_INJECTED | LLKHF_LOWER_IL_INJECTED))
                            != 0
                            || kbd.dwExtraInfo == QOLLECT_INJECTED_EXTRA_INFO;

                        if !is_injected {
                            // Any real physical keystroke immediately cancels any pending copy task
                            // so rapid "select then type" workflows can NEVER get hit by a delayed Ctrl+C.
                            INPUT_GENERATION.fetch_add(1, Ordering::SeqCst);

                            let vk = kbd.vkCode;
                            let is_modifier = vk == VK_CONTROL.0 as u32
                                || vk == VK_LCONTROL.0 as u32
                                || vk == VK_RCONTROL.0 as u32
                                || vk == VK_MENU.0 as u32
                                || vk == VK_LMENU.0 as u32
                                || vk == VK_RMENU.0 as u32
                                || vk == VK_SHIFT.0 as u32
                                || vk == VK_LSHIFT.0 as u32
                                || vk == VK_RSHIFT.0 as u32
                                || vk == VK_LWIN.0 as u32
                                || vk == VK_RWIN.0 as u32;

                            if !is_modifier {
                                TYPING_TRACKER.lock().last_non_modifier_key_time = Instant::now();
                            }

                            if state.is_smart_replace_enabled() {
                                let is_ctrl = is_vk_down(VK_CONTROL)
                                    || is_vk_down(VK_LCONTROL)
                                    || is_vk_down(VK_RCONTROL);
                                let is_alt = is_vk_down(VK_MENU)
                                    || is_vk_down(VK_LMENU)
                                    || is_vk_down(VK_RMENU);
                                let is_win = is_vk_down(VK_LWIN) || is_vk_down(VK_RWIN);

                                // Detect Ctrl+V (0x56 is virtual key code for 'V')
                                if vk == 0x56 && is_ctrl && !is_alt && !is_win {
                                    let mut r_tracker = REPLACE_TRACKER.lock();
                                    if r_tracker.armed {
                                        let now = Instant::now();
                                        let cur_hwnd = GetForegroundWindow();
                                        if now.duration_since(r_tracker.trigger_time).as_secs() <= 5
                                            && cur_hwnd.0 as isize == r_tracker.target_hwnd
                                        {
                                            if let Some(ref text_to_restore) =
                                                r_tracker.preserved_text
                                            {
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
                                    // For non-modifier keys, if it's not backspace/delete, disarm replace tracker
                                    const VK_BACK_CODE: u32 = 0x08;
                                    const VK_DELETE_CODE: u32 = 0x2E;
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
        }

        CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
    }

    struct HookThreadHandle {
        thread_id: u32,
        join_handle: std::thread::JoinHandle<()>,
    }

    static HOOK_RUNTIME: LazyLock<Mutex<Option<HookThreadHandle>>> =
        LazyLock::new(|| Mutex::new(None));

    /// Install or uninstall WH_MOUSE_LL / WH_KEYBOARD_LL strictly on demand.
    /// When copy-on-select is disabled (default), no global keyboard/mouse hooks exist in the process.
    pub fn sync_hook_lifecycle(enabled: bool) {
        let mut runtime = HOOK_RUNTIME.lock();

        if enabled {
            if runtime.is_some() {
                return;
            }

            let (tx, rx) = std::sync::mpsc::channel::<Option<u32>>();
            let join_handle = std::thread::spawn(move || unsafe {
                // Force creation of the thread message queue before publishing thread_id
                // so PostThreadMessageW(WM_QUIT) can never race ahead of queue creation.
                let mut init_msg = MSG::default();
                let _ = PeekMessageW(&mut init_msg, HWND::default(), 0, 0, PM_NOREMOVE);

                let mouse_hook = SetWindowsHookExW(
                    WH_MOUSE_LL,
                    Some(mouse_hook_proc),
                    HINSTANCE::default(),
                    0,
                );
                let kbd_hook = SetWindowsHookExW(
                    WH_KEYBOARD_LL,
                    Some(keyboard_hook_proc),
                    HINSTANCE::default(),
                    0,
                );

                let mouse_ok = mouse_hook.is_ok();
                let kbd_ok = kbd_hook.is_ok();

                if mouse_ok || kbd_ok {
                    let tid = GetCurrentThreadId();
                    let _ = tx.send(Some(tid));
                    println!(
                        "[CopyOnSelect] Low-level hooks installed on demand (mouse: {}, keyboard: {}, tid: {})",
                        mouse_ok, kbd_ok, tid
                    );

                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, HWND::default(), 0, 0).0 > 0 {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }

                    if let Ok(h) = mouse_hook {
                        let _ = UnhookWindowsHookEx(h);
                    }
                    if let Ok(h) = kbd_hook {
                        let _ = UnhookWindowsHookEx(h);
                    }
                    println!("[CopyOnSelect] Low-level hooks uninstalled");
                } else {
                    let _ = tx.send(None);
                    eprintln!("[CopyOnSelect] Failed to install low-level hooks");
                }
            });

            match rx.recv() {
                Ok(Some(thread_id)) => {
                    *runtime = Some(HookThreadHandle {
                        thread_id,
                        join_handle,
                    });
                }
                _ => {
                    let _ = join_handle.join();
                }
            }
        } else if let Some(handle) = runtime.take() {
            CLICK_TRACKER.lock().down_valid = false;
            {
                let mut replace = REPLACE_TRACKER.lock();
                replace.armed = false;
                replace.preserved_text = None;
            }

            unsafe {
                let _ = PostThreadMessageW(handle.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            let _ = handle.join_handle.join();
        }
    }

    pub fn start_monitor(state: Arc<CopyOnSelectState>) {
        let enabled = state.is_enabled();
        *COPY_ON_SELECT_STATE.lock() = Some(state);

        if enabled {
            sync_hook_lifecycle(true);
        } else {
            println!("[CopyOnSelect] Disabled at startup; low-level hooks not installed");
        }
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

pub fn stop_copy_on_select_monitor() {
    #[cfg(target_os = "windows")]
    {
        windows_impl::sync_hook_lifecycle(false);
    }
}
