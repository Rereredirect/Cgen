use anyhow::Result;
use docx_rs::*;
use std::collections::HashMap;
use std::fs::File;

const CJK_TITLE_FONT: &str = "等线 Light (中文标题)";
const TITLE_COLOR: &str = "2F5496";

fn title_fonts() -> RunFonts {
    RunFonts::new()
    .ascii(CJK_TITLE_FONT)
        .east_asia(CJK_TITLE_FONT)
    .cs(CJK_TITLE_FONT)
}

fn to_chinese_number(n: u32) -> String {
    let digits = ["零", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
    let units = ["", "十", "百", "千"];

    if n == 0 {
        return "零".to_string();
    }

    let s = n.to_string();
    let len = s.len();
    let mut out = String::new();
    let mut pending_zero = false;

    for (idx, ch) in s.chars().enumerate() {
        let d = ch.to_digit(10).unwrap_or(0) as usize;
        let unit_idx = len - idx - 1;

        if d == 0 {
            pending_zero = !out.is_empty();
            continue;
        }

        if pending_zero {
            out.push_str("零");
            pending_zero = false;
        }

        if d == 1 && unit_idx == 1 && out.is_empty() {
            out.push_str("十");
        } else {
            out.push_str(digits[d]);
            out.push_str(units[unit_idx]);
        }
    }

    out
}

fn create_doc_with_heading_styles() -> Docx {
    Docx::new()
        .add_style(
            Style::new("Heading1", StyleType::Paragraph)
                .name("heading 1")
                .based_on("Normal")
                .next("Normal")
                .ui_priority(10)
                .outline_lvl(0)
                .fonts(title_fonts())
                .bold()
                .size(48)
                .line_spacing(LineSpacing::new().before(480).after(80))
                .color(TITLE_COLOR),
        )
        .add_style(
            Style::new("Heading2", StyleType::Paragraph)
                .name("heading 2")
                .based_on("Normal")
                .next("Normal")
                .ui_priority(10)
                .outline_lvl(1)
                .fonts(title_fonts())
                .bold()
                .size(40)
                .line_spacing(LineSpacing::new().before(160).after(80))
                .semi_hidden()
                .unhide_when_used()
                .color(TITLE_COLOR),
        )
        .add_style(
            Style::new("Heading3", StyleType::Paragraph)
                .name("heading 3")
                .based_on("Normal")
                .next("Normal")
                .ui_priority(9)
                .outline_lvl(2)
                .bold()
                .size(10)
                .color(TITLE_COLOR),
        )
}

fn add_styled_heading(mut doc: Docx, text: &str, level: u8) -> Docx {
    let style = match level {
        1 => "Heading1",
        2 => "Heading2",
        _ => "Heading3",
    };

    let size = match level {
        1 => 48,
        2 => 40,
        _ => 22,
    };

    doc = doc.add_paragraph(
        Paragraph::new()
            .style(style)
            .keep_next(true)
            .keep_lines(true)
            .line_spacing(match level {
                1 => LineSpacing::new().before(480).after(80),
                2 => LineSpacing::new().before(160).after(80),
                _ => LineSpacing::new().before(80).after(80),
            })
            .add_run(
                Run::new()
                    .add_text(text)
                    .bold()
                    .fonts(title_fonts())
                    .size(size)
                    .color(TITLE_COLOR)
            )
    );

    doc
}

/// 生成完整的实验报告（包含执行结果）
pub fn generate_report_with_results(
    problems: HashMap<u32, String>,
    functions: HashMap<u32, String>,
    execution_results: HashMap<u32, String>,
) -> Result<()> {
    let mut doc = create_doc_with_heading_styles();
    
    // 添加文档标题
    doc = add_document_title(doc);
    
    // 按编号排序处理题目与函数编号并集，避免因编号不完全对齐导致报告空白
    let mut all_nums: Vec<u32> = problems
        .keys()
        .chain(functions.keys())
        .cloned()
        .filter(|&n| n > 0)
        .collect();
    all_nums.sort();
    all_nums.dedup();

    if all_nums.is_empty() {
        doc = add_text_content(doc, "未解析到任何题目或函数，请检查 PDF/C 文件内容与命名格式。");
    }

    for &problem_num in &all_nums {
        let problem_text = problems
            .get(&problem_num)
            .map(String::as_str)
            .unwrap_or("（未解析到该题题目内容）");
        let function_code = functions
            .get(&problem_num)
            .map(String::as_str)
            .unwrap_or("（未解析到该题源程序）");
        let execution_output = execution_results
            .get(&problem_num)
            .map(String::as_str)
            .unwrap_or("");

        // 添加题目部分
        doc = add_problem_section_with_results(doc, problem_num, problem_text, function_code, execution_output);
    }
    
    // 保存文档
    let file = File::create("实验报告.docx")?;
    doc.build().pack(file)?;
    
    Ok(())
}

/// 生成基本的实验报告（不包含执行结果）
pub fn generate_report(
    problems: HashMap<u32, String>,
    functions: HashMap<u32, String>,
) -> Result<()> {
    let empty_results = HashMap::new();
    generate_report_with_results(problems, functions, empty_results)
}

/// 添加文档标题
fn add_document_title(mut doc: Docx) -> Docx {
    doc = doc.add_paragraph(
        Paragraph::new()
            .add_run(
                Run::new()
                    .add_text("C语言实验报告")
                    .bold()
                    .fonts(title_fonts())
                    .size(84)  // 初号
                    .color(TITLE_COLOR)
            )
            .align(AlignmentType::Center)
    );
    
    doc = doc.add_paragraph(
        Paragraph::new()
            .add_run(
                Run::new()
                    .add_text("自动化生成系统")
                    .bold()
                    .fonts(title_fonts())
                    .size(48)
                    .color(TITLE_COLOR)
            )
            .align(AlignmentType::Center)
    );
    
    // 添加空行
    doc = doc.add_paragraph(Paragraph::new());
    
    doc
}

/// 添加单个题目部分（包含执行结果）
fn add_problem_section_with_results(
    mut doc: Docx,
    problem_num: u32,
    problem_text: &str,
    function_code: &str,
    execution_output: &str,
) -> Docx {
    // 标题1：第X题（X 使用中文数字，小一）
    let cn_num = to_chinese_number(problem_num);
    doc = add_styled_heading(doc, &format!("第{}题", cn_num), 1);
    
    // 标题2：题目
    doc = add_styled_heading(doc, "题目", 2);
    
    // 题目内容
    doc = add_text_content(doc, problem_text);
    
    // 标题2：源程序
    doc = add_styled_heading(doc, "源程序", 2);
    
    // 源代码块
    doc = add_code_block(doc, function_code);
    
    // 标题2：运行结果
    doc = add_styled_heading(doc, "运行结果", 2);
    
    // 运行结果内容
    if execution_output.is_empty() {
        doc = add_text_content(doc, "待运行（报告基础结构已生成，执行完成后将自动回填）");
    } else {
        doc = add_code_block(doc, execution_output);
    }
    
    // 标题2：分析
    doc = add_styled_heading(doc, "分析", 2);
    
    // 分析内容预留一个可编辑正文段落，避免光标直接落到分页段
    doc = doc.add_paragraph(
        Paragraph::new()
            .add_run(
                Run::new()
                    .add_text("")
                    .size(22)  // 11号
            )
    );
    
    // // 分页符
    // doc = doc.add_paragraph(
    //     Paragraph::new().page_break_before(true)
    // );
    
    doc
}

/// 添加单个题目部分（不包含执行结果）
fn add_problem_section(
    doc: Docx,
    problem_num: u32,
    problem_text: &str,
    function_code: &str,
) -> Docx {
    add_problem_section_with_results(doc, problem_num, problem_text, function_code, "")
}

/// 添加普通文本内容
fn add_text_content(mut doc: Docx, text: &str) -> Docx {
    if text.is_empty() {
        return doc;
    }
    
    for line in text.lines() {
        if !line.trim().is_empty() {
            doc = doc.add_paragraph(
                Paragraph::new()
                    .add_run(
                        Run::new()
                            .add_text(line)
                            .size(22)  // 11号
                    )
            );
        }
    }
    
    doc
}

/// 添加代码块
fn add_code_block(mut doc: Docx, code: &str) -> Docx {
    for line in code.lines() {
        doc = doc.add_paragraph(
            Paragraph::new()
                .add_run(
                    Run::new()
                        .add_text(line)
                        .size(22)  // 11号
                )
        );
    }
    
    doc
}

/// 更新运行结果（在程序执行后调用）
pub fn update_execution_results(
    mut doc: Docx,
    problem_num: u32,
    execution_output: &str,
) -> Docx {
    // 这里需要实现更新特定题目的运行结果
    // 由于docx-rs的限制，这需要更复杂的文档操作
    // 暂时在生成时直接包含执行结果
    doc
}