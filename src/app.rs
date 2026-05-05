use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateFontW, COLOR_WINDOW, FW_NORMAL, HBRUSH};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::SystemInformation::GetSystemTime;
use windows::Win32::UI::Controls::{
    BST_CHECKED, EM_SETREADONLY, WC_BUTTONW, WC_EDITW, WC_LISTBOXW,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;

const ID_LIST: i32 = 1001;
const ID_DETAILS: i32 = 1002;
const ID_SEARCH: i32 = 1003;
const ID_INCLUDE_ARCHIVED: i32 = 1004;
const ID_REFRESH: i32 = 1005;
const ID_OPEN: i32 = 1006;
const ID_NATIVE: i32 = 1007;
const ID_DOCTOR: i32 = 1008;
const ID_DETAIL: i32 = 1009;
const ID_PREVIEW_SYNC: i32 = 1010;
const ID_SYNC: i32 = 1011;
const ID_EXPORT: i32 = 1012;
const ID_COPY: i32 = 1013;
const ID_PROGRESS: i32 = 1014;
const ID_LANG: i32 = 1015;
const TIMER_PROGRESS: usize = 1;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lang {
    Zh,
    En,
}

impl Default for Lang {
    fn default() -> Self {
        Self::Zh
    }
}

impl Lang {
    fn toggled(self) -> Self {
        match self {
            Self::Zh => Self::En,
            Self::En => Self::Zh,
        }
    }
}

#[derive(Clone, Copy)]
enum UiText {
    Title,
    IncludeArchived,
    Refresh,
    OpenSessions,
    NativeResume,
    DiagnoseResume,
    ShowDetail,
    PreviewSync,
    SyncProvider,
    ExportRestore,
    CopyRestore,
    Language,
    Ready,
    LoadingSessions,
    RefreshingSessions,
    PreviewingSync,
    SyncingProvider,
    ExportingRestore,
    CopyingRestore,
    Working,
    SyncConfirm,
    SyncTitle,
    NoSessions,
    ErrorPrefix,
}

fn ui_text(lang: Lang, key: UiText) -> &'static str {
    match lang {
        Lang::Zh => match key {
            UiText::Title => "Codex 会话连续性（Windows）",
            UiText::IncludeArchived => "包含归档",
            UiText::Refresh => "刷新",
            UiText::OpenSessions => "打开会话文件夹",
            UiText::NativeResume => "原生恢复命令",
            UiText::DiagnoseResume => "/resume 风险诊断",
            UiText::ShowDetail => "查看详情",
            UiText::PreviewSync => "预览同步",
            UiText::SyncProvider => "同步到当前 Provider",
            UiText::ExportRestore => "导出恢复文件",
            UiText::CopyRestore => "复制恢复提示",
            UiText::Language => "English",
            UiText::Ready => "就绪",
            UiText::LoadingSessions => "正在加载会话...",
            UiText::RefreshingSessions => "正在刷新会话",
            UiText::PreviewingSync => "正在预览 Provider 同步",
            UiText::SyncingProvider => "正在同步 Provider 元数据",
            UiText::ExportingRestore => "正在导出恢复文件",
            UiText::CopyingRestore => "正在复制恢复提示",
            UiText::Working => "处理中",
            UiText::SyncConfirm => "将先备份，然后更新 Codex Provider 元数据。是否继续？",
            UiText::SyncTitle => "同步 Provider",
            UiText::NoSessions => "未找到本地会话。",
            UiText::ErrorPrefix => "错误",
        },
        Lang::En => match key {
            UiText::Title => "Codex Continuity for Windows",
            UiText::IncludeArchived => "Include archived",
            UiText::Refresh => "Refresh",
            UiText::OpenSessions => "Open sessions",
            UiText::NativeResume => "Native Resume Command",
            UiText::DiagnoseResume => "Diagnose /resume Risk",
            UiText::ShowDetail => "Show Detail",
            UiText::PreviewSync => "Preview Sync",
            UiText::SyncProvider => "Sync to Current Provider",
            UiText::ExportRestore => "Export Restore File",
            UiText::CopyRestore => "Copy Restore Prompt",
            UiText::Language => "中文",
            UiText::Ready => "Ready",
            UiText::LoadingSessions => "Loading sessions...",
            UiText::RefreshingSessions => "Refreshing sessions",
            UiText::PreviewingSync => "Previewing provider sync",
            UiText::SyncingProvider => "Syncing provider metadata",
            UiText::ExportingRestore => "Exporting restore file",
            UiText::CopyingRestore => "Copying restore prompt",
            UiText::Working => "Working",
            UiText::SyncConfirm => "This will backup and update Codex provider metadata. Continue?",
            UiText::SyncTitle => "Sync Provider",
            UiText::NoSessions => "No local sessions found.",
            UiText::ErrorPrefix => "error",
        },
    }
}

fn current_lang() -> Lang {
    STATE.with(|cell| cell.borrow().as_ref().map(|s| s.lang).unwrap_or_default())
}

fn t(key: UiText) -> &'static str {
    ui_text(current_lang(), key)
}

fn unknown(lang: Lang) -> &'static str {
    match lang {
        Lang::Zh => "未知",
        Lang::En => "unknown",
    }
}

fn unnamed(lang: Lang) -> &'static str {
    match lang {
        Lang::Zh => "未命名",
        Lang::En => "unnamed",
    }
}

fn unknown_cwd(lang: Lang) -> &'static str {
    match lang {
        Lang::Zh => "未知目录",
        Lang::En => "unknown cwd",
    }
}

fn yes_no(lang: Lang, value: bool) -> &'static str {
    match (lang, value) {
        (Lang::Zh, true) => "是",
        (Lang::Zh, false) => "否",
        (Lang::En, true) => "true",
        (Lang::En, false) => "false",
    }
}

fn error_text(error: impl std::fmt::Display) -> String {
    format!("{}: {error}", t(UiText::ErrorPrefix))
}

#[derive(Clone, Default)]
struct Message {
    timestamp: String,
    role: String,
    text: String,
}

#[derive(Clone, Default)]
struct Session {
    id: String,
    path: PathBuf,
    first_timestamp: String,
    last_timestamp: String,
    cwd: String,
    provider: String,
    model: String,
    source: String,
    thread_name: String,
    messages: Vec<Message>,
    errors: Vec<String>,
    event_types: Vec<String>,
}

impl Session {
    fn user_turns(&self) -> usize {
        self.messages.iter().filter(|m| m.role == "user").count()
    }
    fn assistant_turns(&self) -> usize {
        self.messages
            .iter()
            .filter(|m| m.role == "assistant")
            .count()
    }
    fn aborted_turns(&self) -> usize {
        self.event_types
            .iter()
            .filter(|e| *e == "turn_aborted")
            .count()
    }
    fn rolled_back_turns(&self) -> usize {
        self.event_types
            .iter()
            .filter(|e| *e == "thread_rolled_back")
            .count()
    }
}

#[derive(Default)]
struct ConfigSummary {
    values: HashMap<String, String>,
    providers: HashSet<String>,
    path: PathBuf,
}

struct State {
    list: HWND,
    details: HWND,
    search: HWND,
    include_archived: HWND,
    refresh: HWND,
    open: HWND,
    native: HWND,
    doctor: HWND,
    detail: HWND,
    preview_sync: HWND,
    sync: HWND,
    export: HWND,
    copy: HWND,
    language: HWND,
    progress: HWND,
    progress_step: usize,
    busy_label: String,
    lang: Lang,
    sessions: Vec<Session>,
    filtered_indices: Vec<usize>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = RefCell::new(None);
}

