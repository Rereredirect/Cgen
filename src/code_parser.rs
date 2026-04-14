use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::fs;

/// 从C源码文件中提取所有void函数
pub fn extract_functions(file_path: &str) -> Result<HashMap<u32, String>> {
    let content = fs::read_to_string(file_path)?;
    
    // 正则表达式匹配函数（包括void和int类型）
    let function_regex = Regex::new(r"(?m)^(void|int)\s+(func_(?P<num>\d+))\s*\((?P<params>.*?)\)\s*\{([\s\S]*?)\n\}").unwrap();
    
    let mut functions = HashMap::new();
    
    for capture in function_regex.captures_iter(&content) {
        let function_num: u32 = capture.name("num").unwrap().as_str().parse()?;
        let full_function = capture.get(0).unwrap().as_str().trim().to_string();
        
        functions.insert(function_num, full_function);
    }
    
    if functions.is_empty() {
        // 如果正则匹配失败，尝试备用方法
        extract_functions_fallback(&content)
    } else {
        Ok(functions)
    }
}

/// 备用方法：按行分析提取函数
fn extract_functions_fallback(content: &str) -> Result<HashMap<u32, String>> {
    let mut functions = HashMap::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut current_function: Option<(u32, Vec<String>)> = None;
    let mut brace_count = 0;
    
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        
        // 检查是否是函数开始
        if trimmed.starts_with("void func_") {
            // 保存前一个函数
            if let Some((num, function_lines)) = current_function.take() {
                let function_code = function_lines.join("\n");
                functions.insert(num, function_code);
            }
            
            // 提取函数编号
            if let Some(num) = extract_function_number(trimmed) {
                current_function = Some((num, vec![trimmed.to_string()]));
                brace_count = count_opening_braces(trimmed) - count_closing_braces(trimmed);
            }
        } else if let Some((_, ref mut function_lines)) = current_function {
            // 添加到当前函数内容
            function_lines.push(line.to_string());
            
            // 更新大括号计数
            brace_count += count_opening_braces(trimmed) - count_closing_braces(trimmed);
            
            // 如果大括号平衡，函数结束
            if brace_count == 0 {
                if let Some((num, function_lines)) = current_function.take() {
                    let function_code = function_lines.join("\n");
                    functions.insert(num, function_code);
                }
            }
        }
    }
    
    // 保存最后一个函数
    if let Some((num, function_lines)) = current_function {
        let function_code = function_lines.join("\n");
        functions.insert(num, function_code);
    }
    
    Ok(functions)
}

/// 从函数声明中提取编号
fn extract_function_number(line: &str) -> Option<u32> {
    let re = Regex::new(r"func_(\d+)").unwrap();
    re.captures(line)
        .and_then(|cap| cap[1].parse().ok())
}

/// 计算字符串中的开大括号数量
fn count_opening_braces(text: &str) -> i32 {
    text.chars().filter(|&c| c == '{').count() as i32
}

/// 计算字符串中的闭大括号数量
fn count_closing_braces(text: &str) -> i32 {
    text.chars().filter(|&c| c == '}').count() as i32
}