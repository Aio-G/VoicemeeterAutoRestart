use crate::error::VbarError;
use crate::i18n::I18n;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use std::env;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, HANDLE,
    INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::System::Diagnostics::Debug::{
    FormatMessageW, FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ,
};
use windows_sys::Win32::System::Threading::{
    CreateMutexW, CreateProcessW, GetExitCodeProcess, OpenProcess, TerminateProcess,
    WaitForSingleObject, PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_TERMINATE, STARTUPINFOW,
};
use windows_sys::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows_sys::Win32::UI::Shell::ExtractIconExW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, FindWindowW, GetIconInfo, SetForegroundWindow, ShowWindow, HICON, ICONINFO,
    SW_HIDE, SW_RESTORE, SW_SHOW,
};

pub const SYNCHRONIZE: u32 = 0x00100000;
const RUN_REGISTRY_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const APP_REGISTRY_VALUE_NAME: &str = "VBAR";

/// Obtém a descrição legível oficial do Windows para um código de erro Win32
pub fn get_win32_error_description(code: u32) -> String {
    let mut buffer = [0u16; 512];
    let len = unsafe {
        FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            std::ptr::null(),
            code,
            0,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            std::ptr::null(),
        )
    };
    if len > 0 {
        String::from_utf16_lossy(&buffer[..len as usize])
            .trim()
            .to_string()
    } else {
        format!("Código 0x{:08X}", code)
    }
}

/// Ativa conscientização nativa de alto DPI (Per-Monitor DPI V2) para nitidez vetorial em monitores 2K/4K.
pub fn enable_per_monitor_dpi_awareness() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

/// Estrutura RAII para encapsular HANDLEs do Windows e garantir fechamento automático (evitando vazamento de recursos).
#[derive(Debug)]
pub struct AutoHandle(HANDLE);

impl AutoHandle {
    pub fn new(handle: HANDLE) -> Option<Self> {
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            None
        } else {
            Some(Self(handle))
        }
    }

    pub fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for AutoHandle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.0);
            }
            self.0 = std::ptr::null_mut();
        }
    }
}

// Converte string Rust para vetor UTF-16 terminado em nulo exigido pela Win32 API
pub fn to_wide_string(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Registra ou remove o executável na inicialização do Windows (HKCU\Software\Microsoft\Windows\CurrentVersion\Run)
pub fn set_autostart_registry(enable: bool) -> Result<(), VbarError> {
    let wide_key = to_wide_string(RUN_REGISTRY_KEY);
    let wide_val = to_wide_string(APP_REGISTRY_VALUE_NAME);

    unsafe {
        let mut h_key: HKEY = std::ptr::null_mut();
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            wide_key.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut h_key,
        );

        if status != ERROR_SUCCESS {
            return Err(VbarError::Win32Error(
                "RegOpenKeyExW".into(),
                status,
                get_win32_error_description(status),
            ));
        }

        if enable {
            if let Ok(exe_path) = env::current_exe() {
                let cmd_str = format!("\"{}\"", exe_path.to_string_lossy());
                let wide_cmd = to_wide_string(&cmd_str);
                let byte_len = (wide_cmd.len() * std::mem::size_of::<u16>()) as u32;

                let set_res = RegSetValueExW(
                    h_key,
                    wide_val.as_ptr(),
                    0,
                    REG_SZ,
                    wide_cmd.as_ptr() as *const u8,
                    byte_len,
                );
                RegCloseKey(h_key);

                if set_res != ERROR_SUCCESS {
                    return Err(VbarError::Win32Error(
                        "RegSetValueExW".into(),
                        set_res,
                        get_win32_error_description(set_res),
                    ));
                }
            }
        } else {
            let _ = RegDeleteValueW(h_key, wide_val.as_ptr());
            RegCloseKey(h_key);
        }

        Ok(())
    }
}