pub fn run() -> windows::core::Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
    }
    let hmodule = unsafe { GetModuleHandleW(None)? };
    let class_name = w!("CodexContinuityWindowsClass");
    let wc = WNDCLASSW {
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        hInstance: HINSTANCE(hmodule.0),
        lpszClassName: class_name,
        lpfnWndProc: Some(wndproc),
        hbrBackground: HBRUSH((COLOR_WINDOW.0 as isize + 1) as *mut core::ffi::c_void),
        ..Default::default()
    };
    unsafe {
        RegisterClassW(&wc);
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("Codex Continuity for Windows"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1180,
            780,
            None,
            None,
            HINSTANCE(hmodule.0),
            None,
        )?
    };
    if hwnd.0.is_null() {
        return Ok(());
    }
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                create_ui(hwnd);
                LRESULT(0)
            }
            WM_SIZE => {
                layout(hwnd);
                LRESULT(0)
            }
            WM_COMMAND => {
                handle_command(hwnd, wparam);
                LRESULT(0)
            }
            WM_TIMER => {
                tick_progress(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe fn create_ui(hwnd: HWND) {
    let lang = Lang::default();
    let mono = CreateFontW(
        16,
        0,
        0,
        0,
        FW_NORMAL.0 as i32,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        w!("Consolas"),
    );
    let search = child(
        WC_EDITW,
        "",
        style(WS_BORDER | WS_VISIBLE | WS_CHILD, ES_AUTOHSCROLL),
        12,
        46,
        390,
        26,
        hwnd,
        ID_SEARCH,
    );
    let list = child(
        WC_LISTBOXW,
        "",
        style2(
            WS_BORDER | WS_VISIBLE | WS_CHILD | WS_VSCROLL,
            LBS_NOTIFY,
            LBS_NOINTEGRALHEIGHT,
        ),
        12,
        82,
        390,
        620,
        hwnd,
        ID_LIST,
    );
    let include_archived = child(
        WC_BUTTONW,
        ui_text(lang, UiText::IncludeArchived),
        style(WS_VISIBLE | WS_CHILD, BS_AUTOCHECKBOX),
        420,
        46,
        150,
        28,
        hwnd,
        ID_INCLUDE_ARCHIVED,
    );
    let refresh = child(
        WC_BUTTONW,
        ui_text(lang, UiText::Refresh),
        style(WS_VISIBLE | WS_CHILD, BS_PUSHBUTTON),
        578,
        44,
        90,
        30,
        hwnd,
        ID_REFRESH,
    );
    let open = child(
        WC_BUTTONW,
        ui_text(lang, UiText::OpenSessions),
        style(WS_VISIBLE | WS_CHILD, BS_PUSHBUTTON),
        676,
        44,
        150,
        30,
        hwnd,
        ID_OPEN,
    );
    let language = child(
        WC_BUTTONW,
        ui_text(lang, UiText::Language),
        style(WS_VISIBLE | WS_CHILD, BS_PUSHBUTTON),
        834,
        44,
        96,
        30,
        hwnd,
        ID_LANG,
    );
    let native = child(
        WC_BUTTONW,
        ui_text(lang, UiText::NativeResume),
        style(WS_VISIBLE | WS_CHILD, BS_PUSHBUTTON),
        420,
        84,
        190,
        32,
        hwnd,
        ID_NATIVE,
    );
    let doctor = child(
        WC_BUTTONW,
        ui_text(lang, UiText::DiagnoseResume),
        style(WS_VISIBLE | WS_CHILD, BS_PUSHBUTTON),
        618,
        84,
        190,
        32,
        hwnd,
        ID_DOCTOR,
    );
    let detail = child(
        WC_BUTTONW,
        ui_text(lang, UiText::ShowDetail),
        style(WS_VISIBLE | WS_CHILD, BS_PUSHBUTTON),
        816,
        84,
        110,
        32,
        hwnd,
        ID_DETAIL,
    );
    let preview_sync = child(
        WC_BUTTONW,
        ui_text(lang, UiText::PreviewSync),
        style(WS_VISIBLE | WS_CHILD, BS_PUSHBUTTON),
        934,
        84,
        120,
        32,
        hwnd,
        ID_PREVIEW_SYNC,
    );
    let sync = child(
        WC_BUTTONW,
        ui_text(lang, UiText::SyncProvider),
        style(WS_VISIBLE | WS_CHILD, BS_PUSHBUTTON),
        420,
        122,
        190,
        32,
        hwnd,
        ID_SYNC,
    );
    let export = child(
        WC_BUTTONW,
        ui_text(lang, UiText::ExportRestore),
        style(WS_VISIBLE | WS_CHILD, BS_PUSHBUTTON),
        618,
        122,
        160,
        32,
        hwnd,
        ID_EXPORT,
    );
    let copy = child(
        WC_BUTTONW,
        ui_text(lang, UiText::CopyRestore),
        style(WS_VISIBLE | WS_CHILD, BS_PUSHBUTTON),
        786,
        122,
        170,
        32,
        hwnd,
        ID_COPY,
    );
    let progress = child(
        WC_EDITW,
        ui_text(lang, UiText::Ready),
        style(WS_BORDER | WS_VISIBLE | WS_CHILD, ES_READONLY),
        964,
        122,
        202,
        32,
        hwnd,
        ID_PROGRESS,
    );
    let details = child(
        WC_EDITW,
        "",
        style3(
            WS_BORDER | WS_VISIBLE | WS_CHILD | WS_VSCROLL | WS_HSCROLL,
            ES_MULTILINE,
            ES_AUTOVSCROLL,
            ES_AUTOHSCROLL,
        ),
        420,
        164,
        730,
        538,
        hwnd,
        ID_DETAILS,
    );
    SendMessageW(details, WM_SETFONT, WPARAM(mono.0 as usize), LPARAM(1));
    SendMessageW(list, WM_SETFONT, WPARAM(mono.0 as usize), LPARAM(1));
    SendMessageW(details, EM_SETREADONLY, WPARAM(1), LPARAM(0));
    STATE.with(|s| {
        *s.borrow_mut() = Some(State {
            list,
            details,
            search,
            include_archived,
            refresh,
            open,
            native,
            doctor,
            detail,
            preview_sync,
            sync,
            export,
            copy,
            language,
            progress,
            progress_step: 0,
            busy_label: String::new(),
            lang,
            sessions: vec![],
            filtered_indices: vec![],
        })
    });
    apply_language(hwnd);
    set_details(ui_text(lang, UiText::LoadingSessions));
    refresh_sessions();
}

fn style(base: WINDOW_STYLE, extra1: i32) -> WINDOW_STYLE {
    WINDOW_STYLE(base.0 | extra1 as u32)
}

fn style2(base: WINDOW_STYLE, extra1: i32, extra2: i32) -> WINDOW_STYLE {
    WINDOW_STYLE(base.0 | extra1 as u32 | extra2 as u32)
}

fn style3(base: WINDOW_STYLE, extra1: i32, extra2: i32, extra3: i32) -> WINDOW_STYLE {
    WINDOW_STYLE(base.0 | extra1 as u32 | extra2 as u32 | extra3 as u32)
}

unsafe fn child(
    class: PCWSTR,
    text: &str,
    style: WINDOW_STYLE,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    parent: HWND,
    id: i32,
) -> HWND {
    let textw = wide(text);
    CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        class,
        PCWSTR(textw.as_ptr()),
        style,
        x,
        y,
        width,
        height,
        parent,
        HMENU(id as isize as *mut core::ffi::c_void),
        HINSTANCE(GetModuleHandleW(None).unwrap().0),
        None,
    )
    .unwrap_or_default()
}

unsafe fn layout(hwnd: HWND) {
    let mut r = RECT::default();
    let _ = GetClientRect(hwnd, &mut r);
    let width = r.right - r.left;
    let height = r.bottom - r.top;
    let right_width = (width - 438).max(260);
    let progress_width = (width - 982).max(180);
    STATE.with(|cell| {
        if let Some(st) = cell.borrow().as_ref() {
            let _ = MoveWindow(st.search, 12, 46, 390, 26, true);
            let _ = MoveWindow(st.list, 12, 82, 390, height - 96, true);
            let _ = MoveWindow(st.include_archived, 420, 46, 150, 28, true);
            let _ = MoveWindow(st.refresh, 578, 44, 90, 30, true);
            let _ = MoveWindow(st.open, 676, 44, 150, 30, true);
            let _ = MoveWindow(st.language, 834, 44, 96, 30, true);
            let _ = MoveWindow(st.native, 420, 84, 190, 32, true);
            let _ = MoveWindow(st.doctor, 618, 84, 190, 32, true);
            let _ = MoveWindow(st.detail, 816, 84, 110, 32, true);
            let _ = MoveWindow(st.preview_sync, 934, 84, 120, 32, true);
            let _ = MoveWindow(st.sync, 420, 122, 190, 32, true);
            let _ = MoveWindow(st.export, 618, 122, 160, 32, true);
            let _ = MoveWindow(st.copy, 786, 122, 170, 32, true);
            let _ = MoveWindow(st.progress, 964, 122, progress_width, 32, true);
            let _ = MoveWindow(st.details, 420, 164, right_width, height - 178, true);
        }
    });
}

fn apply_language(hwnd: HWND) {
    STATE.with(|cell| {
        if let Some(st) = cell.borrow().as_ref() {
            let lang = st.lang;
            set_text(hwnd, ui_text(lang, UiText::Title));
            set_text(st.include_archived, ui_text(lang, UiText::IncludeArchived));
            set_text(st.refresh, ui_text(lang, UiText::Refresh));
            set_text(st.open, ui_text(lang, UiText::OpenSessions));
            set_text(st.native, ui_text(lang, UiText::NativeResume));
            set_text(st.doctor, ui_text(lang, UiText::DiagnoseResume));
            set_text(st.detail, ui_text(lang, UiText::ShowDetail));
            set_text(st.preview_sync, ui_text(lang, UiText::PreviewSync));
            set_text(st.sync, ui_text(lang, UiText::SyncProvider));
            set_text(st.export, ui_text(lang, UiText::ExportRestore));
            set_text(st.copy, ui_text(lang, UiText::CopyRestore));
            set_text(st.language, ui_text(lang, UiText::Language));
            if st.progress_step == 0 || st.progress_step == 100 {
                set_text(st.progress, ui_text(lang, UiText::Ready));
            }
        }
    });
}

fn toggle_language(hwnd: HWND) {
    STATE.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.lang = st.lang.toggled();
            st.progress_step = 100;
            st.busy_label.clear();
        }
    });
    apply_language(hwnd);
    preview_selected();
}

