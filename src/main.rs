mod pdf_parser;
mod code_parser;
mod executor;
mod doc_generator;
mod config;

use anyhow::Result;
use regex;
use std::fs;
use std::io;
use std::path::Path;

fn main() {
    let result = run();
    if let Err(e) = result {
        eprintln!("程序执行失败: {}", e);
    }

    println!("\n按回车键关闭窗口...");
    let mut buf = String::new();
    let _ = io::stdin().read_line(&mut buf);
}

fn run() -> Result<()> {
    println!("C语言实验报告自动化生成系统启动...");
    
    // 自动查找project后跟着序号最大的PDF和C文件
    let (pdf_file, c_file) = find_latest_project_files()?;
    println!("找到PDF文件: {}", pdf_file);
    println!("找到C文件: {}", c_file);
    
    // 1. 解析PDF题目
    println!("正在解析PDF题目...");
    let problems = pdf_parser::parse_pdf_problems(&pdf_file)?;
    println!("成功解析 {} 个题目", problems.len());
    
    // 2. 解析C源码函数
    println!("正在解析C源码...");
    let functions = code_parser::extract_functions(&c_file)?;
    println!("成功解析 {} 个函数", functions.len());
    
    // 3. 读取原始C代码用于执行
    let original_code = fs::read_to_string(&c_file)?;

    // 4. 先生成基础报告（结构 + 题目 + 源码，运行结果先占位）
    println!("正在生成基础报告（题目与源码）...");
    doc_generator::generate_report(problems.clone(), functions.clone())?;
    println!("基础报告已生成，将在执行后回填运行结果...");
    
    // 5. 执行所有函数并捕获输出
    println!("开始执行程序并捕获输出...");
    let execution_results = executor::execute_all_functions(&functions, &original_code)?;
    
    // 6. 回填运行结果并覆盖生成最终Word文档
    println!("正在回填运行结果并生成最终报告...");
    doc_generator::generate_report_with_results(problems, functions, execution_results)?;
    
    println!("报告生成完成！请查看 '实验报告.docx' 文件");
    Ok(())
}

/// 获取exe所在目录
pub fn get_exe_dir() -> Result<std::path::PathBuf> {
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path.parent()
        .ok_or_else(|| anyhow::anyhow!("无法获取exe所在目录"))?;
    Ok(exe_dir.to_path_buf())
}

/// 查找exe同目录下project后跟着序号最大的PDF和C文件
fn find_latest_project_files() -> Result<(String, String)> {
    let exe_dir = get_exe_dir()?;
    let mut pdf_files = Vec::new();
    let mut c_files = Vec::new();
    
    // 遍历exe所在目录
    for entry in fs::read_dir(&exe_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            // 检查PDF文件
            if file_name.to_lowercase().ends_with(".pdf") && file_name.to_lowercase().contains("project") {
                if let Some(number) = extract_project_number(file_name) {
                    pdf_files.push((number, file_name.to_string()));
                }
            }
            
            // 检查C文件
            if file_name.to_lowercase().ends_with(".c") && file_name.to_lowercase().contains("project") {
                if let Some(number) = extract_project_number(file_name) {
                    c_files.push((number, file_name.to_string()));
                }
            }
        }
    }
    
    // 按项目编号排序，取最大的
    pdf_files.sort_by_key(|(num, _)| *num);
    c_files.sort_by_key(|(num, _)| *num);
    
    let latest_pdf = pdf_files.last()
        .map(|(_, name)| name.clone())
        .ok_or_else(|| anyhow::anyhow!("未找到project相关的PDF文件"))?;
    
    let latest_c = c_files.last()
        .map(|(_, name)| name.clone())
        .ok_or_else(|| anyhow::anyhow!("未找到project相关的C文件"))?;
    
    Ok((latest_pdf, latest_c))
}

/// 从文件名中提取project后的数字
fn extract_project_number(file_name: &str) -> Option<u32> {
    let re = regex::Regex::new(r"project(\d+)").unwrap();
    re.captures(&file_name.to_lowercase())
        .and_then(|cap| cap[1].parse().ok())
}