/// Verifica se a inicialização com o Windows já está habilitada no registro
pub fn is_autostart_registry_enabled() -> bool {
    let wide_key = to_wide_string(RUN_REGISTRY_KEY);
    let wide_val = to_wide_string(APP_REGISTRY_VALUE_NAME);

    unsafe {
        let mut h_key: HKEY = std::ptr::null_mut();
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            wide_key.as_ptr(),
            0,
            KEY_QUERY_VALUE,
            &mut h_key,
        );

        if status != ERROR_SUCCESS {
            return false;
        }

        let mut data_type: u32 = 0;
        let mut byte_len: u32 = 0;
        let query_res = RegQueryValueExW(
            h_key,
            wide_val.as_ptr(),
            std::ptr::null(),
            &mut data_type,
            std::ptr::null_mut(),
            &mut byte_len,
        );

        RegCloseKey(h_key);
        query_res == ERROR_SUCCESS && byte_len > 0
    }
}

/// Oculta uma janela pelo título exato usando Win32 API.
pub fn hide_window_by_title(title: &str) -> bool {
    let wide_title = to_wide_string(title);
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), wide_title.as_ptr());
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_HIDE);
            true
        } else {
            false
        }
    }
}

/// Restaura e traz uma janela para o primeiro plano usando Win32 API.
pub fn show_and_restore_window_by_title(title: &str) -> bool {
    let wide_title = to_wide_string(title);
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), wide_title.as_ptr());
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_SHOW);
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
            true
        } else {
            false
        }
    }
}

/// Notifica a instância existente em execução para exibir a janela (mesmo se estiver minimizada na bandeja).
pub fn wake_existing_instance(title: &str) {
    let wide_tray_class = to_wide_string("VBARTrayWindowClass");
    unsafe {
        let tray_hwnd = FindWindowW(wide_tray_class.as_ptr(), std::ptr::null());
        if !tray_hwnd.is_null() {
            windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                tray_hwnd,
                windows_sys::Win32::UI::WindowsAndMessaging::WM_COMMAND,
                1001, // ID_TRAY_RESTORE
                0,
            );
        }
    }
    show_and_restore_window_by_title(title);
}

/// Extrai o ícone do arquivo executável (.exe) e converte para uma imagem do Slint.
pub fn extract_exe_icon(exe_path: &Path) -> Option<Image> {
    if !exe_path.exists() {
        return None;
    }
    let wide_path = to_wide_string(&exe_path.to_string_lossy());
    let mut large_icon: HICON = std::ptr::null_mut();
    let mut small_icon: HICON = std::ptr::null_mut();

    unsafe {
        let count = ExtractIconExW(
            wide_path.as_ptr(),
            0,
            &mut large_icon,
            &mut small_icon,
            1,
        );

        let icon_to_use = if count > 0 && !large_icon.is_null() {
            if !small_icon.is_null() {
                DestroyIcon(small_icon);
            }
            large_icon
        } else if count > 0 && !small_icon.is_null() {
            small_icon
        } else {
            return None;
        };

        let mut icon_info: ICONINFO = std::mem::zeroed();
        if GetIconInfo(icon_to_use, &mut icon_info) == 0 {
            DestroyIcon(icon_to_use);
            return None;
        }

        let hdc = GetDC(std::ptr::null_mut());
        let mem_dc = CreateCompatibleDC(hdc);

        let mut bmp: BITMAP = std::mem::zeroed();
        GetObjectW(
            icon_info.hbmColor,
            std::mem::size_of::<BITMAP>() as i32,
            &mut bmp as *mut _ as *mut _,
        );

        let width = bmp.bmWidth as u32;
        let height = bmp.bmHeight as u32;

        if width == 0 || height == 0 {
            ReleaseDC(std::ptr::null_mut(), hdc);
            DeleteDC(mem_dc);
            if !icon_info.hbmColor.is_null() {
                DeleteObject(icon_info.hbmColor);
            }
            if !icon_info.hbmMask.is_null() {
                DeleteObject(icon_info.hbmMask);
            }
            DestroyIcon(icon_to_use);
            return None;
        }

        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = width as i32;
        bi.bmiHeader.biHeight = -(height as i32); // DIB top-down
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB as u32;

        let mut buffer: Vec<u8> = vec![0u8; (width * height * 4) as usize];

        GetDIBits(
            mem_dc,
            icon_info.hbmColor,
            0,
            height,
            buffer.as_mut_ptr() as *mut _,
            &mut bi,
            DIB_RGB_COLORS,
        );

        let mut pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
        let pixels = pixel_buffer.make_mut_bytes();

        let mut has_alpha = false;
        for i in 0..(width * height) as usize {
            let b = buffer[i * 4];
            let g = buffer[i * 4 + 1];
            let r = buffer[i * 4 + 2];
            let a = buffer[i * 4 + 3];
            if a > 0 {
                has_alpha = true;
            }
            pixels[i * 4] = r;
            pixels[i * 4 + 1] = g;
            pixels[i * 4 + 2] = b;
            pixels[i * 4 + 3] = a;
        }

        if !has_alpha {
            for i in 0..(width * height) as usize {
                pixels[i * 4 + 3] = 255;
            }
        }

        ReleaseDC(std::ptr::null_mut(), hdc);
        DeleteDC(mem_dc);
        if !icon_info.hbmColor.is_null() {
            DeleteObject(icon_info.hbmColor);
        }
        if !icon_info.hbmMask.is_null() {
            DeleteObject(icon_info.hbmMask);
        }
        DestroyIcon(icon_to_use);

        Some(Image::from_rgba8(pixel_buffer))
    }
}

