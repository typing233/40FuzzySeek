# FuzzySeek

高性能模糊搜索 TUI 工具，可处理百万级候选行而不卡顿。

## 特性

- **海量数据**：后台流式读取 + 分块存储（Arc\<str\> chunked），百万行不卡 UI
- **低内存设计**：数据按 16384 行分块存储，匹配结果只持有索引，不复制原始字符串
- **模糊搜索**：基于 nucleo（Helix 编辑器同款引擎），增量匹配、可中断
- **Unicode 安全**：unicode-width 计算光标位置，中日韩宽字符正确显示
- **ANSI 安全**：带颜色输入不破坏界面，strip-ansi-escapes 剥离后再匹配和高亮
- **稳定排序**：相同分数按原始行号排列，结果可预测
- **全屏/半屏**：`--height N` 指定行数以内联模式运行，不占满终端
- **多选模式**：Tab 切换选中，Ctrl+A 全选，Ctrl+D 全不选
- **预览面板**：`--preview` 非阻塞异步执行，慢命令不卡 UI
- **自定义快捷键**：`--bind` 参数 + 配置文件，可重映射所有动作
- **自适应终端**：resize 自动重绘
- **退出码**：0=选中，130=取消(Ctrl+C/ESC)，2=错误（附明确错误信息）

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
# 从管道读取（全屏）
find . -type f | fuzzyseek

# 从文件读取
fuzzyseek --file candidates.txt

# 半屏模式（只占 15 行，不进入 alternate screen）
find . -type f | fuzzyseek --height 15

# 多选模式
cat urls.txt | fuzzyseek --multi

# 带预览（非阻塞，异步执行）
find . -name "*.rs" | fuzzyseek --preview "head -20 {}"

# 初始查询
ps aux | fuzzyseek --query "python"

# 自定义分隔符（NUL 分隔输出）
find . -type f | fuzzyseek --multi --delimiter $'\0'

# 自定义快捷键
fuzzyseek --file list.txt --bind "confirm:ctrl-y,cancel:ctrl-q"
```

## 快捷键

默认绑定（均可通过 `--bind` 或配置文件自定义）：

| 按键 | 动作名 | 功能 |
|------|--------|------|
| Enter | confirm | 确认选择 |
| Esc / Ctrl+C | cancel | 取消 |
| ↑ / Ctrl+P | up | 上移光标 |
| ↓ / Ctrl+N | down | 下移光标 |
| PageUp | page-up | 上翻页 |
| PageDown | page-down | 下翻页 |
| Ctrl+E | scroll-down | 滚动视口下 |
| Ctrl+Y | scroll-up | 滚动视口上 |
| Tab | toggle | 切换选中（多选模式）|
| Ctrl+A | select-all | 全选（多选模式）|
| Ctrl+D | deselect-all | 取消全选 |
| Ctrl+U | clear-query | 清空查询 |
| Ctrl+W | delete-word | 删除最后一个词 |
| Backspace | backspace | 删除最后一个字符 |
| 鼠标滚轮 | — | 上下滚动 |

## 自定义快捷键

### 通过 --bind 参数

格式：`action:key`，多个用逗号分隔：

```bash
fuzzyseek --bind "confirm:ctrl-y,cancel:ctrl-q,up:ctrl-k,down:ctrl-j"
```

### 通过配置文件

路径：`~/.config/fuzzyseek/config.toml`

```toml
[keys]
confirm = "ctrl-m"
cancel = "ctrl-g"
up = "ctrl-k"
down = "ctrl-j"
toggle = "ctrl-space"
select-all = "alt-a"
```

### 可用的键名

修饰符：`ctrl-`、`alt-`、`shift-`  
键名：`enter`/`return`/`cr`、`esc`、`tab`、`btab`(Shift+Tab)、`bs`/`backspace`、
`del`/`delete`、`up`、`down`、`left`、`right`、`home`、`end`、`pgup`、`pgdn`、`space`、
单字符如 `a`-`z`、`0`-`9`

### 可绑定的动作

`confirm`、`cancel`、`up`、`down`、`page-up`、`page-down`、`toggle`、
`select-all`、`deselect-all`、`backspace`、`clear-query`、`delete-word`、
`scroll-up`、`scroll-down`

## 命令行参数

```
Usage: fuzzyseek [OPTIONS]

Options:
  -f, --file <FILE>           从文件读取输入
  -m, --multi                 启用多选模式
  -p, --preview <COMMAND>     预览命令（{} 为占位符，异步执行不阻塞 UI）
  -q, --query <QUERY>         初始查询字符串
      --height <HEIGHT>       高度行数（0=全屏，>0 = 内联半屏）[默认: 0]
  -d, --delimiter <DELIM>     输出分隔符 [默认: \n]
      --bind <BIND>           自定义快捷键（action:key,action:key）
  -h, --help                  帮助
  -V, --version               版本
