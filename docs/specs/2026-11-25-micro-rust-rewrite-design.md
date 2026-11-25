# micro-rs 设计文档 — 用 Rust 重构 micro 终端编辑器

- 日期：2026-11-25
- 状态：等待批准（用户已选 B 档，2026-11-25）
- 上游参考：https://github.com/micro-editor/micro （MIT，Go + tcell）

## 1. 意图与为什么

用户要「Rust 重构 micro 并发布到 GitHub」。目标是一个**能当日常编辑器用**的终端文本编辑器，保持 micro 的操作手感（nano 风格底栏菜单、常规 Ctrl 键位、鼠标选区），核心用 Rust 重写，而非仅做一个玩具 demo。

「重构」在这里取「功能等价重写」之意：不复用 Go 源码，参照其公开行为与键位，用 Rust 生态的成熟 crate 实现。

## 2. 目标 / 非目标 / 反目标

**目标（B 档，用户已选）**
1. 打开/新建/保存文件；命令行传路径；stdin 输入。
2. 基础编辑：插入、删除、退格、光标移动（字符/行/词/文件首尾）、行号、软换行、Unicode。
3. 撤销/重做（内存栈，会话内）。
4. 多光标（Ctrl-Alt 上下 / 手动添加，列选）。
5. 分屏（水平/垂直）与标签页（多个 buffer）。
6. 语法高亮（覆盖主流语言，内置主题）；配色主题（16/256/真彩）。
7. nano 风格底部菜单栏（两级命令菜单）+ 状态栏 + diff/修改 gutter。
8. 鼠标：拖拽选区、双击选词、三击选行、滚轮滚动。
9. 搜索/替换（Ctrl-F / Ctrl-R），支持区分大小写与正则。
10. 系统剪贴板（copy/paste/cut），不可用时退化为内部剪贴板。
11. 配置文件（键位重绑定、主题、Tab 宽度、行号开关等）。
12. Lua 插件系统（基础 API：注册命令、绑定按键、读写 buffer）。
13. 跨平台：Windows / Linux / macOS，预编译产物发 GitHub Release。

**非目标（本次不做）**
- 130+ 语言语法文件逐一移植（用 syntect 现成语法集替代，行为相近）。
- 插件管理器（在线安装/更新 Lua 插件）。
- 宏录制、自动 lint、持久化撤销。
- 与 micro 的配置目录/配置文件格式二进制兼容。

**反目标（做了就算失败）**
- 单文件编译超过 2 分钟、启动超过 100ms、编辑大文件明显卡顿。
- 依赖 MSVC/GCC 专有工具链导致某平台构建不出来（见 §4 约束）。

## 3.  Approach（三档，用户已选 B）

| 档 | 内容 | 取舍 |
|---|---|---|
| A 核心编辑器 | 编辑/保存/撤销、行号、高亮、底栏菜单、鼠标选区、搜索、配置 | 快，但没多光标/分屏/插件，不算 micro 的「手感」 |
| **B 高度复刻（选中）** | A 全部 + 多光标、分屏/标签、diff gutter、配色主题、Lua 插件、键位重绑 | 完整覆盖日常使用，工作量适中，是本项目的定位 |
| C 全量 1:1 | 再加语法文件全移植、插件管理器、宏、自动 lint | 等于把 micro 重写一遍，收益递减 |

## 4. 约束与已探明的坑

- **Windows MinGW 工具链**：本机 cargo 1.100 / rustc 1.100（D:\Scoop\apps\rustup-gnu\current），GNU 链接需 DLL 工具在 PATH（`D:\Scoop\apps\mingw\current\bin`）。
- **避免 C 依赖**：历史踩坑——mimalloc 与 blake3 的 C SIMD 在 MinGW 下触发 `0xc0000409` 崩溃。syntect 默认带 oniguruma（C）→ 设计上用 `syntect = { version = "5", default-features = false, features = ["parsing", "assets", "dump-load", "fancy-regex"] }` 走纯 Rust 正则引擎，不引入 C 编译。
- **D:\Temp 沙箱不可写**：单测临时目录一律用 `std::env::current_dir()/target/...`，不用系统 temp。
- **GitHub 访问**：所有 GitHub URL 前加代理 `https://v6.gh-proxy.org/`（AGENTS.md 硬规则）；`git push` 直连复用已存凭据（代理 push 会认证失败）；`gh` CLI 已登录 zeno-ling，token 有 repo/workflow scope。
- **构建命令固定**：`$env:PATH += ";D:\Scoop\apps\mingw\current\bin"; cargo build --release`（本项目无 mimalloc feature，不需 `--no-default-features`；如需加则一并写进 README）。

## 5. 技术栈（我的决定，用户未指定）

