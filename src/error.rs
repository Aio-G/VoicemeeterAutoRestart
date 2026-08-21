use crate::i18n::I18n;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum VbarError {
    #[error("Instância do aplicativo já em execução no sistema.")]
    AlreadyRunning,

    #[error("Executável não encontrado no disco: {0}")]
    ExecutableNotFound(PathBuf),

    #[error("O caminho informado não é um arquivo executável válido (.exe): {0}")]
    InvalidExecutable(PathBuf),

    #[error("Falha na chamada Win32 '{0}' (código {1}): {2}")]
    Win32Error(String, u32, String),

    #[error("Erro de I/O no arquivo '{0}': {1}")]
    IoError(PathBuf, String),

    #[error("Erro de serialização JSON: {0}")]
    SerializationError(String),

    #[error("Processo PID {0} não pôde ser encerrado.")]
    ProcessTerminationFailed(u32),
}

impl VbarError {
    /// Formata a mensagem de erro traduzida conforme o idioma ativo no catálogo I18n
    pub fn to_localized_string(&self, i18n: &I18n) -> String {
        match self {
            Self::AlreadyRunning => match i18n.lang {
                crate::i18n::Language::English => {
                    "Another instance of the application is already running.".to_string()
                }
                crate::i18n::Language::Portuguese => {
                    "Outra instância do aplicativo já está em execução no sistema.".to_string()
                }
            },
            Self::ExecutableNotFound(path) => match i18n.lang {
                crate::i18n::Language::English => {
                    format!("Executable not found at path: {:?}", path)
                }
                crate::i18n::Language::Portuguese => {
                    format!("Executável não encontrado no caminho: {:?}", path)
                }
            },
            Self::InvalidExecutable(path) => match i18n.lang {
                crate::i18n::Language::English => {
                    format!("The specified path is not a valid .exe executable: {:?}", path)
                }
                crate::i18n::Language::Portuguese => {
                    format!("O caminho especificado não é um arquivo executável .exe válido: {:?}", path)
                }
            },
            Self::Win32Error(api, code, desc) => match i18n.lang {
                crate::i18n::Language::English => {
                    format!("Windows API '{}' failed with code {}: {}", api, code, desc)
                }
                crate::i18n::Language::Portuguese => {
                    format!("Falha na chamada Win32 '{}' (código {}): {}", api, code, desc)
                }
            },
            Self::IoError(path, msg) => match i18n.lang {
                crate::i18n::Language::English => {
                    format!("I/O error accessing file {:?}: {}", path, msg)
                }
                crate::i18n::Language::Portuguese => {
                    format!("Erro de E/S ao acessar o arquivo {:?}: {}", path, msg)
                }
            },
            Self::SerializationError(msg) => match i18n.lang {
                crate::i18n::Language::English => {
                    format!("Configuration serialization error: {}", msg)
                }
                crate::i18n::Language::Portuguese => {
                    format!("Erro de serialização das configurações: {}", msg)
                }
            },
            Self::ProcessTerminationFailed(pid) => match i18n.lang {
                crate::i18n::Language::English => {
                    format!("Could not terminate process with PID {}", pid)
                }
                crate::i18n::Language::Portuguese => {
                    format!("Não foi possível encerrar o processo com PID {}", pid)
                }
            },
        }
    }
}
