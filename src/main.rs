#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod error;
mod i18n;
mod tray;
mod win32;
mod watchdog;

use config::AppConfig;
use error::VbarError;
use i18n::I18n;
use slint::{CloseRequestResponse, ComponentHandle, Model, ModelRc, VecModel};
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use tray::{create_tray, update_tray_monitoring_state, TrayAction};
use watchdog::{spawn_watchdog, WatchdogCommand, WatchdogEvent};
use win32::{
    create_single_instance_mutex, enable_per_monitor_dpi_awareness, extract_exe_icon,
    hide_window_by_title, is_autostart_registry_enabled, set_autostart_registry,
    show_and_restore_window_by_title, wake_existing_instance,
};

slint::include_modules!();

const MUTEX_NAME: &str = "Local\\VoicemeeterAutoRestartMutex";
const WINDOW_TITLE: &str = "Voicemeeter Auto Restart (VBAR)";

// Atualiza o ícone do processo e validação de existência do arquivo no disco
fn validate_and_update_ui_path(ui: &AppWindow, exe_path_str: &str) -> bool {
    let path = Path::new(exe_path_str);
    let is_valid = path.exists() && path.is_file();
    ui.set_path_invalid(!is_valid);

    if is_valid {
        if let Some(img) = extract_exe_icon(path) {
            ui.set_process_icon(img);
            ui.set_has_process_icon(true);
        } else {
            ui.set_has_process_icon(false);
        }
    } else {
        ui.set_has_process_icon(false);
    }

    is_valid
}