fn handle_command(hwnd: HWND, wparam: WPARAM) {
    let id = (wparam.0 & 0xffff) as i32;
    let code = ((wparam.0 >> 16) & 0xffff) as u16;
    match id {
        ID_REFRESH => with_busy(hwnd, t(UiText::RefreshingSessions), refresh_sessions),
        ID_INCLUDE_ARCHIVED => with_busy(hwnd, t(UiText::RefreshingSessions), refresh_sessions),
        ID_LANG => toggle_language(hwnd),
        ID_OPEN => open_sessions_folder(),
        ID_LIST if code == LBN_SELCHANGE as u16 => preview_selected(),
        ID_SEARCH if code == EN_CHANGE as u16 => apply_filter(),
        ID_DETAIL => {
            if let Some(s) = selected_session() {
                set_details(&render_detail(&s, current_lang()));
            }
        }
        ID_NATIVE => {
            if let Some(s) = selected_session() {
                set_details(&render_native(&s, current_lang()));
            }
        }
        ID_DOCTOR => {
            if let Some(s) = selected_session() {
                set_details(&render_doctor(&s, current_lang()));
            }
        }
        ID_PREVIEW_SYNC => with_busy(hwnd, t(UiText::PreviewingSync), || {
            match render_sync(true, current_lang()) {
                Ok(t) => set_details(&t),
                Err(e) => set_details(&error_text(e)),
            }
        }),
        ID_SYNC => unsafe {
            let msg = wide(t(UiText::SyncConfirm));
            let title = wide(t(UiText::SyncTitle));
            let answer = MessageBoxW(
                hwnd,
                PCWSTR(msg.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_OKCANCEL | MB_ICONWARNING,
            );
            if answer == IDOK {
                with_busy(hwnd, t(UiText::SyncingProvider), || {
                    match render_sync(false, current_lang()) {
                        Ok(t) => {
                            set_details(&t);
                            refresh_sessions();
                        }
                        Err(e) => set_details(&error_text(e)),
                    }
                })
            }
        },
        ID_EXPORT => with_busy(hwnd, t(UiText::ExportingRestore), export_restore),
        ID_COPY => with_busy(hwnd, t(UiText::CopyingRestore), copy_restore),
        _ => {}
    }
}

fn with_busy<F: FnOnce()>(hwnd: HWND, label: &str, action: F) {
    start_progress(hwnd, label);
    pump_ui();
    action();
    stop_progress(hwnd, t(UiText::Ready));
    pump_ui();
}

fn start_progress(hwnd: HWND, label: &str) {
    STATE.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.progress_step = 0;
            st.busy_label = label.to_string();
            set_text(st.progress, &format!("{}  0%", label));
        }
    });
    unsafe {
        let _ = SetTimer(hwnd, TIMER_PROGRESS, 120, None);
    }
}

fn pump_ui() {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn stop_progress(hwnd: HWND, label: &str) {
    unsafe {
        let _ = KillTimer(hwnd, TIMER_PROGRESS);
    }
    STATE.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.progress_step = 100;
            st.busy_label.clear();
            set_text(st.progress, label);
        }
    });
}

fn tick_progress(_hwnd: HWND) {
    STATE.with(|cell| {
        if let Some(st) = cell.borrow_mut().as_mut() {
            st.progress_step = (st.progress_step + 7).min(95);
            let bars = (st.progress_step / 10).min(10);
            let bar = format!("{}{}", "█".repeat(bars), "░".repeat(10 - bars));
            let label = if st.busy_label.is_empty() {
                ui_text(st.lang, UiText::Working)
            } else {
                &st.busy_label
            };
            set_text(
                st.progress,
                &format!("{} [{}] {}%", label, bar, st.progress_step),
            );
        }
    });
}

fn refresh_sessions() {
    let include = STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|s| unsafe {
                SendMessageW(s.include_archived, BM_GETCHECK, WPARAM(0), LPARAM(0)).0
                    == BST_CHECKED.0 as isize
            })
            .unwrap_or(false)
    });
    match load_sessions(include) {
        Ok(sessions) => {
            STATE.with(|cell| {
                if let Some(st) = cell.borrow_mut().as_mut() {
                    st.sessions = sessions;
                }
            });
            apply_filter();
            if selected_session().is_none() {
                set_details(t(UiText::NoSessions));
            }
        }
        Err(e) => set_details(&error_text(e)),
    }
}