```

## 架构

```
┌──────────────────────────────────────────────────────┐
│  Input Thread           │  Matcher Thread             │
│  ─────────────          │  ──────────────             │
│  BufReader(256KB)       │  nucleo Atom                │
│  → chunked Arc<str>     │  → 8192-item scan chunks    │
│    (16384 per chunk)    │  → AtomicBool cancel        │
│  → SharedStore(RwLock)  │  → swap-buffer update       │
│                         │  → stable sort (score,idx)  │
│                         │  → SharedMatchState(RwLock) │
├─────────────────────────┴────────────────────────────┤
│  Preview Thread (optional)                            │
│  ─────────────────────────                            │
│  async Command execution, generation counter          │
│  only latest request displayed, old cancelled         │
├──────────────────────────────────────────────────────┤
│  Main Thread (TUI)                                    │
│  ─────────────────                                    │
│  crossterm events (50ms poll)                         │
│  ratatui rendering (Fullscreen or Inline viewport)    │
│  only draws visible rows (virtual scrolling)          │
│  unicode-width cursor positioning                     │
│  ANSI-stripped highlight indexing                      │
└──────────────────────────────────────────────────────┘
```

### 低内存设计

- **分块存储**：输入按 16384 行一个 Chunk 存储，避免单个 Vec 的巨型 realloc
- **Arc\<str\> 共享**：每行是 Arc\<str\>，读取线程和匹配线程零拷贝共享
- **索引级结果**：MatchResult 只含 `(index, score, positions)`，不持有字符串
- **Swap-buffer**：匹配线程与 UI 之间通过 swap 交换结果 Vec，避免每轮 clone
- **RwLock**：读多写少场景使用 RwLock 替代 Mutex，UI 读不阻塞

### ANSI / Unicode 处理

- 输入带有 ANSI 转义码（如 `ls --color`）时，先 strip 再匹配和计算 highlight 位置
- 显示时使用 stripped 文本渲染，避免残留的 escape 序列破坏布局
- 查询输入框使用 `unicode-width` 计算视觉宽度，中文等宽字符光标位置正确

### 非阻塞预览

- 每次光标移动触发预览请求，由独立线程执行 shell 命令
- 使用 generation counter 保证只显示最新请求的结果
- 预览执行期间显示 "Loading..."，主事件循环不被阻塞

## 性能基准

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

- 每行约 64-80 字节（Arc\<str\> 头 + 字符串数据）
- 100 万行 ≈ 80-100 MB RSS
- 匹配结果只持有 `(usize, u32, Vec<u32>)` 约 40 字节/匹配项

## 依赖说明

| 依赖 | 版本 | 用途 |
|------|------|------|
| ratatui | 0.28 | TUI 框架（Fullscreen + Inline viewport）|
| crossterm | 0.28 | 终端后端（事件、raw mode、鼠标）|
| nucleo | 0.5 | 模糊匹配引擎（Helix 编辑器同款）|
| nucleo-matcher | 0.3 | nucleo 底层匹配器 API |
| unicode-width | 0.2 | Unicode 宽字符宽度计算（光标定位）|
| strip-ansi-escapes | 0.2 | ANSI 转义序列安全剥离 |
| clap | 4.x | 命令行参数解析 |
| parking_lot | 0.12 | 高性能 RwLock/Mutex |
| serde + toml | 1.x / 0.8 | 配置文件解析 |
| dirs | 6.x | 跨平台配置目录定位 |
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

# 中文输入测试
printf "你好世界\n测试数据\n模糊搜索\n" | cargo run --release -- --query "模糊"

# ANSI 颜色输入测试
ls --color=always | cargo run --release

# 半屏模式测试
find . -type f | cargo run --release -- --height 10
```

## 退出码

| 码 | 含义 |
|----|------|
| 0 | 成功选中，结果输出到 stdout |
| 2 | 错误（文件不存在、无终端、参数错误等）|
| 130 | 用户取消（Ctrl+C 或 ESC） |

## 错误处理

所有错误路径都给出明确提示并返回退出码 2：

```
fuzzyseek: cannot open 'xxx': No such file or directory
fuzzyseek: stderr is not a terminal, cannot display TUI
fuzzyseek: no input (provide --file or pipe data via stdin)
fuzzyseek: invalid --bind format 'xxx', expected action:key
fuzzyseek: unknown action 'xxx'
fuzzyseek: unknown key 'xxx' in 'yyy'
fuzzyseek: cannot read config file: ...
fuzzyseek: invalid config file: ...
```

## 跨平台兼容性

| 平台 | 状态 | 备注 |
|------|------|------|
| Linux (x86_64/aarch64) | ✓ 完全支持 | 主要开发平台 |
| macOS (Intel/Apple Silicon) | ✓ 完全支持 | crossterm 原生支持 |
| Windows 10/11 | ✓ 支持 | Windows Terminal / ConPTY |
| Windows (旧版 cmd.exe) | △ 部分 | ANSI 支持有限，鼠标可能受限 |
| WSL/WSL2 | ✓ 完全支持 | 等同 Linux |
| SSH 远程终端 | ✓ 支持 | 依赖终端模拟器能力 |
| FreeBSD/OpenBSD | ✓ 支持 | crossterm 支持 |

crossterm 后端自动处理各平台差异（Windows API vs POSIX termios）。鼠标支持在所有现代终端模拟器中可用。Inline viewport 模式（`--height`）在所有支持 scroll region 的终端中可用。

## License

MIT
