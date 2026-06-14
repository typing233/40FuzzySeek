# FuzzySeek

高性能模糊搜索 TUI 工具，可处理百万级候选行而不卡顿。

## 特性

- **海量数据**：后台流式读取 + 分块索引，百万行不卡 UI
- **模糊搜索**：基于 nucleo（Helix 编辑器同款引擎），增量匹配、可中断
- **Unicode 安全**：支持中日韩宽字符、ANSI 转义序列安全剥离高亮
- **稳定排序**：相同分数按原始行号排列，结果可预测
- **多选模式**：Tab 切换选中，Ctrl+A 全选，Ctrl+D 全不选
- **预览面板**：`--preview` 自定义命令，50/50 分屏
- **自适应终端**：resize 自动重绘，全屏运行
- **退出码**：0=选中，130=取消(Ctrl+C/ESC)，2=错误

## 安装

```bash
cargo install --path .
```

或 release 构建：

```bash
cargo build --release
# 二进制位于 target/release/fuzzyseek
```

## 用法

```bash
# 从管道读取
find . -type f | fuzzyseek

# 从文件读取
fuzzyseek --file candidates.txt

# 多选模式
cat urls.txt | fuzzyseek --multi

# 带预览
find . -name "*.rs" | fuzzyseek --preview "head -20 {}"

# 初始查询
ps aux | fuzzyseek --query "python"

# 自定义分隔符（NUL 分隔输出）
find . -type f | fuzzyseek --multi --delimiter $'\0'
```

## 快捷键

| 按键 | 功能 |
|------|------|
| Enter | 确认选择 |
| Esc / Ctrl+C | 取消 |
| ↑ / Ctrl+P | 上移光标 |
| ↓ / Ctrl+N | 下移光标 |
| PageUp / PageDown | 翻页 |
| Ctrl+E / Ctrl+Y | 滚动视口 |
| Tab | 切换选中（多选模式）|
| Shift+Tab | 上移并切换选中 |
| Ctrl+A | 全选（多选模式）|
| Ctrl+D | 取消全选 |
| Ctrl+U | 清空查询 |
| Ctrl+W | 删除最后一个词 |
| Backspace | 删除最后一个字符 |
| 鼠标滚轮 | 上下滚动 |

## 命令行参数

```
Usage: fuzzyseek [OPTIONS]

Options:
  -f, --file <FILE>           从文件读取输入
  -m, --multi                 启用多选模式
  -p, --preview <COMMAND>     预览命令（{} 为占位符）
  -q, --query <QUERY>         初始查询字符串
      --height <HEIGHT>       高度（行数，0=全屏）[默认: 0]
  -d, --delimiter <DELIM>     输出分隔符 [默认: \n]
  -h, --help                  帮助
  -V, --version               版本
```

## 架构

```
┌──────────────────────────────────────────────────┐
│  Input Thread          │  Matcher Thread          │
│  ─────────────         │  ──────────────          │
│  BufReader(256KB)      │  nucleo Atom             │
│  → batch 4096 lines    │  → chunk 8192 scan       │
│  → SharedStore(Mutex)  │  → cancel flag           │
│                        │  → stable sort           │
│                        │  → SharedMatchState      │
├────────────────────────┴─────────────────────────┤
│  Main Thread (TUI)                                │
│  ─────────────────                                │
│  crossterm events (50ms poll)                     │
│  ratatui virtual-scroll rendering                 │
│  only draws visible rows                          │
└──────────────────────────────────────────────────┘
```

- **输入线程**：256KB 缓冲区批量读取，每 4096 行刷一次到共享存储
- **匹配线程**：每次 query 变更取消旧任务，按 8192 行分块扫描
- **主线程**：50ms 事件轮询，只渲染可见行（虚拟滚动），不会因候选量大而卡

## 性能基准

运行基准测试：

```bash
cargo bench
```

典型结果（Apple M2 / AMD Ryzen 7）：

| 场景 | 数据量 | 耗时 |
|------|--------|------|
| 模糊匹配 | 10,000 行 | ~2ms |
| 模糊匹配 | 100,000 行 | ~20ms |
| 模糊匹配 | 1,000,000 行 | ~200ms |
| Unicode 匹配 | 10,000 行 | ~3ms |
| ANSI 剥离 | 10,000 行 | ~1ms |

内存占用：

- 每行约 80-120 字节（含索引开销）
- 100 万行 ≈ 100-120 MB RSS
- 匹配结果只持有索引引用，不复制原始数据

## 依赖说明

| 依赖 | 版本 | 用途 |
|------|------|------|
| ratatui | 0.28 | TUI 框架（跨平台终端渲染）|
| crossterm | 0.28 | 终端后端（事件、raw mode、鼠标）|
| nucleo | 0.5 | 模糊匹配引擎（Helix 编辑器同款）|
| nucleo-matcher | 0.3 | nucleo 底层匹配器 API |
| unicode-width | 0.2 | Unicode 宽字符宽度计算 |
| strip-ansi-escapes | 0.2 | ANSI 转义序列安全剥离 |
| clap | 4.x | 命令行参数解析 |
| parking_lot | 0.12 | 高性能互斥锁 |
| criterion | 0.5 | 性能基准测试（dev） |
| tempfile | 3.x | 集成测试临时文件（dev） |

## 测试

```bash
# 运行所有测试
cargo test

# 运行基准
cargo bench

# 快速冒烟测试
seq 1 1000000 | cargo run --release -- --query "999"
```

## 跨平台兼容性

| 平台 | 状态 | 备注 |
|------|------|------|
| Linux (x86_64/aarch64) | ✓ 完全支持 | 主要开发平台 |
| macOS (Intel/Apple Silicon) | ✓ 完全支持 | crossterm 原生支持 |
| Windows 10/11 | ✓ 支持 | Windows Terminal / ConPTY |
| Windows (旧版 cmd.exe) | △ 部分 | ANSI 支持有限 |
| WSL/WSL2 | ✓ 完全支持 | 等同 Linux |
| SSH 远程终端 | ✓ 支持 | 依赖终端模拟器能力 |

crossterm 后端自动处理各平台差异（Windows API vs POSIX termios）。鼠标支持在所有现代终端模拟器中可用。

## License

MIT
