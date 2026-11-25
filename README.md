# micro-rs

![CI](https://github.com/zeno-ling/micro-rs/actions/workflows/ci.yml/badge.svg)

一个用 Rust 写的终端文本编辑器，操作手感参照 [micro](https://github.com/micro-editor/micro)（Go + tcell，MIT）。核心用 Rust 生态的成熟 crate 从头重写，不复用 Go 源码。

## 特性

- 打开 / 新建 / 保存文件，支持命令行传路径和 stdin 输入
- 基础编辑：插入、删除、退格、光标移动、行号、软换行、Unicode
- 撤销 / 重做（内存操作栈）
- 多光标（Alt-P / Alt-N 上下添加）
- 水平 / 垂直分屏与多标签页
- 语法高亮（syntect 内置语法 / 主题，可切换主题）
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

底栏菜单可用方向键左右切换、回车执行命令、Esc 关闭。

## 配置

配置文件在 `~/.config/micro-rs/config.json`（Windows 为 `%APPDATA%\micro-rs\config.json`）：

```json
{
  "tab_width": 4,
  "tab_size": 4,
  "line_numbers": true,
  "theme": "base16-ocean.dark"
}
```

## 插件

`Plugins` 运行时基于 Rhai（相比设计文档最初写的 Lua/mlua，改为纯 Rust 的 Rhai 以避免 C 工具链依赖）。插件脚本可定义 `on_save()` 等无参钩子函数，编辑器在对应时机调用。

## 发布脚本

- Windows：`scripts/release.ps1 -Version v0.1.0`，参数 `-SkipBuild` 跳过编译、`-NoUpload` 只打包不上传。
- Linux / macOS：`bash scripts/release.sh -v v0.1.0 -s`（`-s` 只打包不上传）；交叉编译示例：`bash scripts/release.sh -v v0.1.0 -s -t x86_64-apple-darwin -a x64`。

## 许可

MIT，见 [LICENSE](LICENSE)。

参考实现 [micro](https://github.com/micro-editor/micro) 同为 MIT 许可。
