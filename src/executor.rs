use anyhow::{Result, anyhow};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::{Builder, NamedTempFile};

use crate::config::{load_or_create_config, validate_mingw_path, find_mingw_path};

fn append_path_prefix(cmd: &mut Command, path_prefix: &str) {
    let current_path = std::env::var("PATH").unwrap_or_default();
    let injected_path = if current_path.is_empty() {
        path_prefix.to_string()
    } else {
        format!("{};{}", path_prefix, current_path)
    };
    cmd.env("PATH", injected_path);
}

fn get_configured_mingw_bin() -> Option<String> {
    if !cfg!(windows) {
        return None;
    }

    let config = load_or_create_config().ok()?;
    let mingw_root = config.mingw_path?;
    let mingw_bin = if mingw_root.ends_with("\\bin") {
        mingw_root
    } else {
        format!("{}\\bin", mingw_root)
    };

    if Path::new(&mingw_bin).exists() {
        Some(mingw_bin)
    } else {
        None
    }
}

fn resolve_function_name(function_num: u32, original_code: &str) -> String {
    // 支持 func_01 / func_1 / func_001 等命名形式
    let pattern = format!(r"\b(func_0*{})\s*\(", function_num);
    if let Ok(re) = Regex::new(&pattern) {
        if let Some(caps) = re.captures(original_code) {
            if let Some(m) = caps.get(1) {
                return m.as_str().to_string();
            }
        }
    }

    format!("func_{}", function_num)
}

fn function_requires_input(function_code: &str) -> bool {
    let lower = function_code.to_lowercase();
    lower.contains("scanf(")
        || lower.contains("scanf_s(")
        || lower.contains("fgets(")
        || lower.contains("getchar(")
        || lower.contains("gets(")
}

