use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "pt")]
    Portuguese,
}

impl Language {
    pub fn from_code(code: &str) -> Self {
        match code.to_lowercase().trim() {
            "pt" | "pt-br" | "portuguese" => Self::Portuguese,
            _ => Self::English,
        }
    }

    #[allow(dead_code)]
    pub fn as_code(&self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Portuguese => "pt",
        }
    }
}

/// Catálogo centralizado e tipado de traduções (Zero Overhead, Garantia em tempo de compilação)
#[derive(Debug, Clone, Copy)]
pub struct I18n {
    pub lang: Language,
}

impl I18n {
    #[allow(dead_code)]
    pub fn new(lang: Language) -> Self {
        Self { lang }
    }

    pub fn from_code(code: &str) -> Self {
        Self {
            lang: Language::from_code(code),
        }
    }

    // --- Status da Interface e Watchdog ---
    pub fn status_monitoring_active(&self) -> &'static str {
        match self.lang {
            Language::English => "Monitoring active",
            Language::Portuguese => "Monitorando em execução",
        }
    }

    pub fn status_monitoring_paused(&self) -> &'static str {
        match self.lang {
            Language::English => "Monitoring Paused",
            Language::Portuguese => "Monitoramento Pausado",
        }
    }

    pub fn status_searching_process(&self) -> &'static str {
        match self.lang {
            Language::English => "Searching for process...",
            Language::Portuguese => "Buscando processo...",
        }
    }

    pub fn status_executable_not_found(&self) -> &'static str {
        match self.lang {
            Language::English => "⚠️ Executable not found",
            Language::Portuguese => "⚠️ Executável não encontrado",
        }
    }

    pub fn status_process_active(&self, pid: u32) -> String {
        match self.lang {
            Language::English => format!("Active (PID: {})", pid),
            Language::Portuguese => format!("Ativo (PID: {})", pid),
        }
    }

    pub fn status_starting_process(&self) -> &'static str {
        match self.lang {
            Language::English => "Process not found (Starting...)",
            Language::Portuguese => "Processo não encontrado (Iniciando...)",
        }
    }

    pub fn status_crash_loop_detected(&self) -> &'static str {
        match self.lang {
            Language::English => "Crash Loop Detected",
            Language::Portuguese => "Crash Loop Detectado",
        }
    }

    // --- Logs e Eventos do Watchdog ---
    pub fn log_watchdog_started(&self) -> &'static str {
        match self.lang {
            Language::English => "Starting Voicemeeter Auto Restart (VBAR) Watchdog",
            Language::Portuguese => "Iniciando Watchdog Voicemeeter Auto Restart (VBAR)",
        }
    }

    pub fn log_service_stopped(&self) -> &'static str {
        match self.lang {
            Language::English => "Monitoring service stopped.",
            Language::Portuguese => "Encerrando serviço de monitoramento.",
        }
    }

    pub fn log_monitoring_paused(&self) -> &'static str {
        match self.lang {
            Language::English => "Monitoring paused by user.",
            Language::Portuguese => "Monitoramento pausado pelo usuário.",
        }
    }

    pub fn log_monitoring_resumed(&self) -> &'static str {
        match self.lang {
            Language::English => "Monitoring resumed.",
            Language::Portuguese => "Monitoramento retomado.",
        }
    }

    pub fn log_crash_alert_reset(&self) -> &'static str {
        match self.lang {
            Language::English => "Crash alert reset. Monitoring reactivated.",
            Language::Portuguese => "Alerta de falhas resetado. Monitoramento reativado.",
        }
    }

    pub fn log_manual_restart_requested(&self) -> &'static str {
        match self.lang {
            Language::English => "Manual restart requested...",
            Language::Portuguese => "Solicitação manual de reinicialização...",
        }
    }

    pub fn log_terminating_previous_instance(&self, pid: u32) -> String {
        match self.lang {
            Language::English => {
                format!("Terminating previous instance (PID: {}) to release audio drivers...", pid)
            }
            Language::Portuguese => {
                format!("Encerrando instância anterior (PID: {}) para liberar drivers de áudio...", pid)
            }
        }
    }

    pub fn log_process_started_success(&self, proc_name: &str) -> String {
        match self.lang {
            Language::English => format!("Process '{}' started successfully.", proc_name),
            Language::Portuguese => format!("Processo '{}' iniciado com sucesso.", proc_name),
        }
    }

    pub fn log_process_restarted_success(&self, count: u32) -> String {
        match self.lang {
            Language::English => {
                format!("Process restarted successfully (Restart #{})", count)
            }
            Language::Portuguese => {
                format!("Processo reiniciado com sucesso (Reinício #{})", count)
            }
        }
    }

    pub fn log_process_start_failed(&self, err: &str) -> String {
        match self.lang {
            Language::English => format!("Failed to start process: {}", err),
            Language::Portuguese => format!("Falha ao iniciar processo: {}", err),
        }
    }

    #[allow(dead_code)]
    pub fn log_config_updated(&self, interval: f32, target: &str) -> String {
        match self.lang {
            Language::English => {
                format!("Configuration updated. Interval: {:.1}s, Target: {}", interval, target)
            }
            Language::Portuguese => {
                format!("Configuração atualizada. Intervalo: {:.1}s, Alvo: {}", interval, target)
            }
        }
    }

    #[allow(dead_code)]
    pub fn log_language_switched(&self) -> &'static str {
        match self.lang {
            Language::English => "Language switched to English.",
            Language::Portuguese => "Idioma alterado para Português.",
        }
    }

    pub fn log_process_crashed(&self, proc: &str, pid: u32, diag: &str) -> String {
        match self.lang {
            Language::English => format!("Process '{}' (PID: {}) crashed! {}", proc, pid, diag),
            Language::Portuguese => format!("Processo '{}' (PID: {}) crashou! {}", proc, pid, diag),
        }
    }

    pub fn log_process_clean_exit(&self, proc: &str, pid: u32) -> String {
        match self.lang {
            Language::English => {
                format!("Process '{}' (PID: {}) was terminated (Exit Code: 0).", proc, pid)
            }
            Language::Portuguese => {
                format!("Processo '{}' (PID: {}) foi encerrado (Exit Code: 0).", proc, pid)
            }
        }
    }

    pub fn log_crash_loop_triggered(&self, crashes: u32) -> String {
        match self.lang {
            Language::English => {
                format!("Detected {} consecutive crashes! Watchdog paused to protect system.", crashes)
            }
            Language::Portuguese => {
                format!("Detectados {} crashes consecutivos! Watchdog desativado para evitar sobrecarga.", crashes)
            }
        }
    }

    pub fn log_restarting_process(&self, proc: &str) -> String {
        match self.lang {
            Language::English => format!("Restarting process '{}'...", proc),
            Language::Portuguese => format!("Reiniciando processo '{}'...", proc),
        }
    }

    pub fn log_process_not_running(&self, proc: &str) -> String {
        match self.lang {
            Language::English => format!("Process '{}' is not running. Starting...", proc),
            Language::Portuguese => format!("Processo '{}' não está em execução. Iniciando...", proc),
        }
    }

    pub fn log_handle_open_failed(&self, pid: u32) -> String {
        match self.lang {
            Language::English => {
                format!("Could not open process handle for PID: {}. Retrying...", pid)
            }
            Language::Portuguese => {
                format!("Não foi possível abrir handle do processo PID: {}. Tentando novamente...", pid)
            }
        }
    }

    // --- Diagnósticos de Código de Saída Win32 ---
    pub fn exit_code_diagnosis(&self, code: u32) -> String {
        match code {
            0x00000000 => match self.lang {
                Language::English => "Normal Exit (0x00000000)".to_string(),
                Language::Portuguese => "Encerramento Normal (0x00000000)".to_string(),
            },
            0xC0000005 => match self.lang {
                Language::English => {
                    "Access Violation (0xC0000005) - Memory fault / Audio driver conflict".to_string()
                }
                Language::Portuguese => {
                    "Access Violation (0xC0000005) - Falha de memória / Conflito de driver de áudio".to_string()
                }
            },
            0xC00000FD => "Stack Overflow (0xC00000FD)".to_string(),
            0xC000001D => "Illegal Instruction (0xC000001D)".to_string(),
            0xC0000025 => "Noncontinuable Exception (0xC0000025)".to_string(),
            0xC0000008 => "Invalid Handle (0xC0000008)".to_string(),
            0xC000013A => "Ctrl+C Exit (0xC000013A)".to_string(),
            0x80000003 => "Breakpoint Hit (0x80000003)".to_string(),
            _ => match self.lang {
                Language::English => format!("Code: 0x{:08X}", code),
                Language::Portuguese => format!("Código: 0x{:08X}", code),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i18n_catalog() {
        let en = I18n::new(Language::English);
        let pt = I18n::new(Language::Portuguese);

        assert_eq!(en.status_monitoring_active(), "Monitoring active");
        assert_eq!(pt.status_monitoring_active(), "Monitorando em execução");

        assert!(en.exit_code_diagnosis(0xC0000005).contains("Audio driver"));
        assert!(pt.exit_code_diagnosis(0xC0000005).contains("driver de áudio"));
    }
}
