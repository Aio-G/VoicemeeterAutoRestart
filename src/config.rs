use crate::error::VbarError;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_FILENAME: &str = "config.json";
pub const DEFAULT_PROCESS_NAME: &str = "voicemeeter8x64.exe";
pub const DEFAULT_CHECK_INTERVAL: f32 = 3.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub language: String,
    pub process_name: String,
    pub process_path: String,
    pub check_interval_secs: f32,
    pub start_minimized: bool,
    #[serde(default)]
    pub autostart_with_windows: bool,
    pub crash_protection_enabled: bool,
    pub max_consecutive_crashes: u32,
}

/// Detecta dinamicamente caminhos de instalação do Voicemeeter no Windows em múltiplos diretórios e drives
pub fn detect_voicemeeter_installation() -> Option<(String, String)> {
    let candidates = [
        ("voicemeeter8x64.exe", "VB\\Voicemeeter\\voicemeeter8x64.exe"),
        ("voicemeeterpro64.exe", "VB\\Voicemeeter\\voicemeeterpro64.exe"),
        ("voicemeeter8.exe", "VB\\Voicemeeter\\voicemeeter8.exe"),
        ("voicemeeterpro.exe", "VB\\Voicemeeter\\voicemeeterpro.exe"),
        ("voicemeeter.exe", "VB\\Voicemeeter\\voicemeeter.exe"),
    ];

    let mut search_dirs = Vec::new();
    if let Ok(pfx86) = env::var("ProgramFiles(x86)") {
        search_dirs.push(PathBuf::from(pfx86));
    }
    if let Ok(pf) = env::var("ProgramFiles") {
        search_dirs.push(PathBuf::from(pf));
    }
    if let Ok(pfw64) = env::var("ProgramW6432") {
        search_dirs.push(PathBuf::from(pfw64));
    }
    if let Ok(sysdrive) = env::var("SystemDrive") {
        search_dirs.push(PathBuf::from(format!("{}\\Program Files (x86)", sysdrive)));
        search_dirs.push(PathBuf::from(format!("{}\\Program Files", sysdrive)));
        search_dirs.push(PathBuf::from(format!("{}\\VB-Audio", sysdrive)));
    }
    if let Ok(local_app) = env::var("LOCALAPPDATA") {
        search_dirs.push(PathBuf::from(local_app).join("Programs"));
    }
    search_dirs.push(PathBuf::from("C:\\Program Files (x86)"));
    search_dirs.push(PathBuf::from("C:\\Program Files"));
    search_dirs.push(PathBuf::from("D:\\Program Files (x86)"));
    search_dirs.push(PathBuf::from("D:\\Program Files"));

    for dir in &search_dirs {
        for (exe_name, rel_path) in &candidates {
            let full_path = dir.join(rel_path);
            if full_path.exists() && full_path.is_file() {
                return Some((exe_name.to_string(), full_path.to_string_lossy().to_string()));
            }
        }
    }

    None
}

impl Default for AppConfig {
    fn default() -> Self {
        let (proc_name, proc_path) = match detect_voicemeeter_installation() {
            Some((name, path)) => (name, path),
            None => {
                let default_dir = env::var("ProgramFiles(x86)")
                    .unwrap_or_else(|_| "C:\\Program Files (x86)".to_string());
                let default_path = Path::new(&default_dir)
                    .join("VB\\Voicemeeter\\voicemeeter8x64.exe")
                    .to_string_lossy()
                    .to_string();
                (DEFAULT_PROCESS_NAME.to_string(), default_path)
            }
        };

        Self {
            language: "en".to_string(), // English como padrão
            process_name: proc_name,
            process_path: proc_path,
            check_interval_secs: DEFAULT_CHECK_INTERVAL,
            start_minimized: false,
            autostart_with_windows: false,
            crash_protection_enabled: true,
            max_consecutive_crashes: 3,
        }
    }
}

