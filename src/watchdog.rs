use crate::config::AppConfig;
use crate::i18n::I18n;
use crate::win32::{
    find_process_by_name, format_exit_code, get_process_exit_code, launch_process_safely,
    open_process_for_monitoring, terminate_process_by_pid, wait_for_process_exit,
};
use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum WatchdogCommand {
    Pause,
    Resume,
    RestartNow,
    UpdateConfig(AppConfig),
    ResetCrashAlert,
    Stop,
}

#[derive(Debug, Clone)]
pub enum WatchdogEvent {
    Log {
        timestamp: String,
        level: String,
        message: String,
    },
    StatusUpdate {
        is_monitoring: bool,
        target_status: String,
        restart_count: u32,
        consecutive_crashes: u32,
        crash_alert: Option<String>,
    },
}

pub struct WatchdogController {
    cmd_tx: Sender<WatchdogCommand>,
}

impl WatchdogController {
    pub fn send(&self, cmd: WatchdogCommand) {
        let _ = self.cmd_tx.send(cmd);
    }
}

/// Caminho do arquivo de log persistente com rotação no diretório AppData
fn get_log_file_path() -> PathBuf {
    AppConfig::config_path()
        .parent()
        .map(|p| p.join("vbar.log"))
        .unwrap_or_else(|| PathBuf::from("vbar.log"))
}

/// Grava log formatado em arquivo persistente, com rotação automática ao atingir 2MB
fn write_persistent_log(timestamp: &str, level: &str, message: &str) {
    let log_path = get_log_file_path();
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    // Rotação de log se ultrapassar 2 MB
    if let Ok(meta) = fs::metadata(&log_path) {
        if meta.len() > 2 * 1024 * 1024 {
            let backup_path = log_path.with_extension("log.old");
            let _ = fs::rename(&log_path, backup_path);
        }
    }

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = writeln!(file, "[{}] [{}] {}", timestamp, level, message);
    }
}