/// Cria um mutex de instância única. Retorna Ok(handle) se for a única instância, ou Err(VbarError::AlreadyRunning).
pub fn create_single_instance_mutex(mutex_name: &str) -> Result<AutoHandle, VbarError> {
    let wide_name = to_wide_string(mutex_name);
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 1, wide_name.as_ptr());
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            let code = GetLastError();
            return Err(VbarError::Win32Error(
                "CreateMutexW".into(),
                code,
                get_win32_error_description(code),
            ));
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(handle);
            return Err(VbarError::AlreadyRunning);
        }
        Ok(AutoHandle(handle))
    }
}

/// Localiza o PID de um processo pelo nome (insensível a maiúsculas/minúsculas) usando snapshot Win32.
pub fn find_process_by_name(process_name: &str) -> Option<u32> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        let snap_handle = AutoHandle::new(snap)?;

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snap_handle.raw(), &mut entry) != 0 {
            let target_lower = process_name.to_lowercase();
            loop {
                let null_pos = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let exe_name = String::from_utf16_lossy(&entry.szExeFile[..null_pos]);

                if exe_name.to_lowercase() == target_lower {
                    return Some(entry.th32ProcessID);
                }

                if Process32NextW(snap_handle.raw(), &mut entry) == 0 {
                    break;
                }
            }
        }
        None
    }
}

/// Abre o handle de um processo com permissões de sincronização e consulta para monitoramento.
pub fn open_process_for_monitoring(pid: u32) -> Option<AutoHandle> {
    unsafe {
        let handle = OpenProcess(
            SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        );
        AutoHandle::new(handle)
    }
}

/// Encerra um processo travado pelo PID e aguarda a liberação dos drivers de áudio do Windows.
pub fn terminate_process_by_pid(pid: u32, timeout_ms: u32) -> Result<(), VbarError> {
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE | SYNCHRONIZE, 0, pid);
        let auto_h = AutoHandle::new(handle).ok_or(VbarError::ProcessTerminationFailed(pid))?;

        if TerminateProcess(auto_h.raw(), 1) != 0 {
            let _ = WaitForSingleObject(auto_h.raw(), timeout_ms);
            Ok(())
        } else {
            let code = GetLastError();
            Err(VbarError::Win32Error(
                "TerminateProcess".into(),
                code,
                get_win32_error_description(code),
            ))
        }
    }
}