impl AppConfig {
    /// Obtém o caminho do arquivo de configuração na pasta de dados do usuário (%APPDATA%\VBAR\config.json)
    pub fn config_path() -> PathBuf {
        if let Ok(app_data) = env::var("APPDATA") {
            let dir = PathBuf::from(app_data).join("VBAR");
            let _ = fs::create_dir_all(&dir);
            return dir.join(CONFIG_FILENAME);
        }
        if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
            let dir = PathBuf::from(local_app_data).join("VBAR");
            let _ = fs::create_dir_all(&dir);
            return dir.join(CONFIG_FILENAME);
        }
        if let Ok(user_profile) = env::var("USERPROFILE") {
            let dir = PathBuf::from(user_profile).join(".vbar");
            let _ = fs::create_dir_all(&dir);
            return dir.join(CONFIG_FILENAME);
        }
        PathBuf::from(CONFIG_FILENAME)
    }

    /// Carrega as configurações do disco, sanitiza os valores e retorna a struct válida.
    pub fn load() -> Self {
        let path = Self::config_path();

        // 1. Tenta carregar da pasta AppData do usuário
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut cfg) = serde_json::from_str::<AppConfig>(&content) {
                    cfg.sanitize();
                    return cfg;
                }
            }
        }

        // 2. Migração retrocompatível (caso exista vbar_config.json na pasta do executável)
        if let Ok(exe) = env::current_exe() {
            if let Some(parent) = exe.parent() {
                let old_path = parent.join("vbar_config.json");
                if old_path.exists() {
                    if let Ok(content) = fs::read_to_string(&old_path) {
                        if let Ok(mut cfg) = serde_json::from_str::<AppConfig>(&content) {
                            cfg.sanitize();
                            let _ = cfg.save();
                            return cfg;
                        }
                    }
                }
            }
        }

        let mut def = Self::default();
        def.sanitize();
        let _ = def.save();
        def
    }

    /// Sanitiza os campos: valida idioma (padrão 'en'), limites de intervalo e caminhos.
    pub fn sanitize(&mut self) {
        if self.language != "pt" && self.language != "en" {
            self.language = "en".to_string();
        }

        // Se o caminho atual não existir ou estiver vazio, tenta autodetectar
        if self.process_path.trim().is_empty() || !Path::new(&self.process_path).exists() {
            if let Some((detected_name, detected_path)) = detect_voicemeeter_installation() {
                self.process_name = detected_name;
                self.process_path = detected_path;
            }
        }

        if self.process_name.trim().is_empty() {
            self.process_name = DEFAULT_PROCESS_NAME.to_string();
        }

        // Limita o intervalo de checagem entre 0.1s e 60.0s
        self.check_interval_secs = self.check_interval_secs.clamp(0.1, 60.0);

        if self.max_consecutive_crashes == 0 {
            self.max_consecutive_crashes = 3;
        }
    }

    /// Salva as configurações em arquivo JSON com tratamento tipado de erros
    pub fn save(&self) -> Result<(), VbarError> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| VbarError::IoError(parent.to_path_buf(), e.to_string()))?;
        }
        let mut copy = self.clone();
        copy.sanitize();
        let json_str = serde_json::to_string_pretty(&copy)
            .map_err(|e| VbarError::SerializationError(e.to_string()))?;
        fs::write(&path, json_str)
            .map_err(|e| VbarError::IoError(path, e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_sanitize_bounds() {
        let mut cfg = AppConfig {
            language: "es".to_string(),
            process_name: "   ".to_string(),
            process_path: "".to_string(),
            check_interval_secs: 999.0,
            start_minimized: false,
            autostart_with_windows: false,
            crash_protection_enabled: true,
            max_consecutive_crashes: 0,
        };
        cfg.sanitize();

        assert_eq!(cfg.language, "en");
        assert!(!cfg.process_name.is_empty());
        assert!(!cfg.process_path.is_empty());
        assert_eq!(cfg.check_interval_secs, 60.0);
        assert_eq!(cfg.max_consecutive_crashes, 3);

        cfg.check_interval_secs = -5.0;
        cfg.sanitize();
        assert_eq!(cfg.check_interval_secs, 0.1);
    }

    #[test]
    fn test_config_path_in_appdata() {
        let path = AppConfig::config_path();
        assert!(path.to_string_lossy().contains("VBAR") || path.to_string_lossy().contains(".vbar"));
    }
}
