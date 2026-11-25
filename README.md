# micro-rs

![CI](https://github.com/zeno-ling/micro-rs/actions/workflows/ci.yml/badge.svg)

一个用 Rust 写的终端文本编辑器，操作手感参照 [micro](https://github.com/micro-editor/micro)（Go + tcell，MIT）。核心用 Rust 生态的成熟 crate 从头重写，不复用 Go 源码。

## 特性

- 打开 / 新建 / 保存文件，支持命令行传路径和 stdin 输入
- 基础编辑：插入、删除、退格、光标移动、行号、软换行、Unicode
- 撤销 / 重做（内存操作栈）
- 多光标（Alt-P / Alt-N 上下添加）
- 水平 / 垂直分屏与多标签页
- 语法高亮（tree-sitter 解析，参照 Helix 的语义分桶上色，13 种语言，可切换主题）
- 代码折叠（基于 tree-sitter AST 的多行节点范围，F3 折叠 / F4 全部展开）
- nano 风格底部菜单栏 + 状态栏
- 查找 / 替换，支持区分大小写与正则
- 系统剪贴板（不可用自动回退内部剪贴板）
- JSON 配置文件（主题、Tab 宽度、行号开关）
- Rhai 插件运行时（脚本钩子）
- 跨平台：Windows / Linux / macOS

## 下载预编译版

到 [Releases](https://github.com/zeno-ling/micro-rs/releases) 下载对应平台：

| 平台 | 文件 |
|---|---|
| Windows x64 | `micro-rs-<版本>-windows-x64.zip` |
| Linux x64 | `micro-rs-<版本>-linux-x64.tar.gz` |
| macOS arm64 (Apple Silicon) | `micro-rs-<版本>-macos-arm64.tar.gz` |
| macOS x64 (Intel) | `micro-rs-<版本>-macos-x64.tar.gz` |

解压后直接运行 `micro-rs`（Windows 为 `micro-rs.exe`）。

## 从源码安装

需要 Rust stable（1.80+）。

```bash
git clone https://github.com/zeno-ling/micro-rs.git
cd micro-rs
cargo build --release
```

产物在 `target/release/micro-rs`（Windows 为 `micro-rs.exe`）。

Windows + GNU 工具链需要 MinGW 的 DLL 工具在 `PATH`：

```powershell
$env:PATH += ";D:\Scoop\apps\mingw\current\bin"
cargo build --release
```

## 用法

```bash
micro-rs                # 新建空文件
micro-rs file.txt       # 打开文件
micro-rs a.txt b.txt    # 打开多个标签
echo hello | micro-rs   # 从 stdin 读入
```

### 快捷键

| 按键 | 功能 |
|---|---|
| Ctrl-Q | 退出 |
| Ctrl-S | 保存 |
| Ctrl-O | 打开 |
| Ctrl-Z / Ctrl-Y | 撤销 / 重做 |
| Ctrl-X / Ctrl-C / Ctrl-V | 剪切 / 复制 / 粘贴 |
| Ctrl-F / Ctrl-R | 查找 / 替换 |
| Ctrl-N | 查找下一个 |
| Ctrl-A | 全选 |
| Alt-P / Alt-N | 上 / 下加光标 |
| Ctrl+← / Ctrl+→ | 行首 / 行尾 |
| Ctrl+↑ / Ctrl+↓ | 文件首 / 文件尾 |
| 方向键 / Home / End | 移动光标 |
| F2 | 切换主题 |
| F3 | 折叠 / 展开光标处代码块 |
| F4 | 展开所有折叠 |

底栏菜单可用方向键左右切换、回车执行命令、Esc 关闭。

## 配置

配置文件在 `~/.config/micro-rs/config.json`（Windows 为 `%APPDATA%\micro-rs\config.json`）：

```json
{
  "tab_width": 4,
  "tab_size": 4,
  "line_numbers": true,
  "theme": "base16-ocean.dark",
  "keymap": {
    "Ctrl-s": "Save",
    "Ctrl-x Ctrl-s": "Save",
    "Alt-p": "AddCursorAbove",
    "F2": "ToggleTheme"
  },
  "mouse": {
    "MouseWheelUp": "PageUp",
    "MouseWheelDown": "PageDown"
  },
  "plugins": {
    "guard": false
  }
}
```

`keymap` 的键是键位描述，支持**多键序列**（空格分隔，如 `"Ctrl-x Ctrl-s"`）与通配键 `any`；值是命令名（大小写与 `_` / `-` 分隔均可）。用户绑定覆盖内置默认键位。

`mouse` 支持 `MouseWheelUp` / `MouseWheelDown` / `MouseLeft` / `MouseRight` / `MouseMiddle` 五个动作。

`plugins` 按脚本文件名（不含扩展名）给插件开关，缺省为启用。

### 键位系统

键位不再是扁平表，而是与官方 micro `KeyTree` 对齐的**按键序列树**：

- **多键序列**：`Ctrl-x Ctrl-s` 这类前缀组合，挂起时状态栏显示 `Ctrl-x …`。
- **通配绑定**：`any` 匹配任意无修饰键，可用来兜底。
- **模式约束**：每个绑定可限定只在 `buffer` / `prompt` / `menu` 模式下生效；焦点变化时自动切换模式。
- 精确键优先于通配键，更长序列优先于更短序列，与 micro `(*KeyTree).NextEvent` 语义一致。

内置多键序列：`Ctrl-x Ctrl-s` 保存、`Ctrl-x Ctrl-c` 退出、`Ctrl-x Ctrl-f` 打开、`Ctrl-x 2` 水平分屏、`Ctrl-x 3` 垂直分屏、`Ctrl-x o` 切窗格、`Alt-w any` 取消挂起。

## 插件

`Plugins` 运行时基于 Rhai（相比设计文档最初写的 Lua/mlua，改为纯 Rust 的 Rhai 以避免 C 工具链依赖）。把 `.rhai` 脚本放到配置目录下的 `plugins/` 子目录（Windows 为 `%APPDATA%\micro-rs\plugins\`），启动时自动加载。

插件可定义以下钩子。**每个钩子收到一个结构化事件 map**（官方 micro 传 `Event` 对象，这里用 map 等价表达）：

```rhai
// ev 的通用字段
// ev.event  钩子名或事件名，字符串
// ev.arg    本次触发的具体参数（如 Insert/Backspace/路径）
// ev.path   当前文件路径，无文件时为空串
// ev.line   光标行（0 基）
// ev.col    光标列（0 基）
// ev.dirty  缓冲区是否有未保存修改
// ev.tab    当前标签页序号
```

| 钩子 | 触发时机 |
|---|---|
| `onAnyEvent(ev)` | 任意事件（通用钩子，每次都会收到） |
| `onKey(ev)` | 按下任意键，`ev.arg` 是键名 |
| `onSave(ev)` | 保存文件前，`ev.arg` 是路径 |
| `onBufferOpen(ev)` | 打开文件 |
| `onSetActive(ev)` | 切换标签页，`ev.arg` 是序号 |
| `onBufferOptionChanged(ev)` | 主题等选项变化 |
| `onMouse(ev)` | 鼠标动作，`ev.arg` 是 `MouseWheelUp` 等 |
| `onBeforeTextEvent(ev)` | 文本修改前，`ev.arg` 是 `Insert` / `Backspace` / `Delete` / `Newline` / `Tab` / `Paste` |

拦截型钩子（`onBeforeTextEvent`）返回 `true` 或 `#{ block: true }` 即阻止本次操作。

示例（`%APPDATA%\micro-rs\plugins\guard.rhai`）：

```rhai
// 禁止在 .lock 文件里删除
fn onBeforeTextEvent(ev) {
    if ev.path.ends_with(".lock") && ev.event == "Delete" {
        return #{ block: true };
    }
    return false;
}

// 保存时打印路径
fn onSave(ev) {
    print(`saving ${ev.path} at line ${ev.line}`);
}
```

### 管理插件

- `ListPlugins` 命令：状态栏列出已安装插件与开关状态（关闭的显示 `[off]`）。
- `ReloadPlugins` 命令：不重启即重新扫描 `plugins/` 目录。
- 在 `config.json` 的 `plugins` 里改开关，或用上述命令配合配置持久化。

### 与官方 micro 的插件差异

官方 micro 用 **gopher-lua**，插件是 Lua 源文件，通过 `Plugin.Call("onEvent", args)` 走 Lua 栈传参。micro-rs 用 Rhai：语法接近但**不是** Lua，现有 micro Lua 插件不能直接搬过来，需要按上表改写。这样做的原因是在 MinGW 下 vendored lua54 会触发 `0xc0000409` 崩溃，纯 Rust 的 Rhai 能稳定跨平台编译。事件钩子名与语义已按官方对齐，改写只涉及语法层。

## 发布脚本

- Windows：`scripts/release.ps1 -Version v0.1.4`，参数 `-SkipBuild` 跳过编译、`-NoUpload` 只打包不上传。
- Linux / macOS：`bash scripts/release.sh -v v0.1.4 -s`（`-s` 只打包不上传）；交叉编译示例：`bash scripts/release.sh -v v0.1.4 -s -t x86_64-apple-darwin -a x64`。

## 许可

MIT，见 [LICENSE](LICENSE)。

参考实现 [micro](https://github.com/micro-editor/micro) 同为 MIT 许可。