// Salva as configurações editadas na interface, sincroniza registro e notifica o watchdog
fn save_settings_from_ui(ui: &AppWindow, wd: &watchdog::WatchdogController) {
    let path_str = ui.get_process_path().to_string();
    let lang = ui.get_language().to_string();
    let autostart = ui.get_autostart_with_windows();
    let _ = validate_and_update_ui_path(ui, &path_str);

    // Sincroniza a inicialização com o Windows no Registro Win32 (HKCU\Software\Microsoft\Windows\CurrentVersion\Run)
    let _ = set_autostart_registry(autostart);

    let mut new_config = AppConfig {
        language: lang,
        process_name: ui.get_process_name().to_string(),
        process_path: path_str,
        check_interval_secs: ui.get_check_interval(),
        start_minimized: ui.get_start_minimized(),
        autostart_with_windows: autostart,
        crash_protection_enabled: ui.get_crash_protection_enabled(),
        max_consecutive_crashes: ui.get_max_consecutive_crashes().max(1) as u32,
    };
    new_config.sanitize();
    let _ = new_config.save();
    wd.send(WatchdogCommand::UpdateConfig(new_config));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Ativar nitidez vetorial Per-Monitor DPI V2 para monitores de alta resolução (2K/4K)
    enable_per_monitor_dpi_awareness();

    // 1. Garantir Instância Única via Mutex Win32
    let _instance_mutex = match create_single_instance_mutex(MUTEX_NAME) {
        Ok(m) => m,
        Err(VbarError::AlreadyRunning) => {
            // Se já estiver em execução, restaura a janela existente e encerra a nova instância
            wake_existing_instance(WINDOW_TITLE);
            return Ok(());
        }
        Err(e) => {
            eprintln!("Aviso de inicialização de mutex: {}", e);
            match win32::AutoHandle::new(std::ptr::null_mut()) {
                Some(h) => h,
                None => {
                    wake_existing_instance(WINDOW_TITLE);
                    return Ok(());
                }
            }
        }
    };

    // 2. Carregar configurações com segurança da pasta do usuário (%APPDATA%\VBAR\config.json)
    let mut config = AppConfig::load();
    let registry_autostart = is_autostart_registry_enabled();
    let autostart_enabled = config.autostart_with_windows || registry_autostart;
    if config.autostart_with_windows != autostart_enabled {
        config.autostart_with_windows = autostart_enabled;
        let _ = config.save();
    }
    if autostart_enabled {
        let _ = set_autostart_registry(true);
    }

    let i18n = I18n::from_code(&config.language);

    // 3. Inicializar a janela do Slint
    let main_window = AppWindow::new()?;
    let ui_handle = main_window.as_weak();

    // Aplicar configurações iniciais na interface
    main_window.set_language(config.language.clone().into());
    main_window.set_process_name(config.process_name.clone().into());
    main_window.set_process_path(config.process_path.clone().into());
    main_window.set_check_interval(config.check_interval_secs);
    main_window.set_start_minimized(config.start_minimized);
    main_window.set_autostart_with_windows(config.autostart_with_windows);
    main_window.set_crash_protection_enabled(config.crash_protection_enabled);
    main_window.set_max_consecutive_crashes(config.max_consecutive_crashes as i32);
    main_window.set_is_monitoring(true);

    let is_path_valid = validate_and_update_ui_path(&main_window, &config.process_path);

    main_window.set_status_text(i18n.status_monitoring_active().into());
    main_window.set_target_status(
        if is_path_valid {
            i18n.status_searching_process()
        } else {
            i18n.status_executable_not_found()
        }
        .into(),
    );

    main_window.set_restart_count(0);
    main_window.set_consecutive_crashes(0);
    main_window.set_crash_alert_active(false);

    // Interceptar o botão Fechar ("X") para minimizar para a bandeja via Win32 SW_HIDE
    {
        let ui_close_weak = ui_handle.clone();
        main_window.window().on_close_requested(move || {
            if let Some(ui) = ui_close_weak.upgrade() {
                let _ = ui.hide();
            }
            hide_window_by_title(WINDOW_TITLE);
            CloseRequestResponse::KeepWindowShown
        });
    }

    let logs_model = Rc::new(VecModel::<LogItem>::default());
    main_window.set_logs(ModelRc::from(logs_model.clone()));

    // Estado compartilhado de monitoramento
    let is_monitoring_state = Arc::new(AtomicBool::new(true));

    // 4. Criar ícone e menu da Bandeja do Sistema (System Tray)
    let tray_handle = Arc::new(create_tray(true));

    // 5. Canal de eventos do Watchdog (da thread de trabalho para a thread da interface)
    let (event_tx, event_rx) = channel::<WatchdogEvent>();

    let watchdog_ctrl = spawn_watchdog(config.clone(), move |event| {
        let _ = event_tx.send(event);
    });
    let watchdog_ctrl = Arc::new(watchdog_ctrl);

    // 6. Conectar Callbacks da Interface Gráfica
    // Alternar Monitoramento (Pausar/Retomar)
    {
        let wd = watchdog_ctrl.clone();
        let mon_flag = is_monitoring_state.clone();
        let ui_weak = ui_handle.clone();
        main_window.on_toggle_monitoring(move || {
            let current = mon_flag.load(Ordering::Relaxed);
            let next = !current;
            mon_flag.store(next, Ordering::Relaxed);
            update_tray_monitoring_state(next);

            if next {
                wd.send(WatchdogCommand::Resume);
            } else {
                wd.send(WatchdogCommand::Pause);
            }

            if let Some(ui) = ui_weak.upgrade() {
                ui.set_is_monitoring(next);
            }
        });
    }

    // Reiniciar Voicemeeter Agora
    {
        let wd = watchdog_ctrl.clone();
        main_window.on_restart_voicemeeter_now(move || {
            wd.send(WatchdogCommand::RestartNow);
        });
    }

    // Selecionar Idioma no Menu Dropdown
    {
        let ui_weak = ui_handle.clone();
        let wd = watchdog_ctrl.clone();
        main_window.on_set_language(move |selected_lang| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_language(selected_lang.clone());
                save_settings_from_ui(&ui, &wd);
            }
        });
    }

    // Procurar Executável (Janela de Seleção de Arquivo Nativa)
    {
        let ui_weak = ui_handle.clone();
        let wd = watchdog_ctrl.clone();
        main_window.on_browse_executable_path(move || {
            if let Some(file) = rfd::FileDialog::new()
                .add_filter("Executables (*.exe)", &["exe"])
                .set_title("Select Voicemeeter Executable")
                .pick_file()
            {
                if let Some(ui) = ui_weak.upgrade() {
                    let path_str = file.to_string_lossy().to_string();
                    ui.set_process_path(path_str.clone().into());

                    if let Some(name) = file.file_name() {
                        ui.set_process_name(name.to_string_lossy().to_string().into());
                    }

                    save_settings_from_ui(&ui, &wd);
                }
            }
        });
    }

    // Salvamento Automático quando qualquer configuração for alterada
    {
        let ui_weak = ui_handle.clone();
        let wd = watchdog_ctrl.clone();
        main_window.on_settings_changed(move || {
            if let Some(ui) = ui_weak.upgrade() {
                save_settings_from_ui(&ui, &wd);
            }
        });
    }

    // Resetar Alerta de Falhas Consecutivas
    {
        let wd = watchdog_ctrl.clone();
        let ui_weak = ui_handle.clone();
        let mon_flag = is_monitoring_state.clone();
        main_window.on_reset_crash_alert(move || {
            wd.send(WatchdogCommand::ResetCrashAlert);
            mon_flag.store(true, Ordering::Relaxed);
            update_tray_monitoring_state(true);
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_crash_alert_active(false);
                ui.set_crash_alert_message("".into());
                ui.set_is_monitoring(true);
            }
        });
    }

    // Limpar Histórico de Logs
    {
        let logs = logs_model.clone();
        main_window.on_clear_logs(move || {
            while logs.row_count() > 0 {
                logs.remove(0);
            }
        });
    }

    // Minimizar para a Bandeja pelo botão da interface
    {
        let ui_min_weak = ui_handle.clone();
        main_window.on_minimize_to_tray(move || {
            if let Some(ui) = ui_min_weak.upgrade() {
                let _ = ui.hide();
            }
            hide_window_by_title(WINDOW_TITLE);
        });
    }

    // 7. Timer da Interface (Processa eventos do watchdog e comandos da bandeja a cada 80ms)
    let ui_timer = slint::Timer::default();
    {
        let ui_weak = ui_handle.clone();
        let logs_model = logs_model.clone();
        let tray_ref = tray_handle.clone();
        let wd = watchdog_ctrl.clone();
        let mon_flag = is_monitoring_state.clone();

        ui_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(80),
            move || {
                // 7.1 Processar eventos enviados pela thread do Watchdog
                while let Ok(event) = event_rx.try_recv() {
                    if let Some(ui) = ui_weak.upgrade() {
                        let i18n = I18n::from_code(&ui.get_language());
                        match event {
                            WatchdogEvent::Log {
                                timestamp,
                                level,
                                message,
                            } => {
                                let item = LogItem {
                                    timestamp: timestamp.into(),
                                    level: level.into(),
                                    message: message.into(),
                                };
                                if logs_model.row_count() > 300 {
                                    logs_model.remove(0);
                                }
                                logs_model.push(item);
                            }
                            WatchdogEvent::StatusUpdate {
                                is_monitoring,
                                target_status,
                                restart_count,
                                consecutive_crashes,
                                crash_alert,
                            } => {
                                ui.set_is_monitoring(is_monitoring);
                                ui.set_status_text(
                                    if is_monitoring {
                                        i18n.status_monitoring_active()
                                    } else {
                                        i18n.status_monitoring_paused()
                                    }
                                    .into(),
                                );
                                if !ui.get_path_invalid() {
                                    ui.set_target_status(target_status.into());
                                }
                                ui.set_restart_count(restart_count as i32);
                                ui.set_consecutive_crashes(consecutive_crashes as i32);

                                if let Some(alert) = crash_alert {
                                    ui.set_crash_alert_active(true);
                                    ui.set_crash_alert_message(alert.into());
                                } else {
                                    ui.set_crash_alert_active(false);
                                    ui.set_crash_alert_message("".into());
                                }
                            }
                        }
                    }
                }

                // 7.2 Processar ações de clique do menu da bandeja
                while let Some(action) = tray_ref.try_recv() {
                    match action {
                        TrayAction::ShowWindow => {
                            if let Some(ui) = ui_weak.upgrade() {
                                let _ = ui.show();
                            }
                            show_and_restore_window_by_title(WINDOW_TITLE);
                        }
                        TrayAction::ToggleMonitoring => {
                            let current = mon_flag.load(Ordering::Relaxed);
                            let next = !current;
                            mon_flag.store(next, Ordering::Relaxed);
                            update_tray_monitoring_state(next);
                            if next {
                                wd.send(WatchdogCommand::Resume);
                            } else {
                                wd.send(WatchdogCommand::Pause);
                            }
                            if let Some(ui) = ui_weak.upgrade() {
                                ui.set_is_monitoring(next);
                            }
                        }
                        TrayAction::RestartVoicemeeter => {
                            wd.send(WatchdogCommand::RestartNow);
                        }
                        TrayAction::ExitApp => {
                            wd.send(WatchdogCommand::Stop);
                            let _ = slint::quit_event_loop();
                        }
                    }
                }
            },
        );
    }

    // 8. Exibir a Janela apenas se NÃO estiver configurado para iniciar minimizado
    if !config.start_minimized {
        main_window.show()?;
    }

    // Iniciar loop principal de eventos do Slint
    slint::run_event_loop()?;

    // Ao sair, encerra a thread de monitoramento do watchdog
    watchdog_ctrl.send(WatchdogCommand::Stop);

    Ok(())
}
