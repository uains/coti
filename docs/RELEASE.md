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
| Tab / Shift+Tab | 缩进/反缩进 |
| 方向键 / Home / End | 光标移动 |

## 已知限制

- Windows 构建已验证；macOS/Linux 未验证。
- 无原生菜单栏；快捷键与文件对话框已接入。
- IME 使用 GPUI 键盘字符事件，尚未接平台预编辑协议。
- 图片以占位框显示，未加载真实图片。
- 无安装包/签名/自动更新。
