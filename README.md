# Cgen - C语言实验报告自动化生成工具

一个用 Rust 编写的命令行工具，能够自动从 PDF 题目文件和 C 源码文件中提取信息，编译并运行每道题的函数，最终生成格式化的 Word 实验报告（`.docx`）。

## 功能概述

1. 自动扫描可执行文件同目录下序号最大的 `projectN.pdf` 和 `projectN.c` 文件
2. 从 PDF 中解析题目内容（按 `1. 2. 3. ...` 格式识别）
3. 从 C 源码中提取 `func_01`、`func_02` 等命名的函数
4. 逐个编译并执行每个函数，捕获输出（支持交互式输入的题目）
5. 生成包含 **题目、源程序、运行结果、分析** 四部分的 Word 实验报告

## 使用前提

- 系统中需安装 C 编译器（GCC / Clang / MSVC 均可）
- Windows 用户推荐安装 [MSYS2 MinGW](https://www.msys2.org/)，工具会自动在常见路径下查找

## 文件命名约定

将以下文件放在 `cgen.exe` 同一目录下：

| 文件 | 命名格式 | 示例 |
|------|----------|------|
| 题目 PDF | `project<序号>.pdf` | `project3.pdf` |
| C 源码 | `project<序号>.c` | `project3.c` |

程序会自动选取序号最大的一组文件。

## C 源码编写规范

每道题对应一个函数，命名为 `func_<题号>`：

```c
#include <stdio.h>

void func_01() {
    printf("Hello World\n");
}

void func_02() {
    int a;
    scanf("%d", &a);
    printf("%d\n", a * 2);
}

int main() {
    // 可留空或自行调用，工具会自动逐个调用每个 func_ （对于非void的函数，返回值会被丢弃）
    return 0;
}
```

- 支持 `void` 和 `int` 返回类型
- 函数编号支持前导零（如 `func_01`）
- 包含 `scanf` / `fgets` 等输入函数的题目会进入交互模式，手动输入后自动记录

## 配置文件

首次运行时会在可执行文件同目录下生成 `config.json`：

```json
{
  "mingw_path": null
}
```

如果自动检测编译器失败，可手动填写 MinGW 安装根目录（如 `C:\\msys64\\mingw64`）。

## 运行方式

```bash
# 编译
cargo build --release

# 将生成的 cgen.exe 与 projectN.pdf、projectN.c 放在同一目录，双击运行即可
```

运行完成后，会在同目录下生成 `实验报告.docx`。

## 生成报告结构

```
C语言实验报告
├── 第一题
│   ├── 题目      （从 PDF 提取）
│   ├── 源程序    （从 .c 文件提取对应函数）
│   ├── 运行结果  （自动编译执行后捕获的输出）
│   └── 分析      （留空，供手动填写）
├── 第二题
│   └── ...
└── ...
```

## 依赖

| crate | 用途 |
|-------|------|
| `pdf-extract` | PDF 文本提取 |
| `docx-rs` | Word 文档生成 |
| `regex` | 正则匹配题号与函数 |
| `tempfile` | 编译临时文件管理 |
| `crossterm` | 交互式输入的终端控制 |
| `serde` / `serde_json` | 配置文件序列化 |
| `anyhow` | 错误处理 |

## License

MIT