/// Aguarda a finalização de um processo por um tempo limite em milissegundos.
pub fn wait_for_process_exit(handle: &AutoHandle, timeout_ms: u32) -> Option<bool> {
    unsafe {
        let res = WaitForSingleObject(handle.raw(), timeout_ms);
        match res {
            WAIT_OBJECT_0 => Some(true),
            WAIT_TIMEOUT => Some(false),
            _ => None,
        }
    }
}

/// Obtém o código de saída (Exit Code) de um processo encerrado.
pub fn get_process_exit_code(handle: &AutoHandle) -> Option<u32> {
    unsafe {
        let mut exit_code: u32 = 0;
        if GetExitCodeProcess(handle.raw(), &mut exit_code) != 0 {
            Some(exit_code)
        } else {
            None
        }
    }
}

/// Inicia o processo de forma segura com validação prévia de arquivo e escape de aspas (contra Path Hijacking).
pub fn launch_process_safely(exe_path: &Path) -> Result<(), VbarError> {
    if !exe_path.exists() {
        return Err(VbarError::ExecutableNotFound(exe_path.to_path_buf()));
    }
    if !exe_path.is_file() {
        return Err(VbarError::InvalidExecutable(exe_path.to_path_buf()));
    }
    if exe_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase() != "exe" {
        return Err(VbarError::InvalidExecutable(exe_path.to_path_buf()));
    }

    // Envolve em aspas para proteger contra vulnerabilidade de Path Hijacking em pastas com espaços
    let cmd_str = format!("\"{}\"", exe_path.to_string_lossy());
    let mut wide_cmd = to_wide_string(&cmd_str);

    let mut startup_info: STARTUPINFOW = unsafe { std::mem::zeroed() };
    startup_info.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

    let mut process_info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    unsafe {
        let ok = CreateProcessW(
            std::ptr::null(),
            wide_cmd.as_mut_ptr(),
            std::ptr::null() as *const SECURITY_ATTRIBUTES,
            std::ptr::null() as *const SECURITY_ATTRIBUTES,
            0,
            0,
            std::ptr::null(),
            std::ptr::null(),
            &startup_info,
            &mut process_info,
        );

        if ok != 0 {
            // Fechamento automático dos handles da thread e processo via RAII
            let _h_proc = AutoHandle::new(process_info.hProcess);
            let _h_thrd = AutoHandle::new(process_info.hThread);
            Ok(())
        } else {
            let code = GetLastError();
            Err(VbarError::Win32Error(
                "CreateProcessW".into(),
                code,
                get_win32_error_description(code),
            ))
        }
    }
}

/// Formata o código de saída em hexadecimal e traduz as exceções usando o catálogo tipado de I18n.
pub fn format_exit_code(code: u32, i18n: &I18n) -> String {
    i18n.exit_code_diagnosis(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    #[test]
    fn test_to_wide_string() {
        let wide = to_wide_string("test");
        assert_eq!(wide, vec!['t' as u16, 'e' as u16, 's' as u16, 't' as u16, 0]);
    }

    #[test]
    fn test_format_exit_code_with_i18n() {
        let en = I18n::new(Language::English);
        let pt = I18n::new(Language::Portuguese);
        assert!(format_exit_code(0xC0000005, &en).contains("Access Violation"));
        assert!(format_exit_code(0x00000000, &en).contains("Normal"));
        assert!(format_exit_code(0x12345678, &en).contains("0x12345678"));
        assert!(format_exit_code(0xC0000005, &pt).contains("Access Violation"));
    }

    #[test]
    fn test_launch_process_safety_checks() {
        let non_existent = Path::new("C:\\invalid\\path\\fake_app.exe");
        assert!(launch_process_safely(non_existent).is_err());

        let non_exe = Path::new("Cargo.toml");
        assert!(launch_process_safely(non_exe).is_err());
    }

    #[test]
    fn test_get_win32_error_description() {
        let msg = get_win32_error_description(2); // ERROR_FILE_NOT_FOUND
        assert!(!msg.is_empty());
    }
}
