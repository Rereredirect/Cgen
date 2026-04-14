use anyhow::Result;
use pdf_extract;
use regex::Regex;
use std::collections::HashMap;

/// 解析PDF文件，提取题目信息
pub fn parse_pdf_problems(pdf_path: &str) -> Result<HashMap<u32, String>> {
    let text = pdf_extract::extract_text(pdf_path)?;

    // 以“行首题号 + 点”作为主策略，避免正文里数字导致截断
    let problems = extract_problems_fallback(&text)?;
    if !problems.is_empty() {
        return Ok(problems);
    }

    // 兜底：简单正则匹配（兼容少数单行场景）
    let problem_regex = Regex::new(r"(?m)^\s*(?P<num>\d+)\.(?P<content>.*)$").unwrap();
    let mut fallback = HashMap::new();

    for capture in problem_regex.captures_iter(&text) {
        let number: u32 = capture.name("num").unwrap().as_str().parse()?;
        let content = capture.name("content").unwrap().as_str();
        let cleaned_content = clean_problem_content(content);
        fallback.insert(number, cleaned_content);
    }

    Ok(fallback)
}

/// 备用方法：按行分割提取题目
fn extract_problems_fallback(text: &str) -> Result<HashMap<u32, String>> {
    let mut problems = HashMap::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut current_problem: Option<(u32, Vec<String>)> = None;
    
    for line in lines {
        let trimmed = line.trim();

        // 检查是否是题目开始（阿拉伯数字格式）
        if let Some((number, first_content)) = extract_problem_header(trimmed) {
            // 保存前一个题目
            if let Some((num, content_lines)) = current_problem.take() {
                let content = clean_problem_content(&content_lines.join("\n"));
                if !content.is_empty() {
                    problems.insert(num, content);
                }
            }

            let mut content_lines = Vec::new();
            if !first_content.is_empty() {
                content_lines.push(first_content);
            }
            current_problem = Some((number, content_lines));
        } else if let Some((_, ref mut content_lines)) = current_problem {
            // 添加到当前题目内容
            if !trimmed.is_empty() {
                content_lines.push(trimmed.to_string());
            }
        }
    }
    
    // 保存最后一个题目
    if let Some((num, content_lines)) = current_problem {
        let content = clean_problem_content(&content_lines.join("\n"));
        if !content.is_empty() {
            problems.insert(num, content);
        }
    }

    Ok(problems)
}

/// 从题目头中提取“题号 + 同行首段内容”
fn extract_problem_header(text: &str) -> Option<(u32, String)> {
    let re = Regex::new(r"^\s*(\d+)\.\s*(.*)$").unwrap();
    let captures = re.captures(text)?;
    let number = captures.get(1)?.as_str().parse().ok()?;
    let first_content = captures
        .get(2)
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_default();

    Some((number, first_content))
}

/// 清理题目内容，移除多余空白行并做行级 trim
fn clean_problem_content(content: &str) -> String {
    content
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}