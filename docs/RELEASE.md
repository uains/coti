# Coti 0.1.0 — 构建与运行

## 构建

```powershell
./scripts/build-release.ps1
```

产物：`dist/coti.exe`（Windows x64）。

## 运行

```powershell
./dist/coti.exe --gui
./dist/coti.exe --gui .\note.md
```

无参数启动 headless banner，便于 smoke test。

## 快捷键

| 快捷键 | 功能 |
|---|---|
| Ctrl+N | 新建 |
| Ctrl+O | 打开 Markdown |
| Ctrl+S | 保存 |
| Ctrl+Shift+S | 另存为 |
| Ctrl+Z | 撤销 |
| Ctrl+Shift+Z | 重做 |
| Ctrl+F | 搜索 |
| Ctrl+K | 命令面板 |
| Ctrl+Shift+O | 大纲侧栏 |
| Tab / Shift+Tab | 缩进/反缩进 |
| 方向键 / Home / End | 光标移动 |

## 已知限制

- Windows 构建已验证；macOS/Linux 未验证。
- 标题栏内菜单、搜索、大纲、命令面板、状态栏已接入。
- IME 使用 GPUI 键盘字符事件，尚未接平台预编辑协议。
- 本地图片可加载并绘制；远程图片仍是占位。
- 无安装包/签名/自动更新。