fn create_timestamp_temp_dir() -> Result<PathBuf> {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let dir = std::env::current_dir()?.join(format!("{}_temp", ts));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

struct RawModeGuard;

impl RawModeGuard {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

/// 检测系统中可用的C编译器
fn detect_c_compiler() -> Option<String> {
    // 1. 首先检查配置文件中的MINGW路径
    if let Ok(config) = load_or_create_config() {
        if let Some(mingw_path) = &config.mingw_path {
            if validate_mingw_path(mingw_path) {
                let gcc_path = if cfg!(windows) {
                    if mingw_path.ends_with("\\bin") {
                        format!("{}\\gcc.exe", mingw_path)
                    } else {
                        format!("{}\\bin\\gcc.exe", mingw_path)
                    }
                } else {
                    if mingw_path.ends_with("/bin") {
                        format!("{}/gcc", mingw_path)
                    } else {
                        format!("{}/bin/gcc", mingw_path)
                    }
                };
                println!("使用配置文件中的MINGW路径: {}", mingw_path);
                return Some(gcc_path);
            } else {
                println!("配置文件中的MINGW路径无效: {}", mingw_path);
            }
        }
    }
    
    // 2. 尝试自动查找MINGW
    if let Some(mingw_path) = find_mingw_path() {
        let gcc_path = if cfg!(windows) {
            if mingw_path.ends_with("\\bin") {
                format!("{}\\gcc.exe", mingw_path)
            } else {
                format!("{}\\bin\\gcc.exe", mingw_path)
            }
        } else {
            if mingw_path.ends_with("/bin") {
                format!("{}/gcc", mingw_path)
            } else {
                format!("{}/bin/gcc", mingw_path)
            }
        };
        println!("自动找到MINGW路径: {}", mingw_path);
        return Some(gcc_path);
    }
    
    // 3. 尝试常见的C编译器 - 优先尝试clang避免DLL依赖
    let compilers = ["clang", "gcc", "cc", "cl"];
    
    for compiler in &compilers {
        if Command::new(compiler)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
        {
            println!("找到系统C编译器: {}", compiler);
            return Some(compiler.to_string());
        }
    }
    
    // 4. 在Windows上，尝试查找Visual Studio的cl编译器
    #[cfg(windows)]
    {
        if let Some(vs_path) = find_visual_studio() {
            let cl_path = format!("{}\\cl.exe", vs_path);
            println!("找到Visual Studio编译器: {}", cl_path);
            return Some(cl_path);
        }
    }
    
    None
}

/// 在Windows上查找Visual Studio安装路径
#[cfg(windows)]
fn find_visual_studio() -> Option<String> {
    use std::process::Command;
    
    // 尝试通过vswhere查找Visual Studio
    let vswhere_output = Command::new("vswhere")
        .args(&["-latest", "-property", "installationPath"])
        .output()
        .ok()?;
    
    if vswhere_output.status.success() {
        let vs_path = String::from_utf8_lossy(&vswhere_output.stdout).trim().to_string();
        if !vs_path.is_empty() {
            // 查找VC工具目录
            let vc_tools_path = format!("{}\\VC\\Tools\\MSVC", vs_path);
            if std::path::Path::new(&vc_tools_path).exists() {
                // 查找最新的工具版本
                if let Ok(entries) = std::fs::read_dir(&vc_tools_path) {
                    let mut versions: Vec<String> = entries
                        .filter_map(|entry| entry.ok())
                        .filter(|entry| entry.path().is_dir())
                        .filter_map(|entry| {
                            entry.file_name().to_str().map(|s| s.to_string())
                        })
                        .collect();
                    
                    versions.sort();
                    if let Some(latest_version) = versions.last() {
                        let cl_path = format!("{}\\{}\\bin\\Hostx64\\x64", vc_tools_path, latest_version);
                        if std::path::Path::new(&cl_path).exists() {
                            return Some(cl_path);
                        }
                    }
                }
            }
        }
    }
    
    None
}

/// 执行单个函数并捕获输出
pub fn execute_function(function_num: u32, function_code: &str, original_code: &str, temp_dir: &Path) -> Result<String> {
    let need_input = function_requires_input(function_code);
    if need_input {
        println!("正在运行第 {} 题（检测到输入函数：输入会实时转发并记录，方法结束后自动进入下一题）...", function_num);
    } else {
        println!("正在运行第 {} 题（自动执行，非交互模式）...", function_num);
    }
    let function_name = resolve_function_name(function_num, original_code);
    let binary_name = if cfg!(windows) {
        format!("temp_program_{}.exe", function_num)
    } else {
        format!("temp_program_{}", function_num)
    };
    let binary_path = temp_dir.join(binary_name);
    let binary_path_str = binary_path.to_string_lossy().to_string();
    
    // 创建临时C文件
    let temp_file: NamedTempFile = Builder::new().suffix(".c").tempfile()?;
    let temp_path = temp_file.path().to_str().unwrap().to_string();
    
    // 构建完整的临时程序
    let temp_program = format!(
        "#define main __reportgen_original_main\n{}\n#undef main\n\nint main() {{\n    {}();\n    return 0;\n}}",
        original_code, function_name
    );
    
    // 写入临时文件
    fs::write(&temp_path, &temp_program)?;
    
    // 检测C编译器
    let compiler = detect_c_compiler()
        .ok_or_else(|| anyhow!("未找到可用的C编译器。请安装gcc、clang或Visual Studio"))?;
    
    fn mingw_bin_from_compiler(compiler: &str) -> Option<String> {
        if !cfg!(windows) {
            return None;
        }

        let compiler_path = Path::new(compiler);
        let file_name = compiler_path.file_name()?.to_string_lossy().to_ascii_lowercase();
        if file_name != "gcc.exe" {
            return None;
        }

        let bin_dir = compiler_path.parent()?;
        let dll_path = bin_dir.join("libwinpthread-1.dll");
        if dll_path.exists() {
            Some(bin_dir.to_string_lossy().to_string())
        } else {
            None
        }
    }

    println!("使用编译器: {}", compiler);

    let mingw_bin_for_env = mingw_bin_from_compiler(&compiler).or_else(get_configured_mingw_bin);
    
    // 编译C程序
    let compile_result = if compiler.ends_with("cl.exe") {
        // Visual Studio编译器参数
        let mut compile_cmd = Command::new(&compiler);
        let cl_out_arg = format!("/Fe:{}", binary_path_str);
        compile_cmd.args(&[&cl_out_arg, &temp_path]);
        if let Some(mingw_bin) = &mingw_bin_for_env {
            append_path_prefix(&mut compile_cmd, mingw_bin);
        }
        compile_cmd.output()?
    } else {
        // GCC/Clang编译器参数 - 使用静态链接避免DLL依赖
        let mut compile_cmd = Command::new(&compiler);
        compile_cmd.args(&[
            "-x",
            "c",
            "-std=gnu11",
            "-Wno-error=implicit-function-declaration",
            &temp_path,
            "-o",
            &binary_path_str,
            "-static",
        ]);
        if let Some(mingw_bin) = &mingw_bin_for_env {
            append_path_prefix(&mut compile_cmd, mingw_bin);
        }
        compile_cmd.output()?
    };
    
    if !compile_result.status.success() {
        let error_output = String::from_utf8_lossy(&compile_result.stderr);
        let stdout_output = String::from_utf8_lossy(&compile_result.stdout);
        println!("编译错误输出: {}", error_output);
        println!("编译标准输出: {}", stdout_output);
        return Err(anyhow!("编译失败: {}", error_output));
    }
    
    // 执行程序并捕获输出
    let mut run_cmd = Command::new(&binary_path);
    if need_input {
        run_cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    } else {
        run_cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    }

    // Windows 下如果使用 MinGW gcc，运行时把其 bin 注入 PATH，避免找不到 libwinpthread-1.dll
    if let Some(mingw_bin) = mingw_bin_for_env {
        append_path_prefix(&mut run_cmd, &mingw_bin);
    }

    let mut captured_input = String::new();
    let output = if need_input {
        let mut child = run_cmd.spawn()?;
        let _raw_guard = RawModeGuard::new()?;
        let mut line_buf = String::new();

        let status = 'input_loop: loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }

            if event::poll(Duration::from_millis(40))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            let _ = child.stdin.take();
                            println!();
                        }
                        KeyCode::Enter => {
                            println!();
                            line_buf.push('\n');

                            if let Some(stdin) = child.stdin.as_mut() {
                                if let Err(e) = stdin.write_all(line_buf.as_bytes()) {
                                    if e.kind() == ErrorKind::BrokenPipe {
                                        break 'input_loop child.wait()?;
                                    }
                                    return Err(e.into());
                                }
                                if let Err(e) = stdin.flush() {
                                    if e.kind() == ErrorKind::BrokenPipe {
                                        break 'input_loop child.wait()?;
                                    }
                                    return Err(e.into());
                                }
                            }

                            captured_input.push_str(&line_buf);
                            line_buf.clear();
                        }
                        KeyCode::Backspace => {
                            if line_buf.pop().is_some() {
                                print!("\u{8} \u{8}");
                                io::stdout().flush()?;
                            }
                        }
                        KeyCode::Tab => {
                            line_buf.push('\t');
                            print!("\t");
                            io::stdout().flush()?;
                        }
                        KeyCode::Char(c) => {
                            line_buf.push(c);
                            print!("{}", c);
                            io::stdout().flush()?;
                        }
                        _ => {}
                    }
                }
            }
        };

        let _ = child.stdin.take();

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        if let Some(mut s) = child.stdout.take() {
            s.read_to_end(&mut stdout)?;
        }
        if let Some(mut s) = child.stderr.take() {
            s.read_to_end(&mut stderr)?;
        }

        Output { status, stdout, stderr }
    } else {
        run_cmd.output()?
    };
    
    // 清理临时文件
    let _ = fs::remove_file(&binary_path);
    
    // 处理输出
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    let mut result = String::new();

    if need_input {
        result.push_str("输入：\n");
        let input_text = captured_input.trim_end();
        if input_text.is_empty() {
            result.push_str("(无)\n");
        } else {
            result.push_str(input_text);
            result.push('\n');
        }
        result.push_str("输出：\n");
        if !stdout.trim().is_empty() {
            result.push_str(stdout.trim_end());
        } else {
            result.push_str("(无)");
        }
        if !stderr.trim().is_empty() {
            result.push_str("\n错误输出：\n");
            result.push_str(stderr.trim_end());
        }
    } else {
        if !stdout.trim().is_empty() {
            result.push_str(&stdout);
        }
        
        if !stderr.trim().is_empty() {
            if !result.is_empty() {
                result.push_str("\n");
            }
            result.push_str(&format!("错误输出: {}", stderr));
        }
        
        if result.trim().is_empty() {
            result = "程序执行完成，但无输出".to_string();
        }
    }
    
    Ok(result.trim().to_string())
}

/// 批量执行所有函数
pub fn execute_all_functions(functions: &HashMap<u32, String>, original_code: &str) -> Result<HashMap<u32, String>> {
    let mut results = HashMap::new();
    let temp_dir = create_timestamp_temp_dir()?;
    
    // 按编号排序执行
    let mut function_nums: Vec<u32> = functions.keys().cloned().filter(|&n| n > 0).collect();
    function_nums.sort();
    
    for &function_num in &function_nums {
        if let Some(function_code) = functions.get(&function_num) {
            println!("\n=== 执行第 {} 题 ===", function_num);
            
            match execute_function(function_num, function_code, original_code, &temp_dir) {
                Ok(output) => {
                    results.insert(function_num, output);
                    println!("第 {} 题执行完成", function_num);
                }
                Err(e) => {
                    eprintln!("第 {} 题执行失败: {}", function_num, e);
                    results.insert(function_num, format!("执行失败: {}", e));
                }
            }
        }
    }

    if let Err(e) = fs::remove_dir_all(&temp_dir) {
        eprintln!("清理临时目录失败 {}: {}", temp_dir.display(), e);
    }
    
    Ok(results)
}