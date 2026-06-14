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
- **多选模式**：Tab/Ctrl+Space 切换选中，Ctrl+A 全选，Ctrl+D 全不选
- **预览面板**：`--preview` 非阻塞异步执行，带超时、取消、缓存、错误展示
- **类型预览**：配置文件可为不同文件类型指定不同预览命令
- **主题系统**：暗/亮主题，支持环境变量、配置文件、CLI 优先级合并
- **可扩展输入**：支持管道、文件、命令执行（`--cmd`），可配 max-items 防 OOM
- **格式自动解析**：`ps aux`、`ls -l`、`history` 等输出结构化解析
- **冲突检测**：`--bind` 重映射时自动检测并警告键冲突
- **自定义快捷键**：`--bind` 参数 + 配置文件，可重映射所有动作
- **Shell 集成**：Bash/Zsh/Fish 一键开启 Ctrl+R/Ctrl+T/** 补全
- **Vim 插件**：`:FuzzySeek` 命令直接在 Vim/Neovim 中使用
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

# 从命令输出读取
fuzzyseek --cmd "find . -type f"

# 半屏模式（只占 15 行，不进入 alternate screen）
find . -type f | fuzzyseek --height 15

# 多选模式
cat urls.txt | fuzzyseek --multi

# 带预览（非阻塞，异步执行，5秒超时）
find . -name "*.rs" | fuzzyseek --preview "head -20 {}"

# 自定义预览超时
fuzzyseek --file list.txt --preview "cat {}" --preview-timeout 3000

# 初始查询
ps aux | fuzzyseek --query "python"

# 限制最大行数（OOM 保护）
fuzzyseek --cmd "find / -type f" --max-items 100000

# 自定义分隔符（NUL 分隔输出）
find . -type f | fuzzyseek --multi --delimiter $'\0'

# 自定义快捷键
fuzzyseek --file list.txt --bind "confirm:ctrl-y,cancel:ctrl-q"

# 使用亮色主题
fuzzyseek --file list.txt --theme light
```

## 主题

FuzzySeek 支持暗色（默认）和亮色主题，优先级：CLI > 配置文件 > 环境变量 > 默认。

```bash
# CLI 指定
fuzzyseek --theme light

# 环境变量
export FUZZYSEEK_THEME=light

# 配置文件（~/.config/fuzzyseek/config.toml）
[theme]
base = "light"
cursor_fg = "#0066cc"
highlight_fg = "red"
selected_fg = "magenta"
```

支持的颜色值：命名颜色（red, green, blue, cyan, magenta, yellow, white, black, darkgray 等）、十六进制（`#rrggbb`）、256色索引（0-255）。

## Shell 集成

### Bash

```bash
# 在 ~/.bashrc 中添加
eval "$(fuzzyseek --shell-integration bash)"
```

提供：
- **Ctrl+R**：模糊搜索命令历史
- **Ctrl+T**：搜索文件并插入路径
- **Alt+C**：搜索目录并 cd
- **\*\* 补全**：输入路径后跟 `**` 触发模糊补全

### Zsh

```zsh
# 在 ~/.zshrc 中添加
eval "$(fuzzyseek --shell-integration zsh)"
```

### Fish

```fish
# 在 ~/.config/fish/config.fish 中添加
fuzzyseek --shell-integration fish | source
```

### 自定义搜索命令

```bash
# 使用 fd 替代 find
export FUZZYSEEK_CTRL_T_COMMAND="fd --type f --hidden --follow"
export FUZZYSEEK_ALT_C_COMMAND="fd --type d --hidden --follow"
```

## Vim/Neovim 集成

将 `shell/fuzzyseek.vim` 放入 Vim 的插件目录，或使用插件管理器。

```vim
" 使用 vim-plug
Plug 'path/to/fuzzyseek', { 'rtp': 'shell' }
```

提供命令：
- `:FuzzySeek` / `:FuzzySeekFiles` - 查找文件并编辑
- `:FuzzySeekSplit` / `:FuzzySeekVsplit` / `:FuzzySeekTab` - 分屏打开
- `:FuzzySeekBuffers` - 切换 Buffer
- `:FuzzySeekHistory` - 命令历史
- `:FuzzySeekGrep <pattern>` - Grep 搜索

默认映射：`<leader>ff`(文件) `<leader>fb`(Buffer) `<leader>fh`(历史)

## 快捷键

默认绑定（均可通过 `--bind` 或配置文件自定义）：

| 按键 | 动作名 | 功能 |
|------|--------|------|
| Enter | confirm | 确认选择 |
| Esc / Ctrl+C | cancel | 取消 |
| ↑ / Ctrl+P | up | 上移光标 |
| ↓ / Ctrl+N | down | 下移光标 |
| Home | home | 跳到第一项 |
| End | end | 跳到最后一项 |
| PageUp | page-up | 上翻页 |
| PageDown | page-down | 下翻页 |
| Ctrl+E | scroll-down | 滚动视口下 |
| Ctrl+Y | scroll-up | 滚动视口上 |
| Tab / Ctrl+Space | toggle | 切换选中（多选模式）|
| Ctrl+A | select-all | 全选（多选模式）|
| Ctrl+D | deselect-all | 取消全选 |
| Ctrl+U | clear-query | 清空查询 |
| Ctrl+W | delete-word | 删除最后一个词 |
| Backspace | backspace | 删除最后一个字符 |
| Ctrl+\\ | toggle-preview | 切换预览面板显示 |
| Ctrl+R | refresh-preview | 刷新预览 |
| 鼠标滚轮 | — | 上下滚动 |

## 自定义快捷键

### 通过 --bind 参数

格式：`action:key`，多个用逗号分隔：

```bash
fuzzyseek --bind "confirm:ctrl-y,cancel:ctrl-q,up:ctrl-k,down:ctrl-j"
```

冲突检测：如果绑定的键已被其他动作使用，会输出警告并自动重映射。

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
toggle-preview = "ctrl-p"
```

### 可绑定的动作

`confirm`、`cancel`、`up`、`down`、`home`、`end`、`page-up`、`page-down`、
`toggle`、`select-all`、`deselect-all`、`backspace`、`clear-query`、`delete-word`、
`scroll-up`、`scroll-down`、`toggle-preview`、`refresh-preview`

### 可用的键名

修饰符：`ctrl-`、`alt-`、`shift-`  
键名：`enter`/`return`/`cr`、`esc`、`tab`、`btab`(Shift+Tab)、`bs`/`backspace`、
`del`/`delete`、`up`、`down`、`left`、`right`、`home`、`end`、`pgup`、`pgdn`、`space`、
单字符如 `a`-`z`、`0`-`9`

## 配置文件

完整配置示例 `~/.config/fuzzyseek/config.toml`：

```toml
[keys]
confirm = "ctrl-m"
cancel = "ctrl-g"
up = "ctrl-k"
down = "ctrl-j"

[theme]
base = "dark"
cursor_fg = "cyan"
highlight_fg = "yellow"
selected_fg = "green"

[preview]
timeout_ms = 5000
cache_size = 64
max_output_bytes = 1048576

[[preview.rules]]
pattern = "ext:rs"
cmd = "bat --color=always {}"

[[preview.rules]]
pattern = "ext:md"
cmd = "glow {}"

[[preview.rules]]
pattern = "ext:png"
cmd = "chafa {}"

[input]
max_items = 500000
max_line_length = 4096
```

## 命令行参数

```
Usage: fuzzyseek [OPTIONS]

Options:
  -f, --file <FILE>               从文件读取输入
      --cmd <CMD>                 从命令输出读取输入
  -m, --multi                     启用多选模式
  -p, --preview <COMMAND>         预览命令（{} 为占位符）
      --preview-timeout <MS>      预览超时毫秒 [默认: 5000]
  -q, --query <QUERY>             初始查询字符串
      --height <HEIGHT>           高度行数（0=全屏）[默认: 0]
  -d, --delimiter <DELIM>         输出分隔符 [默认: \n]
      --bind <BIND>               自定义快捷键
      --theme <THEME>             主题 (dark/light)
      --max-items <N>             最大读取行数（OOM 保护）
      --shell-integration <SHELL> 打印 Shell 集成脚本 (bash/zsh/fish)
  -h, --help                      帮助
  -V, --version                   版本
```

## 架构

```
┌──────────────────────────────────────────────────────────┐
│  Input Provider (trait)                                    │
│  ─────────────────────                                    │
│  StdinProvider / FileProvider / CommandProvider            │
│  → BufReader(256KB) → chunked Arc<str> → SharedStore      │
│  → ProviderConfig: max_items, max_line_length             │
├──────────────────────────────────────────────────────────┤
│  Matcher Thread                                           │
│  ──────────────                                           │
│  nucleo Atom engine, 8192-item scan chunks                │
│  AtomicBool cancel, swap-buffer update                    │
│  stable sort (score desc, index asc)                      │
├──────────────────────────────────────────────────────────┤
│  Preview System                                           │
│  ──────────────                                           │
│  PreviewResolver (type-based command selection)           │
│  PreviewRunner (timeout + cancel + generation)            │
│  PreviewCache (LRU, configurable capacity)                │
│  Error display in panel                                   │
├──────────────────────────────────────────────────────────┤
│  Theme System                                             │
│  ────────────                                             │
│  Priority: CLI > config.toml > $FUZZYSEEK_THEME > dark   │
│  Dark/Light builtin, custom color overrides               │
├──────────────────────────────────────────────────────────┤
│  KeyBind System                                           │
│  ─────────────                                            │
│  HashMap<Action, Vec<KeyBind>>                            │
│  Conflict detection + warning on --bind reassignment      │
│  18 actions, configurable via TOML or CLI                 │
├──────────────────────────────────────────────────────────┤
│  Main Thread (TUI)                                        │
│  ─────────────────                                        │
│  crossterm events (50ms poll), ratatui rendering          │
│  Fullscreen or Inline viewport, virtual scrolling         │
│  unicode-width cursor, ANSI-stripped highlighting         │
└──────────────────────────────────────────────────────────┘
```

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

## 测试

```bash
# 运行所有测试（66个：46单元 + 20集成）
cargo test

# 运行基准
cargo bench

# 快速冒烟测试
seq 1 1000000 | cargo run --release -- --query "999"

# 命令输入测试
cargo run --release -- --cmd "seq 1 10000" --query "555"

# 主题测试
find . -type f | cargo run --release -- --theme light
```

## 退出码

| 码 | 含义 |
|----|------|
| 0 | 成功选中，结果输出到 stdout |
| 2 | 错误（文件不存在、无终端、参数错误等）|
| 130 | 用户取消（Ctrl+C 或 ESC） |

## 跨平台兼容性

| 平台 | 状态 | 备注 |
|------|------|------|
| Linux (x86_64/aarch64) | ✓ 完全支持 | 主要开发平台 |
| macOS (Intel/Apple Silicon) | ✓ 完全支持 | crossterm 原生支持 |
| Windows 10/11 | ✓ 支持 | Windows Terminal / ConPTY |
| WSL/WSL2 | ✓ 完全支持 | 等同 Linux |

## License

MIT
