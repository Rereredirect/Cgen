use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

fn normalize_mingw_root(path: &str) -> String {
    let trimmed = path.trim_end_matches(['\\', '/']);
    if trimmed.ends_with("\\bin") || trimmed.ends_with("/bin") {
        Path::new(trimmed)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| trimmed.to_string())
    } else {
        trimmed.to_string()
    }
}

/// 配置文件结构
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub mingw_path: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mingw_path: None,
        }
    }
}


/// 获取exe所在目录
fn get_exe_dir() -> Result<std::path::PathBuf> {
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path.parent()
        .ok_or_else(|| anyhow::anyhow!("无法获取exe所在目录"))?;
    Ok(exe_dir.to_path_buf())
}

/// 加载或创建配置文件
pub fn load_or_create_config() -> Result<Config> {
    let exe_dir = get_exe_dir()?;
    let config_path = exe_dir.join("config.json");
    
    if config_path.exists() {
        // 配置文件存在，读取它
        let config_content = fs::read_to_string(&config_path)?;
        let mut config: Config = serde_json::from_str(&config_content)?;

        // 将 mingw_path 规范化为 MinGW 根目录，避免把 bin 目录写入配置
        if let Some(path) = config.mingw_path.clone() {
            let normalized = normalize_mingw_root(&path);
            if normalized != path {
                config.mingw_path = Some(normalized);
                let updated_content = serde_json::to_string_pretty(&config)?;
                fs::write(&config_path, updated_content)?;
            }
        }

        Ok(config)
    } else {
        // 配置文件不存在，创建默认配置
        let config = Config::default();
        let config_content = serde_json::to_string_pretty(&config)?;
        fs::write(&config_path, config_content)?;
        println!("已创建默认配置文件: {}", config_path.display());
        Ok(config)
    }
}

/// 保存配置到文件
pub fn save_config(config: &Config) -> Result<()> {
    let exe_dir = get_exe_dir()?;
    let config_path = exe_dir.join("config.json");
    let config_content = serde_json::to_string_pretty(config)?;
    fs::write(&config_path, config_content)?;
    Ok(())
}

/// 验证MINGW路径是否有效
pub fn validate_mingw_path(path: &str) -> bool {
    let normalized = normalize_mingw_root(path);

    // 检查路径是否已经是bin目录
    let gcc_path = if cfg!(windows) {
        if normalized.ends_with("\\bin") {
            format!("{}\\gcc.exe", normalized)
        } else {
            format!("{}\\bin\\gcc.exe", normalized)
        }
    } else {
        if normalized.ends_with("/bin") {
            format!("{}/gcc", normalized)
        } else {
            format!("{}/bin/gcc", normalized)
        }
    };
    
    Path::new(&gcc_path).exists()
}

/// 自动查找MINGW路径
pub fn find_mingw_path() -> Option<String> {
    // 常见的MINGW安装位置
    let possible_paths = if cfg!(windows) {
        vec![
            "C:\\msys64\\mingw64",
            "C:\\msys64\\mingw32",
            "C:\\mingw64",
            "C:\\mingw32",
            "D:\\msys64\\mingw64",
            "D:\\msys64\\mingw32",
            "D:\\mingw64",
            "D:\\mingw32",
        ]
    } else {
        vec![
            "/usr/bin",
            "/usr/local/bin",
            "/opt/local/bin",
        ]
    };
    
    for path in possible_paths {
        if validate_mingw_path(path) {
            return Some(path.to_string());
        }
    }
    
    None
}