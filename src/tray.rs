use crate::win32::to_wide_string;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::OnceLock;
use std::thread;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, LoadIconW, LoadImageW, PostQuitMessage,
    RegisterClassW, SetForegroundWindow, TrackPopupMenu, TranslateMessage,
    IDI_APPLICATION, IMAGE_ICON, LR_DEFAULTCOLOR, MF_SEPARATOR, MF_STRING,
    TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RIGHTBUTTON, WM_COMMAND, WM_DESTROY,
    WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP, WM_USER, WNDCLASSW,
};

const WM_TRAYICON: u32 = WM_USER + 100;
const HWND_MESSAGE: HWND = -3isize as HWND;

pub const ID_TRAY_RESTORE: usize = 1001;
pub const ID_TRAY_TOGGLE_MON: usize = 1002;
pub const ID_TRAY_RESTART_VM: usize = 1003;
pub const ID_TRAY_EXIT: usize = 1004;

#[derive(Debug, Clone)]
pub enum TrayAction {
    ShowWindow,
    ToggleMonitoring,
    RestartVoicemeeter,
    ExitApp,
}

pub struct TrayHandle {
    event_rx: Receiver<TrayAction>,
    #[allow(dead_code)]
    pub hwnd_raw: isize,
}

impl TrayHandle {
    pub fn try_recv(&self) -> Option<TrayAction> {
        self.event_rx.try_recv().ok()
    }
}

static IS_MONITORING_ATOMIC: AtomicBool = AtomicBool::new(true);
static EVENT_TX: OnceLock<Sender<TrayAction>> = OnceLock::new();

// Procedimento de janela Win32 para capturar cliques no ícone da bandeja e comandos do menu de contexto
unsafe extern "system" fn tray_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            let event = lparam as u32;
            if event == WM_LBUTTONUP || event == WM_LBUTTONDBLCLK {
                if let Some(tx) = EVENT_TX.get() {
                    let _ = tx.send(TrayAction::ShowWindow);
                }
            } else if event == WM_RBUTTONUP {
                let mut pt = POINT { x: 0, y: 0 };
                GetCursorPos(&mut pt);
                SetForegroundWindow(hwnd);

                let hmenu = CreatePopupMenu();
                let mon_label = if IS_MONITORING_ATOMIC.load(Ordering::Relaxed) {
                    to_wide_string("Pausar / Pause")
                } else {
                    to_wide_string("Ativar / Resume")
                };

                let w_open = to_wide_string("Abrir Painel VBAR / Open VBAR");
                let w_restart = to_wide_string("Reiniciar Voicemeeter / Restart");
                let w_exit = to_wide_string("Sair / Exit");

                AppendMenuW(hmenu, MF_STRING, ID_TRAY_RESTORE, w_open.as_ptr());
                AppendMenuW(hmenu, MF_SEPARATOR, 0, std::ptr::null());
                AppendMenuW(hmenu, MF_STRING, ID_TRAY_TOGGLE_MON, mon_label.as_ptr());
                AppendMenuW(hmenu, MF_STRING, ID_TRAY_RESTART_VM, w_restart.as_ptr());
                AppendMenuW(hmenu, MF_SEPARATOR, 0, std::ptr::null());
                AppendMenuW(hmenu, MF_STRING, ID_TRAY_EXIT, w_exit.as_ptr());

                TrackPopupMenu(
                    hmenu,
                    TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_BOTTOMALIGN,
                    pt.x,
                    pt.y,
                    0,
                    hwnd,
                    std::ptr::null(),
                );
                DestroyMenu(hmenu);
            }
            0
        }
        WM_COMMAND => {
            let cmd_id = (wparam & 0xFFFF) as usize;
            if let Some(tx) = EVENT_TX.get() {
                match cmd_id {
                    ID_TRAY_RESTORE => {
                        let _ = tx.send(TrayAction::ShowWindow);
                    }
                    ID_TRAY_TOGGLE_MON => {
                        let _ = tx.send(TrayAction::ToggleMonitoring);
                    }
                    ID_TRAY_RESTART_VM => {
                        let _ = tx.send(TrayAction::RestartVoicemeeter);
                    }
                    ID_TRAY_EXIT => {
                        let _ = tx.send(TrayAction::ExitApp);
                    }
                    _ => {}
                }
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn create_tray(is_monitoring: bool) -> TrayHandle {
    IS_MONITORING_ATOMIC.store(is_monitoring, Ordering::Relaxed);
    let (tx, rx) = channel::<TrayAction>();
    let (hwnd_tx, hwnd_rx) = channel::<isize>();

    let _ = EVENT_TX.set(tx);

    thread::spawn(move || {
        let class_name = to_wide_string("VBARTrayWindowClass");

        unsafe {
            let hinst = GetModuleHandleW(std::ptr::null());
            let app_icon = {
                let ico = LoadImageW(
                    hinst,
                    1 as *const u16,
                    IMAGE_ICON,
                    16,
                    16,
                    LR_DEFAULTCOLOR,
                );
                if !ico.is_null() {
                    ico as *mut _
                } else {
                    LoadIconW(std::ptr::null_mut(), IDI_APPLICATION)
                }
            };

            let wnd_class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(tray_window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: app_icon,
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name.as_ptr(),
            };

            RegisterClassW(&wnd_class);

            // Janela de mensagens (HWND_MESSAGE) que não polui o Gerenciador de Tarefas
            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                std::ptr::null_mut(),
                hinst,
                std::ptr::null_mut(),
            );

            let _ = hwnd_tx.send(hwnd as isize);

            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
            nid.uCallbackMessage = WM_TRAYICON;
            nid.hIcon = app_icon;

            let tip_text = to_wide_string("Voicemeeter Auto Restart (VBAR)");
            let copy_len = tip_text.len().min(nid.szTip.len());
            nid.szTip[..copy_len].copy_from_slice(&tip_text[..copy_len]);

            Shell_NotifyIconW(NIM_ADD, &nid);

            let mut msg = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            Shell_NotifyIconW(NIM_DELETE, &nid);
            DestroyWindow(hwnd);
        }
    });

    let hwnd_raw = hwnd_rx.recv().unwrap_or(0);
    TrayHandle {
        event_rx: rx,
        hwnd_raw,
    }
}

pub fn update_tray_monitoring_state(is_monitoring: bool) {
    IS_MONITORING_ATOMIC.store(is_monitoring, Ordering::Relaxed);
}