fn apply_filter() {
    let search_hwnd = STATE.with(|c| c.borrow().as_ref().unwrap().search);
    let needle = get_text(search_hwnd).to_lowercase();
    let lang = current_lang();
    STATE.with(|cell| unsafe {
        if let Some(st) = cell.borrow_mut().as_mut() {
            SendMessageW(st.list, LB_RESETCONTENT, WPARAM(0), LPARAM(0));
            st.filtered_indices.clear();
            for (idx, sess) in st.sessions.iter().enumerate() {
                let hay = format!(
                    "{} {} {} {}",
                    sess.id, sess.thread_name, sess.provider, sess.cwd
                )
                .to_lowercase();
                if needle.is_empty() || hay.contains(&needle) {
                    st.filtered_indices.push(idx);
                    let item = format!(
                        "{}  {}  {} {} | {} | {}",
                        compact_time(&sess.last_timestamp),
                        if sess.provider.is_empty() {
                            unknown(lang)
                        } else {
                            &sess.provider
                        },
                        match lang {
                            Lang::Zh => "轮次",
                            Lang::En => "turns",
                        },
                        sess.user_turns(),
                        if sess.thread_name.is_empty() {
                            unnamed(lang)
                        } else {
                            &sess.thread_name
                        },
                        sess.id
                    );
                    let w = wide(&item);
                    SendMessageW(
                        st.list,
                        LB_ADDSTRING,
                        WPARAM(0),
                        LPARAM(w.as_ptr() as isize),
                    );
                }
            }
            if !st.filtered_indices.is_empty() {
                SendMessageW(st.list, LB_SETCURSEL, WPARAM(0), LPARAM(0));
            }
        }
    });
    preview_selected();
}

fn selected_session() -> Option<Session> {
    STATE.with(|cell| unsafe {
        let st = cell.borrow();
        let st = st.as_ref()?;
        let sel = SendMessageW(st.list, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
        if sel < 0 {
            return None;
        }
        let idx = *st.filtered_indices.get(sel as usize)?;
        st.sessions.get(idx).cloned()
    })
}

fn preview_selected() {
    if let Some(s) = selected_session() {
        let lang = current_lang();
        match lang {
            Lang::Zh => set_details(&format!(
                "已选择会话\r\n\r\nid:       {}\r\n更新时间: {}\r\nprovider: {}\r\n轮次:     {}\r\n上下文:   {} | {}\r\n\r\n可点击“查看详情”、“/resume 风险诊断”、“原生恢复命令”或 Provider 同步相关操作。",
                s.id,
                compact_time(&s.last_timestamp),
                if s.provider.is_empty() { unknown(lang) } else { &s.provider },
                s.user_turns(),
                if s.thread_name.is_empty() { unnamed(lang) } else { &s.thread_name },
                if s.cwd.is_empty() { unknown_cwd(lang) } else { &s.cwd }
            )),
            Lang::En => set_details(&format!(
                "Selected session\r\n\r\nid:       {}\r\nupdated:  {}\r\nprovider: {}\r\nturns:    {}\r\ncontext:  {} | {}\r\n\r\nClick Show Detail, Diagnose /resume Risk, Native Resume Command, or Provider Sync actions.",
                s.id,
                compact_time(&s.last_timestamp),
                if s.provider.is_empty() { unknown(lang) } else { &s.provider },
                s.user_turns(),
                if s.thread_name.is_empty() { unnamed(lang) } else { &s.thread_name },
                if s.cwd.is_empty() { unknown_cwd(lang) } else { &s.cwd }
            )),
        }
    }
}

fn load_sessions(include_archived: bool) -> Result<Vec<Session>, String> {
    let home = codex_home();
    let names = thread_names(&home);
    let mut out = vec![];
    let mut seen = HashSet::new();
    for path in session_paths(&home, include_archived) {
        let mut s = parse_session(&path);
        if s.id.is_empty() {
            continue;
        }
        if let Some(name) = names.get(&s.id) {
            s.thread_name = name.clone();
        }
        let p = path.to_string_lossy();
        let archive = p.contains("\\archived_sessions\\") || p.contains("\\restore-backup-");
        if archive && seen.contains(&s.id) {
            continue;
        }
        if !archive {
            seen.insert(s.id.clone());
        }
        out.push(s);
    }
    out.sort_by_key(|s| {
        if s.first_timestamp.is_empty() {
            s.path.to_string_lossy().to_string()
        } else {
            s.first_timestamp.clone()
        }
    });
    Ok(out)
}

fn codex_home() -> PathBuf {
    dirs_home().join(".codex")
}
fn dirs_home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn session_paths(home: &Path, include_archived: bool) -> Vec<PathBuf> {
    let mut out = vec![];
    let sessions = home.join("sessions");
    collect_session_files(&sessions, &mut out);
    if include_archived {
        let archived = home.join("archived_sessions");
        if archived.exists() {
            if let Ok(rd) = fs::read_dir(&archived) {
                for e in rd.flatten() {
                    let p = e.path();
                    let n = e.file_name().to_string_lossy().to_string();
                    if p.is_file() && n.starts_with("rollout-") && n.ends_with(".jsonl") {
                        out.push(p);
                    }
                }
            }
        }
        if let Ok(rd) = fs::read_dir(home) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir()
                    && e.file_name()
                        .to_string_lossy()
                        .starts_with("restore-backup-")
                {
                    if let Ok(files) = fs::read_dir(p) {
                        for f in files.flatten() {
                            let fp = f.path();
                            let n = f.file_name().to_string_lossy().to_string();
                            if fp.is_file() && n.starts_with("rollout-") && n.ends_with(".jsonl") {
                                out.push(fp);
                            }
                        }
                    }
                }
            }
        }
    }
    out.sort();
    out
}

fn collect_session_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_session_files(&p, out);
            continue;
        }
        let n = e.file_name().to_string_lossy().to_string();
        if p.is_file() && n.starts_with("rollout-") && n.ends_with(".jsonl") {
            out.push(p);
        }
    }
}

fn thread_names(home: &Path) -> HashMap<String, String> {
    let mut names = HashMap::new();
    let Ok(file) = fs::File::open(home.join("session_index.jsonl")) else {
        return names;
    };
    for line in BufReader::new(file).lines().flatten() {
        if let (Some(id), Some(name)) = (
            json_get_string(&line, "id"),
            json_get_string(&line, "thread_name"),
        ) {
            names.insert(id, name);
        }
    }
    names
}

