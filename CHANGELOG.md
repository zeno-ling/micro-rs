# Changelog

本文件记录 micro-rs 的所有重要变更。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/spec/v2.0.0.html)。

## [未发布]

## [0.1.4] - 2026-11-25

### 新增

- **代码折叠**：基于 tree-sitter AST 的多行节点行列范围生成折叠区（`src/syntax.rs` 的 `collect_folds()`），排除 `source_file` 本身。
  - F3：折叠 / 展开光标所在代码块；光标不在折叠起点时自动找最近的外层折叠区。
  - F4：展开全部折叠。
  - 折叠区除首行外在界面中隐藏，起始行显示 `▾`（已折叠）/ `▸`（可折叠）标记（`src/ui.rs`）。
  - `Editor.folded` 保存当前折叠的起始行集合。
- **tree-sitter 语法高亮**：参照 Helix 的语义分桶思路，解析整份文档成 AST 后按节点 kind 与文本分类上色，覆盖 13 种语言：Rust、Python、JavaScript、TypeScript、TSX、JSON、C、C++、Go、Shell、HTML、CSS、Markdown。
- 新增 4 个语法高亮单元测试（关键字/注释上色、Rust 函数折叠、全 grammar 可解析、语言检测）。

### 变更

- **高亮引擎从 syntect 切换到 tree-sitter**（`src/syntax.rs` 整体重写）：
  - 放弃 highlight query 编译——不同 grammar 版本的节点类型不一致（如 Rust 的 `true`、`crate` 报 `Invalid node type`），改为直接遍历 AST，零 query 依赖、零节点类型漂移风险。
  - 函数 / 类名先用 `child_by_field_name("name")` 上 Function / Type 色，再走通用递归，后写覆盖先写。
  - 按全文 hash + 主题惰性缓存高亮行与折叠区，编辑后自动刷新。
  - 4 套主题（base16-ocean.dark、base16-eighties.dark、InspiredGitHub、Solarized (dark)）名称兼容旧配置，无需改 `config.json`。
- Cargo 依赖：`syntect` → `tree-sitter 0.25` + 13 个 grammar crate。
- Windows 编译 C grammar 前需把 `TEMP` / `TMP` 指向可写目录（见下方修复说明）。

### 修复

- 修复 `paint_range()` 把 AST 绝对字节偏移当成行内偏移写入颜色数组的错误——此前第二行起的所有高亮颜色整体错位。
- 定位并绕过 Windows 下 C 依赖编译崩溃（`0xc0000409`）：根因是 `D:\Temp` 不可写（`cc-rs: Cannot create temporary file in D:\Temp/: Permission denied`），把 `TEMP` / `TMP` 指向项目内可写目录后 gcc 正常编译。这也解释了此前 mlua vendored lua54 与 blake3 C SIMD 的同类崩溃。

## [0.1.3] - 2026-11-25

### 修复

- 修复 Shift 组合符号与大写字母无法输入：终端把 `Shift+1` 上报为 `Char('!')` 加 SHIFT 修饰位，旧逻辑要求 `modifiers.is_empty()` 才插入字符，导致所有带 SHIFT 的字符被丢弃。新判据只拒绝 CONTROL / ALT（真组合键），SHIFT 字符正常插入。
- 命令提示符输入框同样修正，此前只能输入小写与数字，无法输入大写盘符和路径符号。

### 新增

- 3 个单测覆盖 Shift 符号与大写、组合键不误插、字符确实写入缓冲区。

## [0.1.2] - 2026-11-25

### 新增

- **按键序列树（KeyTree）**：对齐官方 micro 的 `(*KeyTree).NextEvent`，支持多键序列（`Ctrl-x Ctrl-s`）、通配键 `any`（对应 `RegisterKeyAnyBinding`）、模式约束（buffer / prompt / menu），精确键优先于通配、长序列优先于短序列，挂起时状态栏显示 `Ctrl-x …`。
- 内置多键序列：`Ctrl-x Ctrl-s` 保存、`Ctrl-x Ctrl-c` 退出、`Ctrl-x Ctrl-f` 打开、`Ctrl-x 2` / `Ctrl-x 3` 分屏、`Ctrl-x o` 切窗格、`Alt-w any` 取消。
- **结构化插件钩子**：钩子收到包含 `event` / `arg` / `path` / `line` / `col` / `dirty` / `tab` 的 Rhai map；`onBeforeTextEvent` 返回 `true` 或 `#{ block: true }` 可拦截文本修改。
- `onMouse` 钩子：滚轮与左右中键事件先派发给插件再执行绑定命令。
- 插件管理命令：`ListPlugins`（列出插件与开关）、`ReloadPlugins`（不重启重新扫描）、`ListKeybindings`（列出键位）。
- `config.json` 新增 `mouse`（鼠标动作绑定）与 `plugins`（按脚本名开关）字段。
- 配置文件可覆盖默认键位，多键序列与通配键均支持。

### 变更

- 插件运行时字段名与函数名统一为官方风格的 camelCase（如 `onBeforeTextEvent`、`onAnyEvent`）。

## [0.1.1] - 2026-11-25

### 修复

- 插件系统此前是死代码：`plugins.hook()` 从未被调用。现已在按键、保存、打开文件、切换标签、切换主题、文本修改前等关键事件点派发钩子。
- `config.json` 的 `keymap` 字段此前从未被读取，用户重绑无效。现解析键位描述并覆盖内置默认绑定。
- 插件加载改为遍历配置目录下的 `plugins/*.rhai`，按文件名排序加载。

## [0.1.0] - 2026-11-25

### 新增

- 首个公开版本，用 Rust 从零重写 micro 终端编辑器的核心体验。
- 打开 / 新建 / 保存文件，命令行传参与 stdin 输入。
- 基础编辑、撤销重做、多光标（Alt-P / Alt-N）、水平 / 垂直分屏、多标签页。
- syntect 语法高亮（内置语法与主题）、行号、软换行、Unicode。
- 查找 / 替换（区分大小写、正则）、系统剪贴板（不可用回退内部剪贴板）。
- nano 风格底部菜单栏与状态栏。
- JSON 配置、Rhai 插件运行时。
- 四平台发布：Windows x64、Linux x64、macOS arm64 / x64。
