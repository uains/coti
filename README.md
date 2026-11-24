# Coti: 日常书写，日结为札

## 为什么是 Coti
大多数编辑器在“写”和“管”之间是割裂的：写 Markdown 用一个工具，记待办用另一个，周回顾再手动翻找。Coti 把这些动作放进同一个界面，但不强迫你先分类、先建库、先配置。

它的核心交互只有三件事：

写：打开就写，Markdown 实时预览，不需要手动保存

记：任务直接写在当天的记录里，- [ ] 是记录的一部分，不是另一个系统

结：按日/周自动聚合未完成项与关键词，帮你决定什么值得留下

Coti 的名字来自拉丁语cotidianus（每日的）。它不追求“第二大脑”式的宏大叙事，而是做一个你每天愿意打开、打开就能写、写完不丢的轻量工具。

## 开发

当前完成阶段 6（History / IME / 剪贴板 / Heading InputRule），下一步进入阶段 7（`md-layout` 布局引擎）。实施进度见 [docs/IMPLEMENTATION.md](docs/IMPLEMENTATION.md)，逐步变更见 [docs/CHANGELOG.md](docs/CHANGELOG.md)，开发命令与质量门禁见 [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)。

```powershell
cargo run -p md-app
cargo test --workspace
```