pub fn spawn_watchdog(
    initial_config: AppConfig,
    event_callback: impl Fn(WatchdogEvent) + Send + 'static,
) -> WatchdogController {
    let (cmd_tx, cmd_rx) = channel::<WatchdogCommand>();

    thread::spawn(move || {
        let mut config = initial_config;
        let mut is_monitoring = true;
        let mut restart_count: u32 = 0;
        let mut consecutive_crashes: u32 = 0;
        let mut last_process_start: Option<Instant> = None;
        let mut crash_alert: Option<String> = None;

        let log = |level: &str, msg: &str| {
            let now = Local::now().format("%H:%M:%S").to_string();
            write_persistent_log(&now, level, msg);
            event_callback(WatchdogEvent::Log {
                timestamp: now,
                level: level.to_string(),
                message: msg.to_string(),
            });
        };

        let emit_status = |mon: bool, status: &str, rsts: u32, crashes: u32, alert: &Option<String>| {
            event_callback(WatchdogEvent::StatusUpdate {
                is_monitoring: mon,
                target_status: status.to_string(),
                restart_count: rsts,
                consecutive_crashes: crashes,
                crash_alert: alert.clone(),
            });
        };

        let mut i18n = I18n::from_code(&config.language);
        log("INFO", i18n.log_watchdog_started());

        'main_loop: loop {
            // Processar comandos pendentes da interface
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    WatchdogCommand::Stop => {
                        log("INFO", i18n.log_service_stopped());
                        break 'main_loop;
                    }
                    WatchdogCommand::Pause => {
                        is_monitoring = false;
                        log("WARN", i18n.log_monitoring_paused());
                    }
                    WatchdogCommand::Resume => {
                        is_monitoring = true;
                        log("INFO", i18n.log_monitoring_resumed());
                    }
                    WatchdogCommand::ResetCrashAlert => {
                        crash_alert = None;
                        consecutive_crashes = 0;
                        is_monitoring = true;
                        log("SUCCESS", i18n.log_crash_alert_reset());
                    }
                    WatchdogCommand::RestartNow => {
                        log("INFO", i18n.log_manual_restart_requested());

                        // Encerra instância anterior se ainda estiver aberta para liberar drivers de áudio
                        if let Some(pid) = find_process_by_name(&config.process_name) {
                            log("WARN", &i18n.log_terminating_previous_instance(pid));
                            let _ = terminate_process_by_pid(pid, 2000);
                            thread::sleep(Duration::from_millis(500));
                        }

                        let path = Path::new(&config.process_path);
                        match launch_process_safely(path) {
                            Ok(_) => {
                                restart_count += 1;
                                last_process_start = Some(Instant::now());
                                log("SUCCESS", &i18n.log_process_started_success(&config.process_name));
                            }
                            Err(e) => {
                                log("ERROR", &i18n.log_process_start_failed(&e.to_localized_string(&i18n)));
                            }
                        }
                    }
                    WatchdogCommand::UpdateConfig(new_cfg) => {
                        config = new_cfg;
                        i18n = I18n::from_code(&config.language);
                    }
                }
            }

            if !is_monitoring {
                emit_status(
                    false,
                    i18n.status_monitoring_paused(),
                    restart_count,
                    consecutive_crashes,
                    &crash_alert,
                );
                thread::sleep(Duration::from_millis(300));
                continue;
            }

            // Verificar se o processo já está rodando
            if let Some(pid) = find_process_by_name(&config.process_name) {
                let status_msg = i18n.status_process_active(pid);
                emit_status(
                    true,
                    &status_msg,
                    restart_count,
                    consecutive_crashes,
                    &crash_alert,
                );

                if let Some(h_proc) = open_process_for_monitoring(pid) {
                    // Monitorar o processo até que ele encerre ou um comando seja recebido
                    loop {
                        while let Ok(cmd) = cmd_rx.try_recv() {
                            match cmd {
                                WatchdogCommand::Stop => {
                                    log("INFO", i18n.log_service_stopped());
                                    break 'main_loop;
                                }
                                WatchdogCommand::Pause => {
                                    is_monitoring = false;
                                    log("WARN", i18n.log_monitoring_paused());
                                    break;
                                }
                                WatchdogCommand::Resume => {}
                                WatchdogCommand::ResetCrashAlert => {
                                    crash_alert = None;
                                    consecutive_crashes = 0;
                                }
                                WatchdogCommand::RestartNow => {
                                    log("INFO", i18n.log_manual_restart_requested());
                                    let _ = terminate_process_by_pid(pid, 2000);
                                    thread::sleep(Duration::from_millis(500));
                                    let _ = launch_process_safely(Path::new(&config.process_path));
                                }
                                WatchdogCommand::UpdateConfig(new_cfg) => {
                                    config = new_cfg;
                                    i18n = I18n::from_code(&config.language);
                                }
                            }
                        }

                        if !is_monitoring {
                            break;
                        }

                        // Aguardar término em fatias de 400ms para manter o watchdog responsivo
                        match wait_for_process_exit(&h_proc, 400) {
                            Some(true) => {
                                // O processo foi encerrado!
                                let exit_code = get_process_exit_code(&h_proc).unwrap_or(0);
                                let exit_diag = format_exit_code(exit_code, &i18n);

                                if exit_code != 0 {
                                    log(
                                        "ERROR",
                                        &i18n.log_process_crashed(&config.process_name, pid, &exit_diag),
                                    );
                                } else {
                                    log("WARN", &i18n.log_process_clean_exit(&config.process_name, pid));
                                }

                                // Verificar se a queda foi rápida (menos de 60 segundos após iniciar)
                                let is_quick_crash = last_process_start
                                    .map(|t| t.elapsed() < Duration::from_secs(60))
                                    .unwrap_or(false);

                                if exit_code != 0 || is_quick_crash {
                                    consecutive_crashes += 1;
                                } else {
                                    consecutive_crashes = 0;
                                }

                                // Proteção contra Crash Loop (quedas sucessivas)
                                if config.crash_protection_enabled
                                    && consecutive_crashes >= config.max_consecutive_crashes
                                {
                                    let alert_msg = i18n.log_crash_loop_triggered(consecutive_crashes);
                                    log("ERROR", &alert_msg);
                                    crash_alert = Some(alert_msg);
                                    is_monitoring = false;
                                    emit_status(
                                        false,
                                        i18n.status_crash_loop_detected(),
                                        restart_count,
                                        consecutive_crashes,
                                        &crash_alert,
                                    );
                                    break;
                                }

                                // Reiniciar processo automaticamente
                                log("INFO", &i18n.log_restarting_process(&config.process_name));
                                thread::sleep(Duration::from_millis(800));

                                let path = Path::new(&config.process_path);
                                match launch_process_safely(path) {
                                    Ok(_) => {
                                        restart_count += 1;
                                        last_process_start = Some(Instant::now());
                                        log(
                                            "SUCCESS",
                                            &i18n.log_process_restarted_success(restart_count),
                                        );
                                    }
                                    Err(err) => {
                                        log("ERROR", &i18n.log_process_start_failed(&err.to_localized_string(&i18n)));
                                    }
                                }

                                thread::sleep(Duration::from_millis(1500));
                                break;
                            }
                            Some(false) => {
                                // Processo ainda em execução normal
                            }
                            None => {
                                // Erro ao consultar o handle
                                break;
                            }
                        }
                    }
                } else {
                    log("WARN", &i18n.log_handle_open_failed(pid));
                    thread::sleep(Duration::from_secs(1));
                }
            } else {
                // Processo não está em execução
                emit_status(
                    true,
                    i18n.status_starting_process(),
                    restart_count,
                    consecutive_crashes,
                    &crash_alert,
                );

                log("WARN", &i18n.log_process_not_running(&config.process_name));

                let path = Path::new(&config.process_path);
                match launch_process_safely(path) {
                    Ok(_) => {
                        restart_count += 1;
                        last_process_start = Some(Instant::now());
                        log(
                            "SUCCESS",
                            &i18n.log_process_started_success(&config.process_name),
                        );
                    }
                    Err(e) => {
                        log("ERROR", &i18n.log_process_start_failed(&e.to_localized_string(&i18n)));
                    }
                }

                // Aguardar o intervalo de checagem configurado
                let sleep_ms = (config.check_interval_secs * 1000.0) as u64;
                thread::sleep(Duration::from_millis(sleep_ms.clamp(500, 60000)));
            }
        }
    });

    WatchdogController { cmd_tx }
}
