# micro-rs 后续路线图（提案，暂不实现）

> 状态：**提案 / 仅记录**。本文件只记录候选方向与优先级，不代表已承诺实现。
> 调研日期：2026-11-25。目标版本：v0.2.0 及以后。

## 背景

v0.1.4 已完成 syntect → tree-sitter 的高亮切换与基于 AST 的代码折叠（见 [CHANGELOG.md](../CHANGELOG.md)）。下一步的能力边界集中在"编辑器 → 准 IDE"的跃迁，以及官方 micro 尚未补齐的复刻缺口。当前代码库已经具备的关键底座：

```text
src/
├── syntax.rs      # tree-sitter 解析缓存：AST + 语义分桶高亮 + fold 区间
├── keytree.rs     # 对齐官方 micro 的 KeyTree：多键序列 / 通配 / 模式约束
├── plugins.rs     # Rhai 钩子运行时（结构化 map 事件）
├── editor.rs      # 事件循环、命令派发、折叠状态（Editor.folded）
└── ui.rs          # ratatui 渲染、折叠标记 ▾/▸
```

## 候选方向（按优先级排序）

| 优先级 | 方向 | 一句话 | 依赖底座 | 预估 |
|---|---|---|---|---|
| 10 | LSP 集成 | 按 Helix 架构接语言服务器：自动补全 + 跳转定义 + 诊断 | 需新增异步 client + JSON-RPC | 大（数小时） |
| 8 | tree-sitter 符号导航 | 复用现有 AST 做符号大纲 + Ctrl-T 跳转，纯 Rust 零外部依赖 | syntax.rs 已就绪 | 中（1-2 小时） |
| 6 | 宏录制 / 回放 | 补齐官方 micro 的 macro 手感，重复编辑一键回放 | 事件序列录制 | 中 |
| 5 | MosaicDisk 扩展 | Top 1000 大文件报表 + 重复文件汇总（另一项目，v0.2.0） | 磁盘扫描基础已就绪 | 中 |

## 各方向展开

### P10 — LSP 集成

参考 [Helix LSP 客户端](https://github.com/helix-editor/helix/blob/master/helix-lsp/src/client.rs) 与 [tower-lsp](https://github.com/ebkalderon/tower-lsp/) 的传输层设计。

```text
editor key event
  → LspClient.didChange(uri, text)
  → language server (rust-analyzer / pyright / ...)
  → diagnostics + completion items
  → editor state (diagnostics map / completion popup)
  → ui.rs 渲染波浪线 / 补全菜单
```

- **收益**：micro-rs 从文本编辑器升级为准 IDE。
- **代价**：异步 LSP 客户端、JSON-RPC over stdio、诊断与补全 UI、需实机安装语言服务器验证。
- **风险**：高。事件循环当前是同步 poll（50ms），需引入后台线程 + 通道，避免阻塞输入。

### P8 — tree-sitter 符号导航（推荐先做）

参考 [tree-sitter 代码导航](https://tree-sitter.github.io/tree-sitter/4-code-navigation.html) 的 tags 思路。复用 `syntax.rs` 已解析好的 AST，无需外部语言服务器。

```text
source text
  → tree-sitter AST (已有)
  → collect_symbols(): 函数 / 类 / 方法 / 结构体定义节点
  → Symbol{name, kind, row, col}
  → Ctrl-T 符号搜索面板 / 侧边大纲
  → 选中 → 光标跳转 + 视图滚动
```

- **收益**：符号搜索、大纲面板、函数间跳转立即可用；纯 Rust，零新增运行时依赖。
- **代价**：符号提取遍历、大纲 UI、光标联动。
- **局限**：跨文件跳转仍受限（不是 LSP），但作为轻量版足够。
- **推荐理由**：AST 现成、风险低、见效快，做完再啃 LSP。

### P6 — 宏录制 / 回放

- 录制用户事件序列，回放时禁止重入，支持持久化到配置文件。
- 补齐官方 micro 的 macro 手感（见 [micro Roadmap](https://github.com/micro-editor/micro/wiki/Roadmap)）。

### P5 — MosaicDisk 扩展（独立项目）

- 仓库：`zeno-ling/mosaicdisk`。
- Top 1000 大文件报表 + 重复文件汇总，发 v0.2.0。
- 与 micro-rs 无代码耦合，可并行推进。

## 决策记录

- 本轮只提交路线图文档，**不实现任何功能**（用户明确"暂时不做"）。
- 一旦决定动工，建议顺序：P8 符号导航 → P10 LSP；P6 / P5 可并行。