fn parse_session(path: &Path) -> Session {
    let mut s = Session {
        path: path.to_path_buf(),
        ..Default::default()
    };
    let Ok(file) = fs::File::open(path) else {
        if s.id.is_empty() {
            s.id = session_id(path);
        }
        return s;
    };

    for line in BufReader::new(file).lines().flatten() {
        let ts = json_get_string(&line, "timestamp").unwrap_or_default();
        if !ts.is_empty() {
            if s.first_timestamp.is_empty() {
                s.first_timestamp = ts.clone();
            }
            s.last_timestamp = ts.clone();
        }
        let typ = json_get_string(&line, "type").unwrap_or_default();
        match typ.as_str() {
            "session_meta" => {
                s.id = json_payload_string(&line, "id").unwrap_or(s.id);
                s.cwd = json_payload_string(&line, "cwd").unwrap_or(s.cwd);
                s.provider = json_payload_string(&line, "model_provider").unwrap_or(s.provider);
                s.model = json_payload_string(&line, "model").unwrap_or(s.model);
                s.source = json_payload_string(&line, "source")
                    .or_else(|| json_payload_string(&line, "originator"))
                    .unwrap_or(s.source);
            }
            "turn_context" => {
                s.cwd = json_payload_string(&line, "cwd").unwrap_or(s.cwd);
                s.model = json_payload_string(&line, "model").unwrap_or(s.model);
            }
            "event_msg" => {
                let event_type = json_payload_string(&line, "type").unwrap_or_default();
                if !event_type.is_empty() {
                    s.event_types.push(event_type.clone());
                }
                if event_type == "error" {
                    if let Some(msg) = json_payload_string(&line, "message") {
                        s.errors.push(sanitize(&msg));
                    }
                }
                if event_type == "user_message" {
                    if let Some(msg) = json_payload_string(&line, "message") {
                        let text = sanitize(&msg);
                        if !text.is_empty() && !noise(&text) {
                            s.messages.push(Message {
                                timestamp: ts.clone(),
                                role: "user".into(),
                                text,
                            });
                        }
                    }
                }
            }
            "response_item" => {
                if json_payload_string(&line, "type").as_deref() == Some("message") {
                    if let Some(role) = json_payload_string(&line, "role") {
                        if ["user", "assistant", "tool", "function"].contains(&role.as_str()) {
                            let text = sanitize(&json_content_text(&line));
                            if !text.is_empty() && !noise(&text) {
                                if !(role == "user"
                                    && s.messages
                                        .iter()
                                        .rev()
                                        .take(3)
                                        .any(|m| m.role == "user" && m.text == text))
                                {
                                    s.messages.push(Message {
                                        timestamp: ts.clone(),
                                        role,
                                        text,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if s.id.is_empty() {
        s.id = session_id(path);
    }
    s
}

fn json_payload_string(line: &str, key: &str) -> Option<String> {
    let payload_start = line.find("\"payload\"")?;
    json_get_string(&line[payload_start..], key)
}

fn json_content_text(line: &str) -> String {
    let mut out = Vec::new();
    for key in ["text", "content", "output"] {
        if let Some(v) = json_get_string(line, key) {
            out.push(v);
        }
    }
    out.join("\n")
}

fn json_get_string(src: &str, key: &str) -> Option<String> {
    let marker = format!("\"{}\"", key);
    let mut rest = src;
    loop {
        let pos = rest.find(&marker)?;
        let after = &rest[pos + marker.len()..];
        let after = after.trim_start();
        if !after.starts_with(':') {
            rest = &after[1.min(after.len())..];
            continue;
        }
        let value = after[1..].trim_start();
        return parse_json_value_as_string(value);
    }
}

fn parse_json_value_as_string(value: &str) -> Option<String> {
    if let Some(stripped) = value.strip_prefix('"') {
        return Some(parse_json_string(stripped));
    }
    let end = value.find([',', '}', ']']).unwrap_or(value.len());
    let raw = value[..end].trim();
    if raw.is_empty() || raw == "null" {
        None
    } else {
        Some(raw.to_string())
    }
}

fn parse_json_string(mut s: &str) -> String {
    let mut out = String::new();
    while !s.is_empty() {
        let mut chars = s.chars();
        let ch = chars.next().unwrap();
        s = chars.as_str();
        match ch {
            '"' => break,
            '\\' => {
                let mut escaped = s.chars();
                let Some(e) = escaped.next() else {
                    break;
                };
                s = escaped.as_str();
                match e {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    '/' => out.push('/'),
                    'b' => out.push('\u{0008}'),
                    'f' => out.push('\u{000c}'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    'u' => {
                        if s.len() >= 4 {
                            let hex = &s[..4];
                            if let Ok(v) = u16::from_str_radix(hex, 16) {
                                if let Some(c) = char::from_u32(v as u32) {
                                    out.push(c);
                                }
                            }
                            s = &s[4..];
                        }
                    }
                    other => out.push(other),
                }
            }
            other => out.push(other),
        }
    }
    out
}
fn sanitize(text: &str) -> String {
    let text = text.replace('\0', "");
    text.split_whitespace()
        .map(redact_word)
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .into()
}

fn redact_word(word: &str) -> String {
    let lower = word.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    if bytes.len() >= 3
        && bytes[0] == b's'
        && bytes[1] == b'k'
        && bytes[2] == b'-'
        && word.len() >= 16
    {
        return "<redacted>".into();
    }
    for marker in [
        "api_key", "apikey", "api-key", "token", "password", "secret", "bearer",
    ] {
        if lower.starts_with(marker) && (word.contains('=') || word.contains(':')) {
            let prefix = word
                .find(['=', ':'])
                .map(|idx| &word[..idx])
                .unwrap_or(marker);
            return format!("{prefix}=<redacted>");
        }
    }
    word.into()
}
fn noise(text: &str) -> bool {
    let t = text.trim();
    t.starts_with("<environment_context>") || t.starts_with("# AGENTS.md instructions")
}
fn session_id(path: &Path) -> String {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    if let Some(rest) = name.strip_prefix("rollout-") {
        if let Some(rest) = rest.strip_suffix(".jsonl") {
            let mut parts = rest.splitn(3, '-');
            let _date = parts.next();
            let _time = parts.next();
            if let Some(id) = parts.next() {
                return id.to_string();
            }
        }
    }
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn read_config() -> ConfigSummary {
    let path = codex_home().join("config.toml");
    let mut c = ConfigSummary {
        path: path.clone(),
        ..Default::default()
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return c;
    };
    let mut current = String::new();
    let mut in_current = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("model_provider") {
            let v = toml_value(line);
            current = v.clone();
            c.values.insert("model_provider".into(), v);
            continue;
        }
        if line.starts_with("disable_response_storage") {
            c.values
                .insert("disable_response_storage".into(), toml_value(line));
            continue;
        }
        if let Some(name) = line
            .strip_prefix("[model_providers.")
            .and_then(|x| x.strip_suffix(']'))
        {
            let name = name.trim_matches('"').to_string();
            in_current = name == current;
            c.providers.insert(name);
            continue;
        }
        if in_current && line.starts_with("base_url") {
            c.values.insert("base_url".into(), toml_value(line));
        }
        if in_current && line.starts_with("wire_api") {
            c.values.insert("wire_api".into(), toml_value(line));
        }
    }
    c
}
fn toml_value(line: &str) -> String {
    let mut v = line
        .split_once('=')
        .map(|x| x.1.trim().to_string())
        .unwrap_or_default();
    if v.starts_with('"') && v.ends_with('"') && v.len() >= 2 {
        v = v[1..v.len() - 1].to_string();
    }
    v
}
fn quote_toml(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}
fn compact_time(v: &str) -> String {
    if v.is_empty() {
        "-".into()
    } else {
        v.chars().take(19).collect::<String>().replace('T', " ")
    }
}

fn timestamp_compact() -> String {
    let t = unsafe { GetSystemTime() };
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    )
}
fn one_line(t: &str, limit: usize) -> String {
    let c = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if c.len() <= limit {
        c
    } else {
        format!(
            "{}…",
            c.chars()
                .take(limit.saturating_sub(1))
                .collect::<String>()
                .trim_end()
        )
    }
}

fn render_detail(s: &Session, lang: Lang) -> String {
    let mut out = match lang {
        Lang::Zh => format!(
            "id:        {}\r\n名称:      {}\r\n路径:      {}\r\n时间:      {} -> {}\r\n目录:      {}\r\nprovider:  {}\r\nmodel:     {}\r\n来源:      {}\r\n轮次:      user={} assistant={}\r\n事件:      aborted={} rolled_back={}\r\n",
            s.id,
            if s.thread_name.is_empty() { unnamed(lang) } else { &s.thread_name },
            s.path.display(),
            s.first_timestamp,
            s.last_timestamp,
            if s.cwd.is_empty() { unknown(lang) } else { &s.cwd },
            if s.provider.is_empty() { unknown(lang) } else { &s.provider },
            if s.model.is_empty() { unknown(lang) } else { &s.model },
            if s.source.is_empty() { unknown(lang) } else { &s.source },
            s.user_turns(),
            s.assistant_turns(),
            s.aborted_turns(),
            s.rolled_back_turns()
        ),
        Lang::En => format!(
            "id:        {}\r\nname:      {}\r\npath:      {}\r\ntime:      {} -> {}\r\ncwd:       {}\r\nprovider:  {}\r\nmodel:     {}\r\nsource:    {}\r\nturns:     user={} assistant={}\r\nevents:    aborted={} rolled_back={}\r\n",
            s.id,
            if s.thread_name.is_empty() { unnamed(lang) } else { &s.thread_name },
            s.path.display(),
            s.first_timestamp,
            s.last_timestamp,
            if s.cwd.is_empty() { unknown(lang) } else { &s.cwd },
            if s.provider.is_empty() { unknown(lang) } else { &s.provider },
            if s.model.is_empty() { unknown(lang) } else { &s.model },
            if s.source.is_empty() { unknown(lang) } else { &s.source },
            s.user_turns(),
            s.assistant_turns(),
            s.aborted_turns(),
            s.rolled_back_turns()
        ),
    };
    if !s.errors.is_empty() {
        out.push_str(match lang {
            Lang::Zh => "错误:\r\n",
            Lang::En => "errors:\r\n",
        });
        for e in s.errors.iter().take(5) {
            out.push_str(&format!("  - {}\r\n", one_line(e, 180)));
        }
    }
    let users = s
        .messages
        .iter()
        .filter(|m| m.role == "user")
        .collect::<Vec<_>>();
    if !users.is_empty() {
        out.push_str(match lang {
            Lang::Zh => "最近用户请求:\r\n",
            Lang::En => "recent user prompts:\r\n",
        });
        for m in users.iter().rev().take(20).rev() {
            out.push_str(&format!(
                "  - {} {}\r\n",
                compact_time(&m.timestamp),
                one_line(&m.text, 220)
            ));
        }
    }
    out
}

fn render_restore(s: &Session, lang: Lang) -> String {
    let latest = s.messages.iter().rev().find(|m| m.role == "user");
    let mut out = match lang {
        Lang::Zh => format!(
            "# Codex 本地会话恢复\n\n请恢复这个本地 Codex 会话并继续任务。\n\n## 会话\n\n- 会话 id: `{}`\n- 会话名称: `{}`\n- 原始 cwd: `{}`\n- 原始 provider: `{}`\n- 时间范围: `{}` -> `{}`\n- 本地 JSONL: `{}`\n\n## 恢复要求\n\n1. 先读取 `本地 JSONL`；不要只依赖服务端 `/resume`。\n2. 总结最新目标、约束、涉及文件、验证状态和剩余工作。\n3. 将最后一条真实用户请求视为当前任务。\n",
            s.id,
            if s.thread_name.is_empty() { &s.id } else { &s.thread_name },
            if s.cwd.is_empty() { unknown(lang) } else { &s.cwd },
            if s.provider.is_empty() { unknown(lang) } else { &s.provider },
            s.first_timestamp,
            s.last_timestamp,
            s.path.display()
        ),
        Lang::En => format!(
            "# Codex Local Session Restore\n\nPlease restore this local Codex session and continue work.\n\n## Session\n\n- Session id: `{}`\n- Thread name: `{}`\n- Original cwd: `{}`\n- Original provider: `{}`\n- Time range: `{}` -> `{}`\n- Local JSONL: `{}`\n\n## Restore Requirements\n\n1. Read `Local JSONL` first; do not rely on server-side `/resume`.\n2. Summarize latest target, constraints, files, verification status, and remaining work.\n3. Treat the last real user request as current.\n",
            s.id,
            if s.thread_name.is_empty() { &s.id } else { &s.thread_name },
            if s.cwd.is_empty() { unknown(lang) } else { &s.cwd },
            if s.provider.is_empty() { unknown(lang) } else { &s.provider },
            s.first_timestamp,
            s.last_timestamp,
            s.path.display()
        ),
    };
    if let Some(m) = latest {
        out.push_str(match lang {
            Lang::Zh => "\n## 最后一条用户请求\n\n",
            Lang::En => "\n## Last User Request\n\n",
        });
        out.push_str(&m.text);
        out.push('\n');
    }
    out
}

fn render_native(s: &Session, lang: Lang) -> String {
    let c = read_config();
    let cur = c.values.get("model_provider").cloned().unwrap_or_default();
    let mut out = match lang {
        Lang::Zh => format!(
            "会话:              {} ({})\r\n会话 provider:     {}\r\n当前 provider:     {}\r\n配置文件:          {}\r\n",
            s.id,
            if s.thread_name.is_empty() { unnamed(lang) } else { &s.thread_name },
            if s.provider.is_empty() { unknown(lang) } else { &s.provider },
            if cur.is_empty() { unknown(lang) } else { &cur },
            c.path.display()
        ),
        Lang::En => format!(
            "session:          {} ({})\r\nsession provider: {}\r\ncurrent provider: {}\r\nconfig:           {}\r\n",
            s.id,
            if s.thread_name.is_empty() { unnamed(lang) } else { &s.thread_name },
            if s.provider.is_empty() { unknown(lang) } else { &s.provider },
            if cur.is_empty() { unknown(lang) } else { &cur },
            c.path.display()
        ),
    };
    if s.provider.is_empty() {
        out.push_str(match lang {
            Lang::Zh => "原生状态:          阻止\r\n原因:              会话 JSONL 没有记录原始 provider\r\n",
            Lang::En => "native status:    blocked\r\nreason:           session JSONL does not record original provider\r\n",
        });
        return out;
    }
    if !c.providers.contains(&s.provider) {
        match lang {
            Lang::Zh => out.push_str(&format!(
                "原生状态:          阻止\r\n原因:              当前配置未定义 provider '{}'\r\n",
                s.provider
            )),
            Lang::En => out.push_str(&format!(
                "native status:    blocked\r\nreason:           provider '{}' is not defined in current config\r\n",
                s.provider
            )),
        }
    } else {
        out.push_str(match lang {
            Lang::Zh => "原生状态:          可能可用\r\n说明:              provider 必须支持 response-chain resume\r\n",
            Lang::En => "native status:    possible\r\nnote:             provider must support response-chain resume\r\n",
        });
    }
    out.push_str(match lang {
        Lang::Zh => "命令:\r\n  ",
        Lang::En => "command:\r\n  ",
    });
    out.push_str(&format!(
        "codex resume {} -c model_provider={} -c disable_response_storage=false",
        s.id,
        quote_toml(&s.provider)
    ));
    if !s.cwd.is_empty() {
        out.push_str(&format!(" -C \"{}\"", s.cwd));
    }
    out.push_str("\r\n");
    out
}

fn render_doctor(s: &Session, lang: Lang) -> String {
    let c = read_config();
    let cur = c.values.get("model_provider").cloned().unwrap_or_default();
    let mut risks = vec![];
    if !cur.is_empty() && !s.provider.is_empty() && cur != s.provider {
        risks.push(match lang {
            Lang::Zh => format!(
                "provider 已变化：会话使用 '{}'，当前配置使用 '{}'",
                s.provider, cur
            ),
            Lang::En => format!(
                "provider changed: session used '{}', current config uses '{}'",
                s.provider, cur
            ),
        });
    }
    if c.values
        .get("disable_response_storage")
        .map(|x| x == "true")
        .unwrap_or(false)
    {
        risks.push(match lang {
            Lang::Zh => "disable_response_storage 为 true，服务端 response-chain resume 可能不可用".into(),
            Lang::En => "disable_response_storage is true, so server-side response-chain resume may be unavailable".into(),
        });
    }
    if c.values
        .get("wire_api")
        .map(|x| x == "responses")
        .unwrap_or(false)
    {
        risks.push(match lang {
            Lang::Zh => "当前 provider 使用 Responses API；provider 必须兼容 response storage/readback".into(),
            Lang::En => "current provider uses the Responses API; provider compatibility must include response storage/readback".into(),
        });
    }
    if s.assistant_turns() == 0 {
        risks.push(match lang {
            Lang::Zh => "本地 JSONL 中没有已完成的 assistant 消息".into(),
            Lang::En => "session has no completed assistant messages in local JSONL".into(),
        });
    }
    if s.aborted_turns() > 0 {
        risks.push(match lang {
            Lang::Zh => format!("会话记录了 {} 个中止轮次", s.aborted_turns()),
            Lang::En => format!("session records {} aborted turn(s)", s.aborted_turns()),
        });
    }
    if s.rolled_back_turns() > 0 {
        risks.push(match lang {
            Lang::Zh => format!("会话记录了 {} 个回滚事件", s.rolled_back_turns()),
            Lang::En => format!(
                "session records {} rollback event(s)",
                s.rolled_back_turns()
            ),
        });
    }
    for e in &s.errors {
        if e.contains("/v1/responses") || e.contains("Invalid URL") || e.contains("Bad Gateway") {
            risks.push(match lang {
                Lang::Zh => format!("会话已记录 API 错误：{}", one_line(e, 160)),
                Lang::En => format!("session already recorded API error: {}", one_line(e, 160)),
            });
            break;
        }
        if e.contains("stream disconnected before completion") {
            risks.push(match lang {
                Lang::Zh => "会话记录了 response.completed 前流断开".into(),
                Lang::En => {
                    "session recorded a stream that closed before response.completed".into()
                }
            });
            break;
        }
    }
    let provider_exists = !s.provider.is_empty() && c.providers.contains(&s.provider);
    let mut out = match lang {
        Lang::Zh => format!(
            "会话:              {} ({})\r\n会话 provider:     {}\r\n当前 provider:     {}\r\n当前 base_url:     {}\r\nwire_api:          {}\r\n已禁用 storage:    {}\r\nprovider 存在:     {}\r\nassistant 轮次:    {}\r\n中止轮次:          {}\r\n回滚事件:          {}\r\n",
            s.id,
            if s.thread_name.is_empty() { unnamed(lang) } else { &s.thread_name },
            if s.provider.is_empty() { unknown(lang) } else { &s.provider },
            if cur.is_empty() { unknown(lang) } else { &cur },
            c.values.get("base_url").map(String::as_str).unwrap_or(unknown(lang)),
            c.values.get("wire_api").map(String::as_str).unwrap_or(unknown(lang)),
            c.values
                .get("disable_response_storage")
                .map(String::as_str)
                .unwrap_or(unknown(lang)),
            yes_no(lang, provider_exists),
            s.assistant_turns(),
            s.aborted_turns(),
            s.rolled_back_turns()
        ),
        Lang::En => format!(
            "session:          {} ({})\r\nsession provider: {}\r\ncurrent provider: {}\r\ncurrent base_url: {}\r\nwire_api:         {}\r\nstorage disabled: {}\r\nprovider exists:  {}\r\nassistant turns:  {}\r\naborted turns:    {}\r\nrollback events:  {}\r\n",
            s.id,
            if s.thread_name.is_empty() { unnamed(lang) } else { &s.thread_name },
            if s.provider.is_empty() { unknown(lang) } else { &s.provider },
            if cur.is_empty() { unknown(lang) } else { &cur },
            c.values.get("base_url").map(String::as_str).unwrap_or(unknown(lang)),
            c.values.get("wire_api").map(String::as_str).unwrap_or(unknown(lang)),
            c.values
                .get("disable_response_storage")
                .map(String::as_str)
                .unwrap_or(unknown(lang)),
            yes_no(lang, provider_exists),
            s.assistant_turns(),
            s.aborted_turns(),
            s.rolled_back_turns()
        ),
    };
    if risks.is_empty() {
        out.push_str(match lang {
            Lang::Zh => "resume 风险:       未发现明显本地风险\r\n",
            Lang::En => "resume risk:      no obvious local risk found\r\n",
        });
    } else {
        out.push_str(match lang {
            Lang::Zh => "resume 风险:       高\r\n原因:\r\n",
            Lang::En => "resume risk:      high\r\nwhy:\r\n",
        });
        for r in risks {
            out.push_str(&format!("  - {}\r\n", r));
        }
    }
    out
}

fn render_sync(dry_run: bool, lang: Lang) -> Result<String, String> {
    let c = read_config();
    let current = c.values.get("model_provider").cloned().unwrap_or_default();
    if current.is_empty() || !c.providers.contains(&current) {
        return Err(match lang {
            Lang::Zh => "当前 model_provider 缺失或未定义".into(),
            Lang::En => "current model_provider is missing or not defined".into(),
        });
    }
    let backup = codex_home().join(format!("provider-sync-backup-{}", timestamp_compact()));
    let providers = c.providers.iter().cloned().collect::<Vec<_>>().join(", ");
    let mut out = match lang {
        Lang::Zh => format!(
            "当前 provider:     {}\r\n已定义 providers: {}\r\n预览模式:          {}\r\n",
            current,
            providers,
            yes_no(lang, dry_run)
        ),
        Lang::En => format!(
            "current provider: {}\r\ndefined providers: {}\r\ndry run:          {}\r\n",
            current, providers, dry_run
        ),
    };
    let mut agent_changes = 0;
    let agents_dir = codex_home().join("agents");
    if let Ok(rd) = fs::read_dir(&agents_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("toml") {
                if let Some(provider) = read_agent_provider(&p) {
                    if !provider.is_empty() && provider != current {
                        agent_changes += 1;
                        out.push_str(&format!(
                            "  agent {}: {} -> {}\r\n",
                            p.file_name().unwrap().to_string_lossy(),
                            provider,
                            current
                        ));
                        if !dry_run {
                            let bdir = backup.join("agents");
                            fs::create_dir_all(&bdir).map_err(|e| e.to_string())?;
                            fs::copy(&p, bdir.join(p.file_name().unwrap()))
                                .map_err(|e| e.to_string())?;
                            let text = fs::read_to_string(&p).map_err(|e| e.to_string())?;
                            let updated = replace_model_provider_line(&text, &current);
                            fs::write(&p, updated.as_bytes()).map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
        }
    }
    out.push_str(&match lang {
        Lang::Zh => format!("agent 引用:       {} 处变更\r\n", agent_changes),
        Lang::En => format!("agent refs:       {} change(s)\r\n", agent_changes),
    });
    let mut session_changes = 0;
    for path in session_paths(&codex_home(), false) {
        let text = fs::read_to_string(&path).unwrap_or_default();
        let mut changed = false;
        let mut old = String::new();
        let mut lines = vec![];
        for line in text.lines() {
            if json_get_string(line, "type").as_deref() == Some("session_meta") {
                if let Some(prov) = json_payload_string(line, "model_provider") {
                    old = prov.clone();
                    if !prov.is_empty() && prov != current {
                        lines.push(replace_json_payload_string(
                            line,
                            "model_provider",
                            &current,
                        ));
                        changed = true;
                        continue;
                    }
                }
            }
            lines.push(line.into());
        }
        if changed {
            session_changes += 1;
            out.push_str(&format!(
                "  session {}: {} -> {}\r\n",
                path.file_name().unwrap().to_string_lossy(),
                old,
                current
            ));
            if !dry_run {
                let bdir = backup.join("sessions");
                fs::create_dir_all(&bdir).map_err(|e| e.to_string())?;
                fs::copy(&path, bdir.join(path.file_name().unwrap())).map_err(|e| e.to_string())?;
                fs::write(&path, format!("{}\n", lines.join("\n"))).map_err(|e| e.to_string())?;
            }
        }
    }
    out.push_str(&match lang {
        Lang::Zh => format!("session 引用:     {} 处变更\r\n", session_changes),
        Lang::En => format!("session refs:     {} change(s)\r\n", session_changes),
    });
    if !dry_run && (agent_changes > 0 || session_changes > 0) {
        out.push_str(&match lang {
            Lang::Zh => format!("备份:              {}\r\n", backup.display()),
            Lang::En => format!("backup:           {}\r\n", backup.display()),
        });
    }
    if agent_changes == 0 && session_changes == 0 {
        out.push_str(match lang {
            Lang::Zh => "状态:              已经同步，无需更改\r\n",
            Lang::En => "status:           already synced\r\n",
        });
    }
    Ok(out)
}
fn replace_json_payload_string(line: &str, key: &str, value: &str) -> String {
    let Some(payload_start) = line.find("\"payload\"") else {
        return line.into();
    };
    let marker = format!("\"{}\"", key);
    let Some(rel) = line[payload_start..].find(&marker) else {
        return line.into();
    };
    let key_pos = payload_start + rel;
    let after_key = key_pos + marker.len();
    let Some(colon_rel) = line[after_key..].find(':') else {
        return line.into();
    };
    let value_start = after_key + colon_rel + 1;
    let ws_len = line[value_start..]
        .chars()
        .take_while(|c| c.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    let raw_start = value_start + ws_len;
    let Some(raw_end) = json_value_end(line, raw_start) else {
        return line.into();
    };
    format!(
        "{}{}{}",
        &line[..raw_start],
        json_quote(value),
        &line[raw_end..]
    )
}

fn json_value_end(line: &str, start: usize) -> Option<usize> {
    if line[start..].starts_with('"') {
        let mut escaped = false;
        for (offset, ch) in line[start + 1..].char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                return Some(start + 1 + offset + ch.len_utf8());
            }
        }
        None
    } else {
        Some(
            start
                + line[start..]
                    .find([',', '}', ']'])
                    .unwrap_or(line.len() - start),
        )
    }
}

fn json_quote(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn read_agent_provider(p: &Path) -> Option<String> {
    let text = fs::read_to_string(p).ok()?;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with("model_provider") {
            return Some(toml_value(l));
        }
    }
    None
}

fn replace_model_provider_line(text: &str, provider: &str) -> String {
    let replacement = format!("model_provider = {}", quote_toml(provider));
    let mut changed = false;
    let mut out = Vec::new();
    for line in text.lines() {
        if !changed && line.trim_start().starts_with("model_provider") {
            let indent = &line[..line.len() - line.trim_start().len()];
            out.push(format!("{indent}{replacement}"));
            changed = true;
        } else {
            out.push(line.to_string());
        }
    }
    if text.ends_with('\n') {
        format!("{}\n", out.join("\n"))
    } else {
        out.join("\n")
    }
}

fn export_restore() {
    if let Some(s) = selected_session() {
        let lang = current_lang();
        let dir = dirs_home().join("Downloads");
        let dir = if dir.exists() { dir } else { dirs_home() };
        let prefix = s.id.chars().take(8).collect::<String>();
        let path = dir.join(format!("codex-restore-{}.md", prefix));
        match fs::write(&path, render_restore(&s, lang)) {
            Ok(_) => {
                set_details(&match lang {
                    Lang::Zh => format!("已写入恢复提示文件: {}", path.display()),
                    Lang::En => format!("Wrote restoration prompt: {}", path.display()),
                });
                shell_open_select(&path);
            }
            Err(e) => set_details(&error_text(e)),
        }
    }
}
fn copy_restore() {
    if let Some(s) = selected_session() {
        let lang = current_lang();
        let prompt = render_restore(&s, lang);
        if set_clipboard(&prompt).is_ok() {
            let prefix = match lang {
                Lang::Zh => "恢复提示已复制到剪贴板。",
                Lang::En => "Restore prompt copied to clipboard.",
            };
            set_details(&format!("{prefix}\r\n\r\n{prompt}"));
        } else {
            set_details(&match lang {
                Lang::Zh => "错误: 写入剪贴板失败".to_string(),
                Lang::En => "error: failed to set clipboard".to_string(),
            });
        }
    }
}

fn open_sessions_folder() {
    let p = codex_home().join("sessions");
    let _ = fs::create_dir_all(&p);
    shell_open(&p);
}
fn shell_open(p: &Path) {
    unsafe {
        let op = wide("open");
        let file = wide(&p.to_string_lossy());
        ShellExecuteW(
            None,
            PCWSTR(op.as_ptr()),
            PCWSTR(file.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
}
fn shell_open_select(p: &Path) {
    unsafe {
        let op = wide("open");
        let exe = wide("explorer.exe");
        let arg = wide(&format!("/select,{}", p.display()));
        ShellExecuteW(
            None,
            PCWSTR(op.as_ptr()),
            PCWSTR(exe.as_ptr()),
            PCWSTR(arg.as_ptr()),
            None,
            SW_SHOWNORMAL,
        );
    }
}
fn set_clipboard(text: &str) -> Result<(), String> {
    unsafe {
        let wide = wide(text);
        if OpenClipboard(None).is_err() {
            return Err("open clipboard".into());
        }
        EmptyClipboard().ok();
        let bytes = wide.len() * 2;
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|e| format!("{e:?}"))?;
        let ptr = GlobalLock(h) as *mut u16;
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
        GlobalUnlock(h).ok();
        SetClipboardData(CF_UNICODETEXT.0 as u32, HANDLE(h.0)).map_err(|e| format!("{e:?}"))?;
        CloseClipboard().ok();
        Ok(())
    }
}

fn set_details(text: &str) {
    STATE.with(|cell| {
        if let Some(st) = cell.borrow().as_ref() {
            set_text(st.details, text);
        }
    });
}
fn set_text(hwnd: HWND, text: &str) {
    unsafe {
        let w = wide(text);
        let _ = SetWindowTextW(hwnd, PCWSTR(w.as_ptr()));
    }
}
fn get_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        let mut buf = vec![0u16; (len + 1) as usize];
        GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..len as usize])
    }
}
fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}
