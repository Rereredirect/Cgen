use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone)]
struct ParsedFunction {
    name: String,
    code: String,
}

/// 从C源码文件中提取所有void函数
pub fn extract_functions(file_path: &str) -> Result<HashMap<u32, String>> {
    let content = fs::read_to_string(file_path)?;

    extract_functions_with_helpers(&content)
}

/// 解析所有函数块，并将位于上一个func和下一个func之间的辅助函数归到下一个func。
fn extract_functions_with_helpers(content: &str) -> Result<HashMap<u32, String>> {
    let parsed_functions = extract_all_functions(content);
    let mut functions = HashMap::new();
    let mut pending_helpers: Vec<String> = Vec::new();

    for parsed in parsed_functions {
        if let Some(num) = extract_func_number_by_name(&parsed.name) {
            let mut merged_parts = Vec::new();
            merged_parts.extend(pending_helpers.drain(..));
            merged_parts.push(parsed.code);
            functions.insert(num, merged_parts.join("\n\n"));
        } else {
            pending_helpers.push(parsed.code);
        }
    }

    Ok(functions)
}

/// 按行+大括号平衡提取源码中的所有函数定义（包含func_xx和辅助函数）。
fn extract_all_functions(content: &str) -> Vec<ParsedFunction> {
    let function_start_regex = Regex::new(
        r"^\s*(?:[A-Za-z_][\w\s\*]*?)\s+([A-Za-z_]\w*)\s*\([^;]*\)\s*\{",
    )
    .unwrap();

    let lines: Vec<&str> = content.lines().collect();
    let mut all_functions = Vec::new();
    let mut current_function: Option<(String, Vec<String>)> = None;
    let mut brace_count = 0;

    for line in lines {
        let trimmed = line.trim();

        if current_function.is_none() {
            if let Some(name) = extract_function_name(trimmed, &function_start_regex) {
                current_function = Some((name, vec![line.to_string()]));
                brace_count = count_opening_braces(trimmed) - count_closing_braces(trimmed);

                if brace_count == 0 {
                    if let Some((name, function_lines)) = current_function.take() {
                        all_functions.push(ParsedFunction {
                            name,
                            code: function_lines.join("\n"),
                        });
                    }
                }
            }
            continue;
        }

        if let Some((_, ref mut function_lines)) = current_function {
            function_lines.push(line.to_string());
            brace_count += count_opening_braces(trimmed) - count_closing_braces(trimmed);

            if brace_count == 0 {
                if let Some((name, function_lines)) = current_function.take() {
                    all_functions.push(ParsedFunction {
                        name,
                        code: function_lines.join("\n"),
                    });
                }
            }
        }
    }

    if let Some((name, function_lines)) = current_function {
        all_functions.push(ParsedFunction {
            name,
            code: function_lines.join("\n"),
        });
    }

    all_functions
}

fn extract_function_name(line: &str, function_start_regex: &Regex) -> Option<String> {
    function_start_regex
        .captures(line)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

/// 从函数名中提取func编号
fn extract_func_number_by_name(name: &str) -> Option<u32> {
    let re = Regex::new(r"^func_(\d+)$").unwrap();
    re.captures(name)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_between_funcs_belongs_to_next_func() {
        let source = r#"
void func_02() {
    printf("q2\n");
}

void find(int n, int time) {
    if (time==10) return;
    if (n%3==0||n%7==0) {
        find(n-1, time+1);
        printf("%d%s", n, (time!=10?" ":""));
    } else find(n-1, time);
}

void func_03() { find(1000,0); }

void helper_x() {
    printf("x\n");
}

void func_04() {
    printf("q4\n");
}
"#;

        let extracted = extract_functions_with_helpers(source).expect("extract failed");

        let func_03 = extracted.get(&3).expect("func_03 missing");
        assert!(func_03.contains("void find(int n, int time)"));
        assert!(func_03.contains("void func_03() { find(1000,0); }"));

        let func_04 = extracted.get(&4).expect("func_04 missing");
        assert!(func_04.contains("void helper_x()"));
        assert!(func_04.contains("void func_04()"));
    }
}