| 用途 | crate | 理由 |
|---|---|---|
| 终端 UI | `ratatui` 0.29 + `crossterm` 0.28 | Rust 终端 UI 事实标准，鼠标/真彩支持完善 |
| 文本模型 | `ropey` 1.6 | 大文件 O(log n) 插入/删除，micro 同类编辑器通用选择 |
| 语法高亮 | `syntect` 5（fancy-regex，无 C） | 现成 Sublime 语法/主题资源，几百种语言 |
| Lua 插件 | `mlua` 0.10（lua54） | 成熟绑定，沙箱可控 |
| 系统剪贴板 | `copypasta` 0.10 | 跨平台，失败回退内部剪贴板 |
| 配置 | `serde` + `serde_json` | 简单可靠，键位表/主题用 JSON |
| CLI 参数 | `clap` 4 | 标准做法 |
| 配置目录 | `dirs` 6 | `~/.config/micro-rs` |
| 错误 | `anyhow` / `thiserror` | 应用与库分层 |

## 6. 架构与模块

```
micro-rs/
├── Cargo.toml
├── src/
│   ├── main.rs          CLI 入口（clap），构造 Editor 并进入事件循环
│   ├── editor.rs        Editor 全局状态、事件循环、Tab/Split 布局树
│   ├── buffer.rs        Buffer：ropey + 光标 + 选区 + 多光标 + 撤销栈
│   ├── undo.rs          操作级撤销/重做（插入/删除/替换的逆操作栈）
│   ├── view.rs          视口：滚动、行号、软换行、坐标换算
│   ├── input.rs         按键解析 + keymap 重绑定 + 命令分发
│   ├── commands.rs      命令实现（移动/编辑/搜索/替换/文件/分屏…）
│   ├── search.rs        搜索/替换（大小写、正则）
│   ├── syntax.rs        syntect 封装，高亮缓存（按可见行）
│   ├── theme.rs         配色主题（内置 dark/light/solarized + 16/256/truecolor）
│   ├── plugins.rs       mlua 运行时，插件 API 注册
│   ├── config.rs        配置读写（~/.config/micro-rs/config.json）
│   ├── clipboard.rs     系统剪贴板 + 内部回退
│   └── ui/
│       ├── mod.rs       绘制总入口
│       ├── menubar.rs   nano 风格底栏（两级菜单）
│       ├── statusbar.rs 状态栏（文件/光标/编码/修改标记）
│       ├── gutter.rs    行号 + 修改标记
│       └── tabbar.rs    标签页标题栏
├── tests/               集成测试（打开→编辑→保存 roundtrip）
├── .github/workflows/ci.yml
├── README.md / LICENSE(MIT)
└── scripts/release.ps1 / release.sh   （复用 MosaicDisk 的多平台发布脚本）
```

**关键数据流**：crossterm 事件 → input.rs（按 keymap 映射为 Command）→ commands.rs 操作 Buffer/View/Editor → ratatui 按帧绘制（可见行区间经 syntax.rs 高亮，theme.rs 上色）。Buffer 不依赖 UI，便于单测。

## 7. 实施阶段（顺序）

1. **骨架**：Cargo.toml、clap CLI、ratatui 空白事件循环、q 退出、Ctrl-C 优雅退出。
2. **Buffer 核心**：ropey 封装、光标移动、插入/删除、选区、撤销栈；单测覆盖。
3. **渲染**：视口/滚动、行号、状态栏、nano 底栏菜单；打开文件显示。
4. **文件与搜索**：打开/保存/新建、搜索/替换、命令行路径与 stdin。
5. **进阶编辑**：多光标、软换行、鼠标拖拽/双击/三击、系统剪贴板。
6. **外观**：syntect 高亮（可见行缓存）、主题、真彩降级、修改 gutter。
7. **布局**：标签页、水平/垂直分屏。
8. **可扩展**：配置文件（键位重绑）、mlua 插件 API。
9. **验证与发布**：CI（三平台 build + Linux test）、README、Release 脚本、GitHub Release。

## 8. 验证计划

- 单元测试：buffer 插入/删除/撤销重做、光标边界、选区、搜索替换、配置解析。
- 集成测试：临时目录内 打开→编辑→保存→重读 一致性（用 current_dir()/target）。
- 手动验收：真实终端跑一遍 §2 的核心操作清单，确认底栏菜单/鼠标/分屏/主题/插件。
- CI：push main + PR，ubuntu/macos/windows 三平台 `cargo build --release`，ubuntu 跑 `cargo test --release`。

## 9. 发布计划

- 新仓库 `zeno-ling/micro-rs`（`gh repo create ... --public`），MIT LICENSE。
- 复用 MosaicDisk 的 release 脚本：Windows x64 zip + Inno Setup 安装包、Linux x64 tar.gz、macOS arm64/x64 tar.gz（GitHub Actions 产物 + 本地 gh release upload --clobber）。
- README：截图、键位表、配置与插件说明、四平台下载直链、CI badge。

## 10. 留给实现者的决定（我的 call，非用户指定）

- 二进制名 `micro-rs`，配置目录 `~/.config/micro-rs`，避免覆盖用户已有的上游 `~/.config/micro`。
- 撤销为内存栈（会话内），不做 micro 的磁盘持久化撤销。
- diff gutter 简化为「本会话已修改行」标记，而非与磁盘文件做完整 diff 算法。
- 插件 API 第一版只暴露：注册命令、绑定按键、取/设当前 buffer 文本与光标；插件管理器留到后续版本。
- 发布版本号从 `0.1.0` 起。
