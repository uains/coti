# 变更记录

本文件按实施步骤记录代码、文档与工程配置的变更。每个小步骤一个条目，包含变更内容、验证结果、设计说明和下一步；阶段级进度在 `IMPLEMENTATION.md` 中汇总。

## 2026-11-23 · 步骤 0.1：创建 Cargo Workspace

**状态：** 已完成并确认。

### 变更

- 根 `Cargo.toml` 从单包清单改为 virtual workspace，`resolver = "3"`，当前成员为 `crates/md-app`。
- 原根包 `coti` 的应用入口迁移到 `crates/md-app`：包名 `md-app`，可执行文件名保留 `coti`。
- 删除根 `src/`（原 `lib.rs` 为空，`main.rs` 仅占位输出）。
- 新增 `docs/ARCHITECTURE_DECISIONS.md`，记录 ADR-0001：Workspace 结构与 GPUI 延后接线。
- `Cargo.lock` 重新生成；移除当前未引用的 GPUI 传递依赖，锁文件从 6043 行缩减为 6 行。

### 验证

- `cargo metadata --no-deps`：成功解析。
- `cargo build --workspace`：成功，无警告。
- `cargo test --workspace`：成功（当前无测试）。
- `cargo run -p md-app`：输出 `Coti: 日常书写，日结为札`。

### 设计说明

- 阶段 0～7 不把 GPUI 放入 active dependency graph，避免非 UI 阶段被平台工具链阻塞；GPUI revision 固定在 ADR-0001 中，阶段 8 接入。
- 应用包名使用文档约定的 `md-app`，二进制名保留产品名 `coti`，避免后续安装包与启动命令改名。
- 当前 Workspace 只有 `md-app` 一个成员；0.2 补齐其余六个 crate 并建立依赖边。

### 下一步

- 0.2 创建七个 crate，明确目录、职责和依赖关系。

## 2026-11-23 · 步骤 0.2：创建七个 crate

**状态：** 已完成。

### 变更

- 新增 `crates/md-core`、`md-document`、`md-markdown`、`md-editor`、`md-layout`、`md-render`，与既有 `md-app` 共同组成七个 crate。
- 根 Workspace 成员改为 `crates/*`，并在 `[workspace.dependencies]` 集中声明内部路径依赖。
- 建立依赖边：
  - `md-document -> md-core`
  - `md-markdown -> md-core, md-document`
  - `md-editor -> md-core, md-document`
  - `md-layout -> md-core, md-document`
  - `md-render -> md-core, md-document, md-editor, md-layout`
  - `md-app -> 全部六个下层 crate`
- `md-app` 增加 `lib.rs` 与 `run()`，`main.rs` 仅负责调用，便于后续集成测试与应用装配。
- 每个 crate 写入职责边界文档；空模块阶段用 `use <crate> as _;` 保持声明依赖处于链接状态，避免 Cargo 的 unused-dependency 警告。
- 七个 crate 均设置 `publish = false`。

### 验证

- `cargo metadata --no-deps`：成功解析七个 package。
- `cargo build --workspace`：成功，无警告。
- `cargo test --workspace`：成功（七个 crate 均可进入测试目标）。
- `cargo run -p md-app`：输出 `Coti: 日常书写，日结为札`。
- `cargo tree --workspace`：依赖边与上表一致，无环，且不含 GPUI。

### 设计说明

- `md-render` 当前只声明核心与布局依赖，不接入 GPUI；阶段 8 再增加 `gpui`/`gpui_platform`。
- 依赖方向保持单向：`md-app` 只向下依赖，`md-core` 不依赖任何内部 crate。
- `md-app` 同时保留 lib 与 bin，后续窗口、文件 IO 和菜单装配放在 lib，`main.rs` 只做启动。

### 下一步

- 0.3 定义公共基础类型规范，明确每个类型的归属 crate，避免后续重复定义。

## 2026-11-23 · 步骤 0.3：定义公共基础类型规范

**状态：** 已完成。

### 变更

- 新增 `docs/TYPE_OWNERSHIP.md`，给出七个 crate 的公共类型唯一归属清单、归属原则和禁止事项。
- 新增 ADR-0002：`DocPos`、`Affinity`、`Position`、`BlockGapPosition` 归 `md-document`，不下沉到 `md-core`。
- 明确阶段 1 不实现 `DocPos`；阶段 2～3 再在 `md-document` 中实现。

### 冲突处理

- `IMPLEMENTATION.md` 步骤 1.1 把 `DocPos` 列在 `md-core`，与 `DESIGN.md` §2.2 的 crate 职责冲突。
- 按“设计文档为基线”的原则采用 `md-document` 归属，理由和影响记录在 ADR-0002；阶段 1 验收范围据此调整。

### 验证

- 逐项核对 `DESIGN.md` §2.2、§3、§4 与 `IMPLEMENTATION.md` 阶段 1～3 的类型归属。
- `cargo build --workspace`：成功，无代码变更引入的编译问题。

### 设计说明

- 类型清单覆盖文本、几何、文档、Markdown、编辑、布局、渲染与应用八类公共类型；后续新增类型先登记再实现。
- 本轮只建立规范，不提前实现阶段 1 的 `TextBuf` 或阶段 2 的文档模型。

### 下一步

- 0.4 配置统一错误处理：每个库 crate 使用 `thiserror` 定义可分类错误，`md-app` 使用 `anyhow` 聚合边界错误。

## 2026-11-23 · 步骤 0.4：统一错误处理

**状态：** 已完成。

### 变更

- 新增 `docs/ERROR_HANDLING.md`，定义库 crate 与应用边界的错误分层、命名、source 链和演进规则。
- `md-core` 新增 `TextError`：越界、非 UTF-8 字符边界、非法范围、非法 UTF-16。
- `md-document` 新增 `DocumentError`、`PositionError`，并通过 `#[from]` 承接 `TextError`。
- `md-markdown`、`md-editor`、`md-layout`、`md-render` 分别新增 `MarkdownError`、`EditorError`、`LayoutError`、`RenderError`，下层错误使用 `#[error("{0}")]` + `#[from]` 保留 Display 与 source 链。
- 所有库错误枚举加 `#[non_exhaustive]`；各 crate 提供 `*Result<T>` 别名。
- `md-app` 改用 `anyhow::Result<()>`，`main` 返回错误时由标准入口打印并退出非零状态。
- Workspace 新增 `thiserror = "2"`、`anyhow = "1"` 依赖。

### 验证

- `cargo build --workspace`：成功，无警告。
- `cargo test --workspace`：成功。
- `cargo run -p md-app`：正常输出启动信息，退出码 0。

### 设计说明

- 库 crate 不暴露 `anyhow::Error`，保持错误可分类、可匹配；`anyhow` 只用于应用装配边界。
- 错误变体携带 offset、范围、BlockId 的 index/generation、revision 等定位字段，便于 tracing 和用户提示。
- 错误枚举标注 `#[non_exhaustive]`，后续阶段可以增加变体而不破坏下游匹配。

### 下一步

- 0.5 配置 tracing：库 crate 使用结构化 `tracing` 事件，`md-app` 初始化 subscriber 并支持 `RUST_LOG`。

## 2026-11-23 · 步骤 0.5：配置 tracing 日志

**状态：** 已完成。

### 变更

- Workspace 新增 `tracing = "0.1"`、`tracing-subscriber = { version = "0.3", features = ["env-filter"] }`。
- `md-app` 新增 `logging` 模块：从 `RUST_LOG` 读取过滤级别，未设置时默认 `info`，重复初始化安全。
- `md-app::run()` 在启动时初始化 subscriber，并记录 `starting Coti` 的 info 事件。
- 新增 `docs/OBSERVABILITY.md`，定义日志入口、分层记录内容、span 字段、隐私与性能约束。

### 验证

- `cargo build --workspace`：成功。
- `cargo run -p md-app`：输出带时间戳的 `INFO md_app: starting Coti`，随后输出应用启动信息。
- `RUST_LOG=md_app=debug cargo run -p md-app`：环境变量过滤生效。

### 设计说明

- 只在应用边界初始化 subscriber；库 crate 在真正产生关键操作时再添加 `tracing` 依赖和 span，避免空日志和热路径开销。
- 默认不记录正文，只记录长度、块数、offset、耗时等摘要；正文诊断需要显式开启 `trace`。
- 当前时间戳为 UTC；若发布阶段需要本地时间，再引入 `time`/本地时区支持，不影响日志接口。

### 下一步

- 0.6 配置单元、集成和属性测试基础设施，确保每个 crate 具有基础测试且可独立运行。

## 2026-11-23 · 步骤 0.6：配置单元与集成测试

**状态：** 已完成。

### 变更

- 七个 crate 均加入单元测试：覆盖错误文本、错误分类、source 转发、日志初始化与入口返回值。
- 新增 `crates/md-app/tests/app_smoke.rs` 集成测试：
  - 构造 `TextError → DocumentError → LayoutError → RenderError`，验证跨层 source 链至少三层。
  - 通过 `CARGO_BIN_EXE_coti` 真实启动二进制，断言退出码为 0 且 stdout 包含 `Coti`。
- 修正包装错误的 source 语义：从 `#[error(transparent)]` 改为 `#[error("{0}")]` + `#[from]`，使中间层不会被 `source()` 跳过。
- 新增 `docs/TESTING.md`，记录命令、四层测试、当前测试清单、命名与夹具约定。
- 更新 `docs/ERROR_HANDLING.md` 和 0.4 变更说明，准确描述当前 source 链实现。

### 验证

- `cargo test --workspace`：20 个测试全部通过。
  - `md-core` 3、`md-document` 3、`md-markdown` 2、`md-editor` 3、`md-layout` 3、`md-render` 2、`md-app` 4。
- `cargo test -p md-core`：3 个测试通过，可独立运行。
- `cargo test --workspace --doc`：全部 crate 的文档测试目标可运行。
- `cargo test -p md-app --test app_smoke`：二进制启动与 source 链测试通过。

### 设计说明

- 阶段 0 不引入 `proptest` 的空壳属性测试；属性测试从阶段 1 的文本边界开始，使用真实不变式。
- 集成测试只依赖标准库和被测 crate，不访问网络或用户目录，适合 CI。
- `md-app` 的 lib/bin 拆分让集成测试既能调用装配入口，也能验证真实二进制。

### 下一步

- 0.7 配置 fmt、clippy、CI、依赖分层检查与测试文档入口。

## 2026-11-23 · 步骤 0.7：配置 fmt、clippy 与 CI

**状态：** 已完成。

### 变更

- 根 Workspace 新增统一 lint 配置：
  - Rust：`unsafe_code = "deny"`、`missing_docs = "warn"`、`unused_must_use = "deny"`。
  - Clippy：`all = "warn"`，`dbg_macro = "deny"`，`todo` / `unimplemented` 警告。
- 七个 crate 均启用 `[lints] workspace = true`。
- 新增 `rust-toolchain.toml`（stable + rustfmt + clippy）与 `rustfmt.toml`（Edition 2024、100 列、LF）。
- 新增 `.github/workflows/ci.yml`：Ubuntu / Windows / macOS 三平台执行 fmt、clippy（`-D warnings`）、test 和分层检查，并使用 `--locked` 防止锁文件漂移。
- 新增 `scripts/check-layering.sh` 与 `scripts/check-layering.ps1`，检查 `md-core`～`md-layout` 的依赖树中不出现 GPUI。
- 新增 `docs/DEVELOPMENT.md`，汇总环境、命令、边界、提交与文档索引。
- `README.md` 增加开发入口和当前阶段说明。
- 补上 `main.rs` 的 crate 文档，使 `missing_docs` 在二进制目标上通过。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：20 个测试通过。
- `./scripts/check-layering.ps1`：通过，非 UI crate 依赖树无 GPUI。
- `cargo clippy --locked` / `cargo test --locked`：锁文件无需更新。
- CI workflow 尚未在 GitHub 远端实际运行；本地等价命令全部通过。

### 设计说明

- `missing_docs` 设为 warn 并由 CI 的 `-D warnings` 提升为错误，保证公共 API 从阶段 0 起就有文档。
- `unsafe_code = "deny"` 只作用于本仓库代码，不影响依赖；如未来确需 unsafe，应单独评审并在最小模块内调整。
- 分层检查只校验“非 UI crate 无 GPUI”这一硬约束；完整依赖方向仍由代码审查和 `cargo tree` 保证。
- 未加入 `.gitattributes` 强制行尾，避免影响现有 Windows 工作区；CI 以提交内容为准。

### 下一步

- 阶段 0 汇总验收，更新 `IMPLEMENTATION.md`，等待确认后进入阶段 1。

## 2026-11-23 · 阶段 1 / 步骤 1.1～1.3：文本基础类型与 UTF-8 边界

**状态：** 已完成。

### 变更

- 新增 `md-core::TextOffset`：`#[repr(transparent)]` 的字节偏移 newtype，提供 `ZERO`、`new`、`get`、`checked_add`、`checked_sub`。
- 新增 `md-core::TextRange`：半开区间 `[start, end)`，提供 `new`、`empty`、`start`、`end`、`len`、`is_empty`、`is_valid`、`contains`，以及与 `Range<usize>` 的双向转换。
- 新增 `md-core::TextBuf`：内部持有 `String`，提供 `new`、`as_str`、`into_string`、`len_bytes`、`is_empty`、`is_char_boundary`、`validate_offset`、`validate_range`、`slice`。
- `TextBuf` 不暴露 `&mut String`；所有切片都经过 UTF-8 边界校验。
- 新增 ADR-0003，记录 newtype 选择、半开区间和“允许构造 reversed range、操作时返回错误”的约定。
- 新增 7 个 `md-core` 单元测试：空缓冲、ASCII、CJK、Emoji、reversed range、越界、range 长度语义。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy -p md-core --all-targets -- -D warnings`：通过。
- `cargo test -p md-core`：10 个测试通过（含原有错误测试）。
- `cargo test --workspace`：27 个测试通过。

### 设计说明

- `TextOffset`/`TextRange` 不承担 `DocPos` 语义；`DocPos` 仍按 ADR-0002 留在 `md-document`。
- 允许构造 reversed `TextRange` 是为了避免 `new` panic；真正的合法性由 `TextBuf::validate_range` 判断。
- `TextBuf` 暂用 `String` 实现，优先保证边界正确；长文性能优化留到布局/编辑阶段用基准数据决定。

### 下一步

- 1.4～1.6：grapheme 边界、前后移动、UTF-8/UTF-16 转换。

## 2026-11-23 · 阶段 1 / 步骤 1.4～1.6：Grapheme 与 UTF-16

**状态：** 已完成。

### 变更

- 引入 `unicode-segmentation = "1"`。
- `TextBuf` 新增 `is_grapheme_boundary`、`next_grapheme_boundary`、`prev_grapheme_boundary`，使用 `GraphemeCursor` 按扩展 grapheme 规则移动。
- `TextBuf` 新增 `len_utf16`、`to_utf16`、`from_utf16`、`utf8_to_utf16`、`utf16_to_utf8`。
- `TextError` 新增 `Utf16OutOfBounds`、`NotUtf16Boundary`，分别处理越界和代理对中间位置。
- 新增 9 个测试：组合字符、ZWJ 家庭 Emoji、空/ASCII 边界、非字符边界拒绝、UTF-16 往返、双向 offset 转换、非法 UTF-16。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：36 个测试通过（`md-core` 19 个）。

### 设计说明

- `GraphemeCursor` 使用完整文本和 `chunk_start = 0`；当前阶段不做分块增量，但避免了每次从字符串头部重新枚举。
- `next_grapheme_boundary` / `prev_grapheme_boundary` 对非 UTF-8 字符边界返回 `None`，不静默吸附；调用方必须先保证 offset 合法。
- UTF-16 offset 指向代理对中间时返回 `NotUtf16Boundary`，不猜测前后位置；IME 适配层需要显式处理该错误。
- 用户级左右移动仍由上层调用 grapheme API，禁止对 `TextOffset` 直接 `+1/-1`。

### 下一步

- 1.7～1.8：文本插入、删除、替换，以及区间拆分与合并。

## 2026-11-23 · 阶段 1 / 步骤 1.7～1.8：文本修改与区间拆分

**状态：** 已完成。

### 变更

- `TextBuf` 新增 `to_owned_range`、`insert`、`remove`、`replace`、`split_off`、`append`、`append_buf`、`concat`。
- `TextRange` 新增 `split_at`：在区间内或端点处拆成两个半开区间；超出范围返回 `None`。
- 所有修改操作先完成范围/边界校验，再修改 `String`；失败时缓冲区保持不变。
- 新增 9 个测试：ASCII/CJK/Emoji 插入、非法 offset 不修改、删除/替换原子性、拆分/合并往返、空前缀拆分、`concat`/`to_owned_range`、`TextRange::split_at`。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy -p md-core --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：43 个测试通过（`md-core` 26 个）。

### 设计说明

- `split_off` 保留 prefix、返回 tail，便于后续 Inline Run 拆分与序列化切片。
- `replace` 允许替换为不同字节长度的文本，但范围两端仍必须是 UTF-8 字符边界。
- `TextRange::split_at` 在 start/end 端点也返回结果，便于统一处理空 run；不做隐式裁剪。
- 修改操作目前是 `String::replace_range`，复杂度 O(n)；正确性优先，后续用基准决定是否引入 rope。

### 下一步

- 1.9～1.10：几何类型 `Point`/`Size`/`Rect`/`EdgeInsets`，属性测试与边界测试。

## 2026-11-23 · 阶段 1 / 步骤 1.9～1.10：几何类型与属性测试

**状态：** 已完成。

### 变更

- 新增 `md-core::Point`、`Size`、`Rect`、`EdgeInsets`，使用 `f32`，提供构造、边界、包含、相交、并集、inset、平移等基础方法。
- `Rect` 使用半开区间 `[left, right)` / `[top, bottom)` 的碰撞语义；`intersect` 在无重叠时返回 `None`。
- 引入 `proptest = "1"` 作为 `md-core` dev-dependency。
- 新增 `crates/md-core/tests/text_properties.rs`，覆盖 7 个属性：
  - UTF-16 编解码往返。
  - 每个字符边界的 UTF-8 ↔ UTF-16 offset 往返。
  - grapheme 前进到末尾、后退到起点。
  - 自切片替换恒等。
  - 插入后删除往返。
  - split/concat 往返。
- 更新 `docs/TESTING.md` 的测试清单、属性测试说明和阶段验收。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：56 个测试通过。
  - `md-core` 32 个单元测试 + 7 个属性测试。
- 属性测试生成 ≤64 个 char 的任意 UTF-8 字符串，单次运行约 0.03s。

### 设计说明

- 几何类型只使用 `f32`，不依赖 GPUI 或视觉常量；GPUI 类型转换留在 `md-render`。
- `Rect::inset` 允许产生负尺寸，不在几何层静默 clamp；布局层需要显式处理无效尺寸。
- `Size::is_empty` 把负尺寸视为空，便于布局快速跳过；是否允许负尺寸由布局层策略决定。
- 属性测试只验证已实现的不变式，不提前测试 `DocPos`/Selection。

### 下一步

- 阶段 1 汇总验收，更新 `IMPLEMENTATION.md`，等待确认后进入阶段 2（`md-document` Arena 与 Block 模型）。

## 2026-11-23 · 阶段 2 / 步骤 2.3（第一批）：Inline 基础类型

**状态：** 已完成。

### 变更

- 实际实施顺序调整为 **2.3 → 2.2 → 2.1 → 2.4**，因为 `BlockKind` 引用 `InlineContent`。
- 新增 `md-document` 的 `MarkFlags`（bitflags）、`Marks`、`LinkRef`、`ImageRef`、`AtomKind`、`Run`、`InlineContent`。
- `Run` 支持 Text / Atom 两种形态；Text 长度按 UTF-8 字节，Atom 固定占 1 个 `DocPos` 单位。
- `InlineContent` 支持：
  - `from_text`、`push_run`、`insert_run`、`remove_run`。
  - `push_text`（相同 Marks 自动合并）、`push_atom`。
  - `split_text_run`（按 UTF-8 边界拆分，保留两侧 Marks）。
  - `normalize`（合并相邻同 Marks Text Run、删除空 Text Run）。
  - `validate`（长度、空 Run、相邻同 Marks Run）。
- 新增 `bitflags = "2"` 依赖。
- 新增 11 个 `md-document` 单元测试。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy -p md-document --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：67 个测试通过（`md-document` 14 个）。

### 设计说明

- `InlineContent.runs` 保持私有，只暴露 `&[Run]`；外部不能绕过 `len` 缓存直接修改文本。
- `Marks` 字段公开以便构造，但 `InlineContent` 的修改 API 会维护长度和不变量。
- `split_text_run` 在 run 起点/终点返回 `false`，不制造空 Run；空 Run 由 Normalizer 处理。
- `push_run` 拒绝空 Text Run；`normalize` 仍保留空 Run 清理路径，供解析/逆操作等内部路径使用。

### 下一步

- 2.2：`BlockKind`、`BlockNode` 与父子类型约束。

## 2026-11-23 · 阶段 2 / 步骤 2.2：Block 模型与父子约束

**状态：** 已完成。

### 变更

- 新增 `BlockId`：`index + generation`，提供只读访问与 `Display`；为 2.1 Arena 做准备。
- 新增 `BlockKind` 全部节点类型：Paragraph、Heading、TableCell、CodeBlock、HtmlBlock、MathBlock、FrontMatter、Quote、List、ListItem、Table、TableRow、ThematicBreak、ImageBlock。
- 新增 `FenceStyle`、`Align`。
- 新增 `BlockNode`：持有 `BlockId`、`parent`、`BlockKind`；移动节点保留原 ID。
- 实现容器/叶子判定、`children`/`children_mut`、`inline_content`/`inline_content_mut`、`kind_name`。
- 实现父子约束 `can_be_child_of`：
  - List → 仅 ListItem。
  - Table → 仅 TableRow。
  - TableRow → 仅 TableCell。
  - Quote / ListItem → block-level 节点。
- 实现本地 `BlockKind::validate`：Heading 1..=6、fence 合法性、空 Quote/List/ListItem/Table/TableRow 拒绝、Inline 内容校验。
- 按已确认方案，`BlockNode` 暂不包含 `source` 字段；SourceHint 归属阶段 4 前决定。
- 新增 8 个块模型测试。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy -p md-document --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：76 个测试通过（`md-document` 23 个）。

### 设计说明

- `BlockId` 使用私有字段，避免上层随意构造后误用；`BlockId::new` 仍公开用于反序列化和测试，查不到时返回 `None`。
- 父子约束先放在 `BlockKind`，Document 只负责验证整棵树；这样规则集中、测试直接。
- 空容器在 `BlockKind::validate` 层拒绝；解析器需要在容器为空时补空 Paragraph，而不是放宽校验。
- `Table` 的列数与每行列数一致性需要整棵树上下文，留给 `Document::validate`。

### 下一步

- 2.1：`NodeArena`、Generation、FreeList、暂存与恢复。

## 2026-11-23 · 阶段 2 / 步骤 2.1：NodeArena 与节点生命周期

**状态：** 已完成。

### 变更

- 新增 `NodeArena`、内部 `NodeSlot` 与 `NodeArena::validate`。
- `BlockId` 由 `index + generation` 组成；槽位释放时 generation 递增，旧 ID 立即失效。
- 实现 `alloc`、`get`、`get_mut`（crate 内）、`contains`、`iter`。
- 实现三种生命周期：
  - `free`：立即释放并回收槽位，适合无 Undo 需求的清理。
  - `stage`：节点转入 staged 状态，从 live 视图隐藏，但保留原 ID 和内容。
  - `restore`：用原 ID/generation 恢复 staged 节点。
  - `release_staged`：永久释放 staged 节点并递增 generation。
- FreeList 复用槽位；generation 达到 `u32::MAX` 后不再回收该槽位，避免 generation 回绕让旧 ID 重新有效。
- `validate` 校验 live/staged/free 数量、节点 ID 与槽位匹配、free list 无重复/越界/占用。
- 新增 8 个 Arena 测试：分配、释放后复用、generation 失效、staging/restore/release、非法操作、迭代跳过、free list、`get_mut`。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy -p md-document --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：84 个测试通过（`md-document` 31 个）。

### 设计说明

- staged 节点不计入 `len`，但保留槽位与 generation；只有 `release_staged` 才递增 generation，因此 Undo 可以用原 `BlockId` 恢复。
- 事务层应使用 `stage`/`restore`/`release_staged`；`free` 明确标注为非 Undo 安全的快速路径。
- `alloc` 的 `u32` 索引空间耗尽会 panic；这属于 40 亿节点以上的硬上限，不是可恢复的用户错误。
- `NodeArena` 不校验 parent 是否存在或类型是否匹配；这些规则由 `Document` 统一验证。

### 下一步

- 2.3 第二批：Inline Normalizer 已完成基础合并；补充映射准备与更多边界测试。
- 之后进入 2.4 `Document`。

## 2026-11-23 · 阶段 2 / 步骤 2.3（第二批）：Normalizer 报告与 Atom 插入

**状态：** 已完成。

### 变更

- 新增 `InlineNormalizeChange`：记录 `removed_empty_runs`、`merged_text_runs`，为阶段 5 的 PositionMapping 提供变化摘要。
- 新增 `InlineContent::normalize_with_report`；`normalize` 改为其布尔包装。
- 新增 `InlineContent::insert_atom(index, kind, marks)`。
- 新增 3 个测试：Atom 指定位置插入、Normalizer 合并报告、干净内容空报告。
- `InlineNormalizeChange` 加入 `md-document` 公开导出。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy -p md-document --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：87 个测试通过（`md-document` 34 个）。

### 设计说明

- Normalizer 不直接吞掉变化：需要映射时调用 `normalize_with_report`，由上层转换为 PositionMapping。
- `removed_empty_runs` 目前主要服务解析/逆操作等内部路径；公开插入 API 已拒绝空 Text Run。
- Atom 插入复用 `insert_run`，因此统一维护 `len` 与索引边界。

### 下一步

- 2.4：`Document` 根、受控结构修改、快照、Validator 与随机结构不变式测试。

## 2026-11-23 · 阶段 2 / 步骤 2.4：Document、Validator 与快照

**状态：** 已完成。

### 变更

- 新增 `Document`：
  - `new` 创建含一个空 Paragraph 的合法文档。
  - `insert_block`、`append_block` 支持根级和容器内插入。
  - `remove_block` 删除整棵子树；根为空时自动补空 Paragraph。
  - `move_block` 保留原 `BlockId`，更新 parent，并拒绝环、非法子类型、越界和“搬空容器”。
  - `snapshot` / `restore` 提供整树快照。
  - `node`、`parent`、`children`、`kind`、`root_children`、`block_count`、`leaf_count`、`is_ancestor`。
- 新增 `DocumentSnapshot`。
- 新增 `Document::validate`：
  - 调用 Arena 校验。
  - 拒绝 staged 节点、空 root、孤儿节点、重复可达、循环、错误 parent。
  - 校验 root/容器子节点类型。
  - 校验 Heading、fence、空容器、Inline 内容。
  - 校验 Table 每行列数与 `aligns` 一致。
- 新增 13 个 `Document` 单元测试和 3 个 `proptest` 结构属性测试。
- `md-document` 增加 `proptest` dev-dependency。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：103 个测试通过。
  - `md-document` 47 个单元测试 + 3 个属性测试。
- 属性测试覆盖随机插入根段落、随机删除根块、随机移动根块并保留 ID。

### 设计说明

- `Document` 的结构修改允许短暂的不完整容器（例如先插入空 Quote，再追加子块），`validate` 是提交前的统一门槛；这避免构造顺序导致无法构建嵌套树。
- `remove_block` 目前直接释放节点，适合阶段 2 的结构测试；阶段 5 的 Transaction/History 将改用 Arena staging 或 inverse snapshot。
- `Document` 暂不含 `DocumentIndex`；阶段 3 再加入叶子顺序、长度前缀和位置映射。
- `BlockNode` 仍不含 `source` 字段；SourceHint 归属按已确认方案留到阶段 4 前决定。

### 下一步

- 阶段 2 汇总验收，更新 `IMPLEMENTATION.md`，等待确认后进入阶段 3（DocumentIndex、DocPos、Selection）。

## 2026-11-24 · 阶段 3 / 步骤 3.1：DocumentIndex 与 Fenwick 前缀和

**状态：** 已完成。

### 变更

- `BlockKind` 新增 `leaf_len` 与 `is_valid_leaf_offset`：
  - Inline 叶使用 `InlineContent.len`。
  - Raw 叶使用 UTF-8 字节长度。
  - ThematicBreak / ImageBlock 占 1 个 `DocPos` 单位。
- `InlineContent` 新增 `is_valid_offset`：Text 必须落在 UTF-8 边界，Atom 只允许 start/end。
- 新增 `DocumentIndex`：
  - `leaf_order`、`leaf_lengths`、`block_locations`、`parent_index`。
  - Fenwick Tree 维护 `leaf_len + 1` 的前缀和，用于 Separator 感知的 `DocPos` 查找。
  - `leaf_at`、`leaf_start`、`leaf_len`、`block_location`、`parent`、`leaf_index`。
  - `rebuild` 与 `validate`。
- `Document` 增加 `index` 字段：
  - `new` 时构建。
  - `insert_block` / `remove_block` / `move_block` 后重建。
  - `snapshot` / `restore` 包含索引。
  - `validate` 增加索引一致性检查。
- 新增 9 个测试：索引空文档、Separator 起点、容器位置、结构变更重建、陈旧索引检测、Fenwick lower_bound、叶长度与 offset 边界。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：112 个测试通过（`md-document` 56 个单元测试 + 3 个属性测试）。
- `Document::validate` 在测试中同时覆盖树结构与索引一致性。

### 设计说明

- Fenwick 每个 cell 存 `leaf_len + 1`：前缀和直接包含叶子之间唯一的 Block Separator；总长度再减 1，避免尾部多余 Separator。
- 第一版按设计文档采用“结构修改后重建”，不做增量维护；先保证正确性，后续用性能数据决定是否增量更新。
- 索引是派生缓存；`DocumentIndex::validate` 通过与 `rebuild` 结果逐字段比较来发现陈旧或损坏。
- 非法/暂时不完整的文档仍允许重建部分索引，`Document::validate` 会以树结构检查为准。

### 下一步

- 3.2：`DocPos`、`Affinity`、`Position`、`BlockGapPosition`、Separator 两端解析与双向转换。

## 2026-11-24 · 阶段 3 / 步骤 3.2：DocPos、Affinity 与 Position 转换

**状态：** 已完成。

### 变更

- 新增 `DocPos`：独立 newtype，提供 `ZERO`、`new`、`get`、`checked_add`、`checked_sub`、`Display` 与 `usize` 转换。
- 新增 `Affinity`：`Upstream` / `Downstream`，默认 `Upstream`。
- 新增 `Position { block_id, offset, affinity }`。
- 新增 `BlockGapPosition { parent: Option<BlockId>, child_index, affinity }`，用 `None` 表示根级间隙。
- `DocumentIndex` 新增：
  - `resolve_position(document, DocPos, Affinity)`。
  - `to_doc_pos(document, Position)`。
  - `is_valid_doc_pos`。
- Separator 解析规则：
  - `Upstream` 优先前叶末尾。
  - `Downstream` 优先后叶开头。
  - 空叶子在 `Downstream` 下保持可定位。
- `PositionError` 新增 `OffsetOutOfBounds`；非法 UTF-8/Atom 中间 offset 返回 `IllegalBoundary`。
- 新增 7 个位置测试：Separator 两端、空叶子、全长度至少一种 Affinity 往返、错误分类、CJK 码点内部拒绝。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：119 个测试通过（`md-document` 63 个单元测试 + 3 个属性测试）。

### 设计说明

- `DocPos` 是 UTF-8 字节与逻辑单位的混合坐标；`resolve_position` 会拒绝落在 UTF-8 码点或 Atom 内部的线性位置，不静默吸附。
- Separator 两端是歧义位置：不对“任意 Affinity 都往返”作承诺，只要求每个合法 `DocPos` 至少有一种 Affinity 可无损表示。
- 容器不贡献长度；`BlockGapPosition` 不强行映射到唯一 `DocPos`，留给结构操作与 Editor policy。
- `to_doc_pos` 通过叶子局部合法 offset 判定，防止调用方绕过 UTF-8/Atom 边界。

### 下一步

- 3.3：`Anchor`、`Selection`、`SelectionSet`、Node/TableCells 选择与多选区校验。

## 2026-11-24 · 阶段 3 / 步骤 3.3：Selection 与 SelectionSet

**状态：** 已完成。

### 变更

- 新增 `Anchor`、`CellPos`、`SelectionKind`、`Selection`。
- 支持三种选择：
  - `Range`：线性文本范围，保留 anchor/head 方向。
  - `Node(BlockId)`：整块选择。
  - `TableCells { table, start, end }`：表格单元格矩形范围。
- 新增 `SelectionSet`：
  - `new`、`from_selections`、`push`。
  - `sort_descending` 按 DocPos 倒序排序，并保持 primary 指向原选择。
  - `primary`、`primary_selection`、`selections`、`iter`、`set_primary`。
  - `validate` 校验空集、primary、每个 Selection、排序与 Range 重叠。
- 新增 `SelectionError` 与 `SelectionResult`。
- `Range` 校验通过 `DocumentIndex::resolve_position` 拒绝越界和非法 UTF-8/Atom 边界。
- `TableCells` 校验 table 存在、是 Table、行列在 `aligns/rows` 范围内、start ≤ end。
- 新增 7 个 Selection 测试。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：126 个测试通过（`md-document` 70 个单元测试 + 3 个属性测试）。

### 设计说明

- `Node` / `TableCells` 的 anchor/head 使用 `DocPos::ZERO` 占位，不参与其语义校验；真正语义由 kind 决定。
- `TableCells` 增加 `table: BlockId`，否则无法在只有行列坐标时定位所属表格；这是对设计示例的必要补充。
- `SelectionSet` 排序后保持 primary 指向原 primary 选择，而不是简单保留原索引。
- 多选区重叠只对 `Range` 之间强制检查；Node/TableCells 与 Range 的交叉语义留给 Editor policy。
- `Selection` 使用 `f32 goal_x`，因此只实现 `PartialEq`，不实现 `Eq`。

### 下一步

- 3.4：位置/选区属性测试，覆盖空文档、空段落、中文、Atom、多层 Quote/List/Table，以及阶段 3 文档汇总。

## 2026-11-24 · 阶段 3 / 步骤 3.4：位置与选区属性测试

**状态：** 已完成。

### 变更

- 新增 `crates/md-document/tests/position_properties.rs`。
- 构造包含以下结构的合法文档夹具：
  - 空 Paragraph。
  - 中文文本。
  - Text + SoftBreak Atom + Text。
  - 多层 Quote。
  - 嵌套 List / ListItem。
  - Table / TableRow / TableCell。
- 新增 3 个属性测试：
  - 每个合法 `DocPos` 至少有一种 Affinity 可往返。
  - 每个叶子的所有合法局部 offset 都能转换回对应 `DocPos`。
  - 随机文档上的多选区集合排序与 `validate` 通过。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：129 个测试通过。
  - `md-document` 70 个单元测试 + 6 个属性测试。
- 属性测试覆盖：空文档性质、空段落、中文、Atom、嵌套容器、表格单元格。

### 设计说明

- 属性测试先过滤非 UTF-8/Atom 边界的 `DocPos`；非法位置由 `resolve_position` 返回 `IllegalBoundary`，不参与往返断言。
- Roundtrip 断言采用“至少一种 Affinity 成立”，与 Separator 两端歧义语义一致。
- 多选区属性测试只构造不重叠的 collapsed Range；重叠检测由单元测试覆盖。
- 文档夹具在属性测试中先执行 `Document::validate`，避免用非法树测试位置系统。

### 下一步

- 阶段 3 汇总验收，更新 `IMPLEMENTATION.md`、`TESTING.md`，等待确认后进入阶段 4（`md-markdown`）。

## 2026-11-24 · 阶段 4 / 步骤 4.1：SourceHint 接入文档模型

**状态：** 已完成。

### 变更

- 新增 `md-document::source` 模块：`SourceHint`、`StyleHints`、`HeadingStyle`、`ListStyleHint`、`HardBreakStyle`。
- `BlockNode` 增加 `source: SourceHint`，提供 `source()` 访问。
- `NodeArena::alloc_with_source` 与 `Document::insert_block_with_source` / `append_block_with_source`。
- `Document` 新增：
  - `new_empty`：供 Markdown importer 从空树开始构建。
  - `source` / `set_source`。
  - `set_inline_content`。
  - `raw_text_mut`。
  - `set_list_item_task`。
- `Document::validate` 新增 pristine SourceHint 的 range/UTF-8 边界校验。
- 新增 ADR-0007：SourceHint 移入 `md-document`，`md-markdown` 不再拥有侧表。
- 新增 4 个 SourceHint 测试。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：133 个测试通过（`md-document` 74 个单元测试 + 6 个属性测试）。

### 设计说明

- SourceHint 是单一数据源：Parser 写入、Editor 维护、Serializer 读取；避免侧表与节点生命周期不同步。
- `md-document` 只保存 Markdown 风格元数据，不解析 Markdown；风格字段集中在 `source` 模块。
- `Document::new_empty` 是受控的 importer 入口；解析器必须在返回前填入至少一个根块。
- pristine SourceHint 的 range 在 `validate` 时检查 UTF-8 边界与原文长度。

### 下一步

- 4.2：pulldown-cmark Parser/Builder，构建 Block/Inline/Marks/Atom 并写入 SourceHint。

## 2026-11-24 · 阶段 4 / 步骤 4.2：Markdown Parser/Builder

**状态：** 已完成。

### 变更

- 引入 `pulldown-cmark = "0.13"`（锁定到 0.13.6）。
- 新增 `MarkdownDocument`：
  - 持有 `Document`、原始 `Arc<str>` 与 dirty 标记。
  - `parse` 返回未编辑文档；`serialize` 未编辑时逐字节返回原文。
- 新增 `parser` 模块，启用：
  - CommonMark。
  - GFM。
  - Tables、Strikethrough、TaskLists、Footnotes、Math、YAML MetadataBlock。
- Builder 支持：
  - Heading、Paragraph、BlockQuote、List、ListItem、Table/TableRow/TableCell。
  - CodeBlock、HtmlBlock、FrontMatter、ThematicBreak、MathBlock（DisplayMath）。
  - Strong/Emphasis/Strikethrough、Code、Link、Image、SoftBreak、HardBreak、InlineMath、FootnoteRef。
  - TaskListMarker 写入 ListItem.task。
  - 每个块写入 pristine SourceHint（原始 Arc + range）。
- 新增 5 个 parser 测试：标题/段落/引用、列表/任务、代码/表格/分隔线、链接/图片/Atom、Frontmatter/HTML。
- 新增 `serializer` 占位：dirty 序列化在 4.3 实现；pristine 仍返回原文。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：138 个测试通过。
- 5 个 parser 测试全部通过 `Document::validate` 与逐字节 pristine 输出。

### 设计说明

- 空容器由 Builder 补空 Paragraph，保证 Document 不变量。
- FootnoteDefinition / DefinitionList 暂映射为 Quote 容器，确保内容不丢失；这是阶段 4 的已知模型近似。
- inline image 的 alt 目前未保留在 `AtomKind::Image` 中；仅保存 ImageRef，后续需要扩展 AtomKind 或 ImageRef。
- `MarkdownDocument::serialize` 的 dirty 路径当前是占位，下一步替换为真正的 Serializer。

### 下一步

- 4.3：Serializer：Inline Writer、Block Writer、Marks 输出、列表序号、代码围栏、表格、Raw、dirty 重新序列化。

## 2026-11-24 · 阶段 4 / 步骤 4.3：Markdown Serializer

**状态：** 已完成。

### 变更

- `serializer` 从占位替换为规范化 Markdown 输出。
- 支持 Block：
  - Paragraph、Heading、TableCell。
  - CodeBlock（围栏字符/长度、语言、代码内容）。
  - HtmlBlock、MathBlock、FrontMatter（Raw）。
  - Quote（逐行 `> ` 前缀）。
  - List/ListItem（有序序号、无序 `- `、task checkbox、嵌套缩进）。
  - Table/TableRow（表头、对齐分隔行、单元格）。
  - ThematicBreak、ImageBlock。
- 支持 Inline：
  - Strong、Emphasis、Strike、Highlight、Underline、Code。
  - Link 与图片 Atom。
  - HardBreak、SoftBreak、InlineMath、FootnoteRef。
  - 文本转义与代码 span 反引号长度选择。
- `MarkdownDocument::serialize` 的 dirty 路径改为调用 Serializer；未编辑路径仍逐字节返回原文。
- 新增 3 个 Serializer 测试。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：141 个测试通过（`md-markdown` 10 个）。
- Parser 的 5 个测试继续通过 `Document::validate` 与 pristine 字节相等。

### 设计说明

- dirty 序列化目前统一走规范 Writer，不读取 SourceHint 的 pristine 片段；等 Editor 能准确标记脏节点后，再启用“未修改子块直接输出原文”。
- List 的 tight/loose、引用嵌套缩进和 HTML 内联保真仍是近似；阶段 4 后续用夹具暴露并修正。
- inline image 的 alt 仍未保留；`AtomKind::Image` 只有 ImageRef，需要在后续步骤扩展。
- 代码围栏默认使用反引号，未复用原 fence 字符；SourceHint.style.code_fence 已预留。

### 下一步

- 4.4：生成 200+ Markdown 夹具，建立 AC-01 逐字节保真测试与 parser/serializer 回归集。

## 2026-11-24 · 阶段 4 / 步骤 4.4：200+ Markdown 夹具与 AC-01 保真测试

**状态：** 已完成。

### 变更

- 在 `crates/md-markdown/tests/fixtures/markdown/` 生成 **200 个 Markdown 夹具**：
  - 20 类模板 × 10 个变体。
  - 覆盖标题、段落、强调、列表、任务列表、嵌套列表、引用、代码围栏/缩进代码、表格、链接、图片、Frontmatter、脚注、数学、HTML、中文、Emoji、组合字符、硬/软换行、Setext 标题、分隔线、转义、混合结构。
- 新增 `crates/md-markdown/tests/fixtures.rs`：
  - 夹具数量 ≥200。
  - 每个夹具 `parse → validate → serialize` 逐字节相等。
  - dirty canonical 输出必须能重新 parse 且 validate。
  - dirty canonical 二次序列化必须稳定（幂等）。
- 新增 `BlockKind::FootnoteDefinition { label, children }`：
  - Parser 不再把脚注定义降级为 Quote。
  - Serializer 输出 `[^label]:` 并缩进子块。
- MathBlock dirty 输出使用 `$$` 包裹并裁剪首尾换行；FrontMatter 输出恢复 `---` 分隔符。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：145 个测试通过。
  - `md-markdown` 10 个单元测试 + 4 个夹具集成测试。
- 200 个夹具全部通过：
  - pristine 逐字节相等。
  - dirty canonical reparse + validate。
  - dirty canonical 稳定性。
- `./scripts/check-layering.ps1`：通过。

### 设计说明

- 200 篇为合成夹具，不是外部真实语料；AC-01 的“真实 Markdown”要求仍需在阶段 11 前补充真实语料复验。
- canonical 稳定性目前覆盖 200 个夹具，但 `parse(serialize(doc)) ≅ doc` 的结构同构断言尚未实现；当前以 reparse + validate + 二次序列化稳定作为过渡门槛。
- Dirty 路径仍统一规范化输出，没有利用 `SourceHint` 的 pristine 子块与 StyleHints；逐块保真优化留到 Editor 能准确标记脏节点之后。
- inline image 的 alt 仍未保留；阶段 4 的夹具不依赖该字段。
- DefinitionList 仍映射为 Quote 容器；属于已知模型近似。

### 下一步

- 阶段 4 汇总验收，更新 `IMPLEMENTATION.md`、`TESTING.md`，等待确认后进入阶段 5（`md-editor`）。

## 2026-11-24 · 阶段 5 / 步骤 5.1：Intent、Editor 状态与静态 Rule 表

**状态：** 已完成；阶段 5 仍在进行中。

### 变更

- 新增 `Intent`、`MoveDirection`、`BlockStyle`，覆盖 InsertText、Newline、Delete、Move、Marks、BlockKind、List、Indent、Paste、Undo/Redo、IME。
- 新增 `EditorConfig`、`Composition`、`Editor`：
  - document、selection、pending_marks、composition、config。
  - document/state revision。
  - `new`、`with_selection`、`set_selection`、`dispatch` 与访问器。
- 新增静态有序 Rule 表：
  - `RuleId` 稳定数值。
  - `Ctx`、`Rule { id, guard }`、`match_rule`。
  - 当前实现 MoveLeft/MoveRight/MoveDocStart/MoveDocEnd 四条规则。
- 新增 `DocumentIndex::prev_cursor` / `next_cursor`，处理 grapheme、Atom 和跨叶移动。
- 新增 `InlineContent::prev_offset` / `next_offset`。
- `md-editor` 新增 22 个测试；`md-document` 新增 2 个 cursor 测试。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：167 个测试通过（`md-editor` 22 个）。

### 设计说明

- Rule 表采用静态有序数组，guard 顺序即优先级；apply 仍由 Editor 按 RuleId 执行，等 Operation/Transaction 落地后改为统一 apply。
- Move 只处理 Range 选择；Node/TableCells 移动返回 InvalidIntent。
- 多选区 Move 逐项映射 head，排序后由 `SelectionSet::from_selections` 保持 primary。
- 其他 Intent 目前尚未注册规则，返回 InvalidIntent；这是阶段 5 进行中的明确状态，不是最终能力。

### 下一步

- 5.2：Operation 原语与 inverse（文本/Inline/Block/Move/Split/Join）。
- 5.3：Transaction 快照回滚、preflight、validate、ChangeSet。
- 5.4：PositionMapping。
- 5.5：Enter/Backspace/DeleteForward/Tab/DeleteRange 规则表与夹具。

## 2026-11-24 · 阶段 5 / 步骤 5.4～5.5（第一批）：PositionMapping 与文本规则

**状态：** 5.4 基础完成；5.5 部分完成；阶段 5 仍在进行中。

### 变更

- 新增 `PositionMapping`、`MappingOp`、`MappedPosition`：
  - Insert、Delete、Move 三类线性映射原语。
  - 顺序组合 `compose`。
  - 删除范围内的位置标记 `deleted`。
- 新增 `INSERT_TEXT`、`DELETE_BACKWARD` 两个 RuleId 与 guard。
- `Editor::dispatch` 接入 Transaction：
  - `InsertText` 在当前 Range 内执行 `ReplaceText`，光标移到插入文本之后。
  - `DeleteBackward` 删除前一个 grapheme/atom；非折叠选区删除整个范围。
- 新增 8 个 editor/mapping 测试；`md-editor` 目前 44 个测试。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：193 个测试通过。
  - `md-editor` 44 个测试。

### 设计说明

- PositionMapping 当前是线性全局原语，尚未与 Block Split/Join、Normalizer 变更组合。
- Insert/Delete 只支持同一叶子内的 Range；跨块编辑返回 `InvalidIntent`。
- 文本插入使用 Transaction；但 Transaction 尚未发布 PositionMapping，Editor 直接按长度移动光标。
- Rule 表已开始接入 Transaction，但 Enter/DeleteForward/Tab/DeleteRange/List/Quote/Table 规则仍未实现。

### 下一步

- 5.5 第二批：Enter、DeleteForward、DeleteRange、Tab 与列表/引用规则。
- 5.6：随机 Intent 属性测试、History 接口准备、阶段 5 汇总。

## 2026-11-24 · 阶段 5 / 步骤 5.2～5.3：Operation 与 Transaction

**状态：** 已完成；阶段 5 仍在进行中。

### 变更

- `InlineContent` 新增 `split_at` 与 `replace_text`，支持保留 Marks 的范围替换、拆分和删除。
- 新增 `Operation`：
  - `ReplaceText`、`SetInlineContent`、`SetRawText`。
  - `InsertBlock`、`RemoveBlock`、`MoveBlock`、`SetBlockKind`。
- 每个 Operation 返回 `AppliedOperation { inverse, affected }`。
- `Document` 新增 `set_raw_text`、`set_block_kind`。
- 新增 `Transaction`：
  - base revision 检查。
  - 顺序 apply，收集 inverse 与 affected。
  - 任一步失败或 `Document::validate` 失败时恢复快照。
  - 成功后递增 document revision，返回 `ChangeSet`。
- 新增 6 个 Transaction 测试、8 个 Operation 测试、4 个 Inline 替换/拆分测试。
- `md-editor` 目前 36 个测试。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：185 个测试通过。
  - `md-editor` 36 个测试。
- Transaction 测试覆盖成功提交、空事务、陈旧 revision、操作失败回滚、validate 失败回滚。

### 设计说明

- 采用已确认的“快照回滚 + inverse”策略：Document 快照保证失败强一致，Operation inverse 供阶段 6 History 使用。
- `RemoveBlock` 目前只允许叶子块；容器删除的 inverse 需要子树快照/staging，留到后续。
- `RemoveBlock` 的 inverse 是重新插入同 Kind 的新 BlockId，不保证原 ID；Identity-preserving 恢复需要 staging/History 设计。
- `SetBlockKind` 只支持叶子块；容器结构替换不在本批。
- Rule 表尚未接入 Transaction；`Editor::dispatch` 仍只执行 Move 规则。
- SplitBlock/JoinBlocks 与 PositionMapping 尚未实现。

### 下一步

- 5.4：PositionMapping 原语（Text/Inline/Block/Split/Join/Move/Normalizer）。
- 5.5：Enter/Backspace/DeleteForward/Tab/DeleteRange 规则表，接入 Transaction。
- 5.6：随机 Intent 属性测试与阶段 5 汇总。

## 2026-11-24 · 阶段 5 / 步骤 5.5（第二批）：Enter 规则

**状态：** 已完成；阶段 5 仍在进行中。

### 变更

- 新增 `RuleId::ENTER` 与 guard。
- `Editor::dispatch` 支持 `InsertNewline`：
  - 折叠光标处用 `InlineContent::split_at` 拆分当前块。
  - 当前块保留左半内容，新 Paragraph 插入同一父容器。
  - 通过 Transaction 提交并定位到新块起点。
- 新增 3 个 Enter 测试。
- `md-editor` 47 个测试。

### 验证

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test -p md-editor`：47 个测试通过。

### 已知限制

- Enter 仅支持折叠 Range 与 Inline 叶；非折叠选区、Heading/List/Quote/Table 专用规则未实现。
- 跨块 DeleteForward/DeleteRange/Tab 仍待实现。

## 2026-11-24 · 阶段 5 / 步骤 5.5（第三批）：DeleteForward

- 新增 `RuleId::DELETE_FORWARD` 与 guard。
- 折叠光标删除后一个 grapheme/atom；非折叠 Range 删除整个范围。
- 跨块删除仍明确拒绝。
- `md-editor` 50 个测试；全 Workspace 199 个测试通过。

## 2026-11-24 · 阶段 5 / 步骤 5.5（第四批）：SplitBlock / JoinBlocks

- `Document` 新增 `split_block`、`join_blocks`，内部快照回滚。
- Paragraph 分裂为两个 Paragraph；Heading 分裂为 Heading + Paragraph。
- Join 合并 Inline 内容、保留左块 ID/kind，删除右块。
- `Operation` 新增 `SplitBlock`、`JoinBlocks` 及 inverse。
- `md-editor` 52 个测试；全 Workspace 204 个测试通过。
- 下一步：DeleteRange 跨块删除、Tab/List/Quote/Table 规则。

## 2026-11-24 · 阶段 5 / 步骤 5.5（第五批）：跨块 DeleteRange

- `Editor` 非折叠选区支持同父 Inline 叶的跨块删除：
  - 删除首叶后缀、尾叶前缀。
  - 移除中间叶。
  - Join 首尾，保留左叶 ID/kind。
- 跨容器/非 Inline 叶明确拒绝。
- `md-editor` 53 个测试；全 Workspace 205 个测试通过。
- 仍待实现：Tab/Shift+Tab、List/Quote/Table 专用规则、随机 Intent 属性测试。

## 2026-11-24 · 阶段 5 / 步骤 5.5（第六批）：Tab / Shift+Tab

- 新增 `RuleId::CHANGE_INDENT` 与 guard。
- CodeBlock：Tab 插入 `\t`，Shift+Tab 删除前一个 `\t`。
- Paragraph/Heading/TableCell：Tab 明确 no-op，遵循设计 T06。
- ListItem 缩进/反缩进尚未实现，需嵌套 List 结构规则。
- `md-editor` 56 个测试；全 Workspace 208 个测试通过。

## 2026-11-24 · 阶段 5 / 步骤 5.5（第七批）：ListItem 缩进/反缩进

- 新增 `Operation::IndentListItem` / `OutdentListItem`。
- Indent：挂到前一同级 ListItem 的末个 List 下；无嵌套 List 时创建同风格 List。
- Outdent：移动到父 ListItem 之后的祖父 List；旧 List 空时删除。
- `Document::move_block_allow_empty` 支持结构 Cleanup。
- Tab 规则识别 ListItem 并使用 Transaction；CodeBlock 行为保留。
- 首项 Indent 为明确 no-op。
- `md-editor` 59 个测试；全 Workspace 211 个测试通过。

## 2026-11-24 · 阶段 5 / 步骤 5.5（第八批）：TableCell Tab 导航

- Tab/Shift+Tab 识别 TableCells 选择与 TableCell 内的文本光标。
- 下一格 / 上一格；跨行移动到下一行首格/上一行末格。
- 末格 Tab、首格 Shift+Tab 为 no-op；追加新行尚未实现。
- `md-editor` 62 个测试；全 Workspace 214 个测试通过。

## 2026-11-24 · 阶段 5 / 步骤 5.5（第九批）：末格追加表格行

- 新增 `Operation::AppendTableRow` / `RemoveTableRow`。
- 末格 Tab 追加同列数空行并选中新行首格。
- RemoveTableRow 仅允许删除末尾空行，作为 Append 的 inverse。
- `md-editor` 64 个测试；全 Workspace 216 个测试通过。

## 2026-11-24 · 阶段 5 / 步骤 5.5（第十批）：Quote / Table Enter

- `InlineContent::insert_atom_at` 支持任意合法 offset 插入 Atom。
- TableCell Enter 插入 HardBreak Atom，不拆分表格。
- Quote 内 Enter 复用段落分裂，保持 Quote 容器。
- 新增 2 个 Enter 测试；`md-editor` 66 个测试；全 Workspace 218 个测试通过。

## 2026-11-24 · 阶段 5 / 步骤 5.5（第十一批）：TableCell Backspace

- 光标在 TableCell 起点时回退到前一格末尾。
- 跨行回退到上一行末格；首格 Backspace 为 no-op。
- 新增 3 个测试；`md-editor` 69 个测试；全 Workspace 221 个测试通过。

## 2026-11-24 · 阶段 5 / 步骤 5.5（第十二批）～5.6：Quote Backspace 与随机 Intent 属性测试

- 新增 `Operation::OutdentQuoteBlock` / `IndentBlockIntoQuote`。
- Quote 首子块 Backspace 移出 Quote；唯一子块移出后删除空 Quote；非首子块保持 Quote。
- 新增 `crates/md-editor/tests/editor_properties.rs`：
  - 随机 InsertText/DeleteBackward 后 Document 始终 valid。
  - Enter + InsertText 序列 valid。
  - 陈旧 Transaction 不修改文档。
  - 随机光标位置后 Selection 仍合法。
- `md-editor` 74 个单元测试 + 4 个属性测试；全 Workspace 230 个测试通过。

## 2026-11-24 · 阶段 5 汇总：Headless 编辑核心完成

- 阶段 5 核心闭环完成：Intent → Rule → Operation → Transaction → Document/Selection 更新。
- 已实现规则：Move、InsertText、DeleteBackward/Forward、Enter、DeleteRange、ChangeIndent（代码块/列表/表格）、TableCell 导航与追加行、Quote Backspace。
- 已知缺口：完整 DESIGN §8 规则表、Node/TableCells 删除、Split/Join PositionMapping、History（阶段 6）、根级 ListItem 退出列表。
- 阶段 5 按“核心范围”验收；完整规则覆盖作为后续增量。

## 2026-11-24 · 阶段 6：History / IME / 剪贴板 / 标题 InputRule

- **History**：
  - `HistoryEntry` 保存 forward/inverse、前后 Selection、group、revision、时间戳、字节估算、逻辑 state id。
  - Undo/Redo Stack；`Unknown` → `Intent::Undo/Redo` 规则。
  - 连续 InsertText 500ms 合并；Save Point 阻止跨保存点合并。
  - max_entries / max_bytes 淘汰；`save_point`、`is_modified`、`can_undo/redo`。
  - 10 步 undo/redo、Selection 恢复、Redo 清理、限额测试。
- **IME**：
  - `ImeSetMarked`、`ImeCommit`、`ImeCancel`。
  - Composition 状态；Commit 作为一条历史记录；一次 Undo 完整撤销。
- **剪贴板**：
  - `copy_text` 同块与跨块纯文本复制（换行连接）。
  - `paste_text` 按空行拆分为段落，整体合并为一条历史记录。
- **InputRule**：
  - `# `～`###### ` 在 Paragraph 起始处转换为对应 Heading，单次 Undo 回退。
- `md-editor` 89 个单元测试 + 4 个属性测试；全 Workspace 245 个测试通过。

## 2026-11-24 · 阶段 6 汇总：输入核心完成

- 阶段 6 按核心范围验收：History、IME Commit/Cancel、纯文本剪贴板、Heading InputRule。
- 已知增量：Markdown/HTML 剪贴板、List/Quote/CodeBlock InputRule、跨块 Composition、IME marked range 高亮、真实输入法集成测试。

## 2026-11-24 · 阶段 7：`md-layout` 纯几何布局引擎

- **7.1 测量**：`ResolvedStyle`、`LayoutEnv`、`MeasureStyle`、`PaintStyle`、`TextMeasurer`、`FakeMeasurer`、`LineMetrics`、宽度缓存。
- **7.2 Inline**：grapheme cluster、Atom、HardBreak、自动换行、CJK 逐字断行、caret x/offset 映射。
- **7.3 Block**：Paragraph、Heading、CodeBlock、Html/Math/FrontMatter、Quote、List/ListItem、Table/Row/Cell、Image、ThematicBreak。
- **7.4 几何**：`caret_rect`、`position_at_point`、`rects_for_range`、`selection_rects`、`block_outer`、`closest_x`、`visible`、`total_height`。
- **7.5 增量**：`LayoutEngine` 叶级 line cache、invalidate、全量/增量逐字段相等、5000 次编辑等价测试、±1 屏可见范围。
- `md-layout` 19 个测试；全 Workspace 261 个测试通过。
- 已知增量：精确 CJK 禁则/悬挂空白、图片内在尺寸、表格列宽策略、滚动锚定与完整虚拟化。

## 2026-11-24 · 阶段 8：`md-render` 渲染核心

- **Paint 模型**：`Theme`、`Color`、`PaintLayer`、`PaintCommand`、`PaintPlan`；块背景、选区、Glyph、Image、Caret 分层排序。
- **Shaping**：`Shaper`、`FakeShaper`、`ShapedGlyph`/`ShapedRun`、`ShapingCache`。
- **输入适配**：Key → Intent、鼠标点击/拖动 → DocPos、ScrollState、CaretBlink。
- **资源**：ImageState、ResourceCache、version 与占位状态。
- **View**：`RenderView` 持有 layout/theme/scroll/blink/input，paint 不修改 Document。
- `md-render` 12 个测试；全 Workspace 271 个测试通过。
- **未接入**：真实 GPUI 窗口/元素/paint 适配；阶段 8 按“GPUI-ready 渲染核心”验收，平台适配与真实窗口验证留待阶段 9 装配。

## 2026-11-24 · 阶段 9：`md-app` 应用核心

- **9.1 应用基础**：`AppState`、`AppWindow`、`AppConfig`、最近文件、关闭决策。
- **9.2 文件 IO**：
  - 新建、打开 `.md`/`.markdown`、UTF-8 读取、解析。
  - 保存/另存为；保存成功后 `mark_saved`。
  - 原子写临时文件 + fsync + rename；Windows 替换回退。
  - 外部文件修改检测。
- **9.3 菜单与快捷键**：文件/编辑/格式/视图/主题/帮助菜单，命令与快捷键映射。
- **9.4 主题与偏好**：墨/纸/高对比主题、字号/行高、TOML 子集持久化。
- `md-app` 9 个单元测试 + 5 个应用流测试 + 2 个 smoke 测试；全 Workspace 283 个测试通过。
- **未接入**：真实 GPUI 窗口/Element/paint；阶段 9 按“应用核心”验收，桌面壳层装配待完成。

## 2026-11-24 · 阶段 10：产品功能核心

- **文档大纲**：Heading 扁平大纲、层级、DocPos。
- **搜索**：大小写敏感/不敏感、全部命中、上一个/下一个循环。
- **折叠**：FoldState、可折叠 Heading/Quote/List 集合。
- **状态栏**：字符/词/块数、阅读时长、DocPos。
- **导出**：Markdown（Serializer）与纯文本投影。
- **命令面板**：按标签/id/快捷键过滤命令。
- **图片**：相对文档路径解析。
- **滚动**：`scroll_to_caret`、打字机居中。
- `md-app` 13 unit + 5 flow + 2 smoke；全 Workspace 287 个测试通过。
- **未完成**：真实 GPUI 大纲/搜索高亮/折叠交互/命令面板/状态栏 UI；图片异步加载与表格 UI。

## 2026-11-24 · 阶段 11：正确性、规模门禁与 Release 配置

- **AC-03 Undo 完备性**：1000 组随机序列 × 24 Intents；全部 Undo 恢复初始内容/编辑前选区，全部 Redo 恢复最终内容。
- **AC-04 不变式**：1000 组随机 Document 操作，每步 `validate()`。
- **AC-02 模型闭合**：200 夹具 canonical 输出重新 parse 后 `Document::content_eq`。
- **AC-05 增量等价**：已有 5000 次编辑测试。
- **规模门禁**：100k 字符段落、10k 段落布局 `#[ignore]` 性能门禁。
- **Release profile**：strip symbols；为保证 GPUI 可交付构建，关闭 thin LTO 并使用 16 codegen units。
- Undo/Redo 改为 Document 快照权威，修复 JoinBlocks inverse 无法恢复 BlockId 的 redo 失效。
- 全 Workspace 291 个测试通过。
- **未完成**：真实 200 篇外部语料、全部 RuleId 覆盖、P95 性能测量、跨平台安装包/图标/签名、真实 GPUI 壳层。

## 2026-11-24 · GPUI 真实窗口基线

- `gpui` / `gpui_platform` 按 ADR-0001 revision 接入，仅 `md-app` 引用；`md-render` 及以下仍无 GPUI。
- `CotiView`：GPUI `Render`，按 `PaintPlan` 绝对定位绘制背景、文本、选区、Caret、图片占位。
- 键盘：字符、Backspace/Delete/Enter/Tab/Esc/方向/Home/End、Ctrl+Z/Ctrl+Shift+Z、Ctrl+S。
- 鼠标：点击经 `LayoutTree::position_at_point` 定位，`track_focus` 获取键盘焦点。
- 启动：`coti --gui [path]`；无参数保留 headless banner。
- Windows 实机 smoke：窗口初始化成功并保持运行；带路径启动成功。
- 全 Workspace 291 个测试通过；分层检查仍通过。
- **未完成**：菜单栏/文件对话框/滚动/IME/图片加载/自定义 Element/平台三端验证。

## 2026-11-24 · GPUI 自定义 Element 渲染

- `DocumentElement` 实现 GPUI `Element` 的 `request_layout` / `prepaint` / `paint`。
- Layout 高度由 `LayoutTree::total_height` 提供；`prepaint` 将 `PaintPlan` 转为 `PaintQuad` 与 `ShapedLine`。
- `paint` 直接调用 `Window::paint_quad` 与 `ShapedLine::paint`，不再为每个绘制项创建 `div`。
- 文本使用 GPUI `TextSystem::shape_line`，颜色/字号来自 `PaintCommand`。
- 保留外层 `div` 只负责焦点、键盘与鼠标命中；绘制全部由自定义 Element 完成。
- Windows 实机 smoke：带 Markdown 文件启动、首帧绘制成功、保持运行。
- 全 Workspace 291 个测试通过；分层检查通过。
- **未完成**：滚动事件、IME ElementInputHandler、图片真实绘制、局部重绘/裁剪优化。

## 2026-11-24 · 桌面壳层收尾与可执行文件

- 自定义 GPUI `Element`：`DocumentElement` 直接 `paint_quad` + `ShapedLine::paint`。
- 滚轮：`on_scroll` 更新 `ScrollState`，按总高度 clamp。
- 文件交互：Ctrl+O 打开、Ctrl+N 新建、Ctrl+S 保存、Ctrl+Shift+S 另存为；使用原生文件对话框。
- 窗口标题随文件路径更新。
- 新增 `scripts/build-release.ps1`，产物 `dist/coti.exe`。
- 新增 `docs/RELEASE.md`：构建、运行、快捷键、已知限制。
- 全 Workspace 291 个测试通过；Windows Release 构建与 GUI smoke 已验证。

## 2026-11-24 · 最终可执行文件

- 构建 `dist/coti.exe`（Windows x64，11.5 MB）。
- Release executable 实机 smoke：打开 Markdown、自定义 Element 首帧渲染、窗口保持运行。
- 全 Workspace 291 个测试通过；clippy 与分层检查通过。
- 构建脚本 `scripts/build-release.ps1`；运行说明 `docs/RELEASE.md`。

## 2026-11-24 · 布局与渲染修正（参考实现对齐）

- `md-layout` 支持注入真实 `TextMeasurer`；`layout_with_measurer` 供 GPUI 层使用。
- GPUI 渲染前用 `WindowTextSystem::layout_line` 重新布局，换行/caret 与真实字体一致；删除对 FakeMeasurer 的运行时依赖。
- `Line` 记录字体大小，Heading/CodeBlock 不再被统一按正文字号绘制。
- 页面增加 24px padding、760px 最大版心并居中。
- 自定义 Element 仍负责最终绘制；布局缓存按 document revision + viewport width 失效。
- 291 个测试通过；clippy/分层检查通过。

## 2026-11-24 · 主题与排版调整

- 主题语义扩展：heading/code_text/quote_text/table_header/accent。
- PaintPlan 新增引用条、代码块边框、表格表头/网格、列表项目符号/序号。
- 标题使用 600/500 字重；`MeasureStyle.weight` → `Line.font_weight` → GPUI `FontWeight`。
- 墨/纸/高对比三主题补齐语义色；Ctrl+Shift+T 循环主题。
- 修复代码块换行：按行构造成 HardBreak，避免 GPUI `layout_line` 收到换行符崩溃。
- 291 个测试通过；实际窗口截图验证标题/引用/列表/代码块主题。

## 2026-11-24 · 完全移除运行时 FakeMeasurer 路径

- `AppWindow` 初始使用空布局，不再先用 FakeMeasurer 布局。
- 首次 GPUI 渲染即注入 `SysMeasurer`，之后按 document revision/width 重排。
- `TextMeasurer` 增加 `cluster_advances`：整段 shaping 后按 grapheme 边界取 advance，避免逐字测量误差。
- CodeBlock 使用 mono font（Consolas）；`Line` 携带 `mono`/`font_weight`。
- 删除 `RenderView::sync` 与 `sync_incremental` 的 Fake 默认布局入口；生产 app 只走真实 measurer。
- `FakeMeasurer` 仅保留在 md-layout 单元测试/无窗口测试中（与参考工程一致）。
- 291 个测试通过；实际窗口截图验证换行、字重、代码字体、主题。

## 2026-11-24 · 跨段删除与拖拽选区修复

- Backspace 在段首自动 Join 前一段（Paragraph/Heading，同父）。
- DeleteForward 在段尾自动 Join 后一段；空段、末段删除不再失效。
- DeleteForward 在 separator 处区分 Upstream/Downstream，避免误合并空段。
- 鼠标拖动建立跨块 Range 选区（on_mouse_move/on_mouse_up）。
- 新增 2 个跨段删除测试；293 个测试通过。

## 2026-11-24 · 主题背景覆盖整个客户区

- 视图根节点增加主题背景色；DocumentElement 只负责内容区域。
- 空文档、内容不足一屏时，末行以下不再露出白色父容器背景。
- 截图验证：空文档整个客户区为墨色背景，光标可见。
- 原生窗口标题栏仍由系统绘制（未做自定义 titlebar）。

## 2026-11-24 · 自定义标题栏，主题覆盖整窗

- `WindowDecorations::Client` + 透明 titlebar，移除系统白色标题栏。
- 自绘 36px 标题栏：文件名、拖拽区、最小化/最大化/关闭按钮。
- 标题栏使用 `theme.surface`，与正文背景连续。
- 窗口内容 viewport 高度扣除标题栏，避免布局溢出。
- 空文档实测：标题栏与正文均为墨色，无白色区域。
- 293 个测试通过；clippy 通过。

## 2026-11-24 · 界面壳层：菜单栏/搜索/大纲/命令面板/状态栏/图片

- 标题栏整合：文件/编辑/格式/视图/主题/帮助菜单，搜索、大纲、命令、主题按钮，窗口控制。
- 搜索浮层：Ctrl+F，输入/Backspace/Enter 下一个/Esc；显示命中计数。
- 大纲侧栏：Ctrl+Shift+O，Heading 列表，点击跳转。
- 命令面板：Ctrl+K，过滤命令、上下选择、Enter 执行。
- 底部状态栏：保存状态、字符/块/分钟/DocPos、主题名。
- 图片：GPUI 渲染层加载本地图片，`paint_image` 绘制；相对文档路径。
- 鼠标坐标扣除标题栏高度，修复点击偏移。
- 293 个测试通过。
- **仍缺**：平台 IME 预编辑、原生菜单栏、安装包/图标/签名。

## 2026-11-24 · 标题栏菜单/搜索/大纲/命令面板/状态栏/图片

- 标题栏整合菜单、搜索、大纲、命令、主题按钮。
- 搜索浮层 Ctrl+F；命令面板 Ctrl+K；大纲 Ctrl+Shift+O。
- 状态栏显示保存状态、字符/块/分钟/DocPos、主题。
- 本地图片加载并 `paint_image` 绘制，相对文档路径解析。
- 鼠标坐标扣除标题栏高度。
- Release `dist/coti.exe` 15.9 MB；293 测试通过。
- 仍缺：平台 IME 预编辑、原生菜单栏、安装包/图标/签名。

## 2026-11-24 · IME、并排大纲与界面壳层

- 实现 GPUI `EntityInputHandler`：
  - UTF-16 ↔ DocPos 映射、text_for_range、selected/marked range。
  - replace_text_in_range 提交；replace_and_mark_text_in_range 预编辑；unmark/cancel。
  - bounds_for_range / selection_bounds_for_range / character_index_for_point。
  - 预编辑文本叠加绘制在 caret 处。
- DocumentElement paint 调用 `window.handle_input`；render 保持焦点。
- 大纲改为 flex 兄弟面板，不再绝对定位覆盖正文。
- 标题栏菜单/搜索/大纲/命令/主题、状态栏、本地图片绘制。
- 293 个测试通过。

## 2026-11-24 · 大纲左侧并排布局修复

- 大纲面板移到正文左侧，作为 flex 兄弟节点，不再覆盖正文。
- 正文 `w(0).flex_1().overflow_hidden()` 强制收缩；侧栏 `flex_none().w(240)` 固定宽度。
- 实测截图：侧栏 240px，正文整体右移，无重叠。
- IME `EntityInputHandler` 代码已接入（UTF-16 映射、marked/commit/cancel、bounds、预编辑绘制）。
- 293 个测试通过。

## 2026-11-24 · 修复搜索/全选/IME 光标/图标

- 搜索不再关闭大纲。
- 搜索输入只由 key handler 处理一次，移除 EntityInputHandler 的重复写入。
- Ctrl+A 全选整个文档。
- IME 预编辑：正文 caret 隐藏，改为在预编辑文本末尾绘制 caret；ime_text 包含 composition。
- 标题栏按钮改用 assets/icons SVG：search、panel-left-open/close、command、palette、minimize、maximize、x。
- 配置 GPUI AssetSource，资源目录 assets/。
- 293 个测试通过。

## 2026-11-24 · 修复搜索重复/全选/IME 光标/SVG 图标

- 搜索打开不再关闭大纲。
- 搜索文本只由 key handler 写入一次，避免 EntityInputHandler 重复。
- Ctrl+A 全选文档。
- IME 预编辑时正文 caret 隐藏，改为在预编辑末尾绘制 caret；ime_text 包含 composition。
- 标题栏按钮改用 assets/icons SVG：search、panel-left-open/close、command、palette、minimize、maximize、x。
- GPUI AssetSource 加载 assets/。
- 293 个测试通过；Release 16.9 MB。

## 2026-11-24 · 修复标题栏拖动、搜索、全选、IME 光标与 SVG 图标

- 标题栏拖动：Windows 后端 `start_window_move` 为 no-op，改用 Win32 `ReleaseCapture + WM_NCLBUTTONDOWN/HTCAPTION`；实物拖动验证窗口从 (719,517) 移到 (492,122)。
- 搜索不再关闭大纲。
- 搜索文本只写一次，移除 EntityInputHandler 重复写入。
- Ctrl+A 全选。
- IME 预编辑：包含 composition 文本，隐藏原 caret，在预编辑末尾绘制新 caret。
- 标题栏按钮使用 assets/icons SVG；配置 GPUI AssetSource。
- 293 个测试通过。

## 2026-11-24 · 搜索/命令输入 caret 与原生拖动

- 搜索框、命令框增加可见 caret 与点击聚焦（FocusHandle）。
- 搜索打开不再关闭大纲；搜索文本只写一次。
- Ctrl+A 全选。
- IME 预编辑末尾 caret；ime_text 包含 composition。
- Windows 原生拖动：ReleaseCapture + WM_NCLBUTTONDOWN(HTCAPTION)，实测窗口移动成功。
- 标题栏 SVG 图标与 AssetSource。
- 293 个测试通过。

## 2026-11-24 · 搜索/命令框 caret 聚焦与闪烁

- 打开搜索/命令面板时不自动激活输入框；点击输入框后才显示 caret。
- caret 位于文本最前（空输入时在占位符前），不再跑到占位符末尾。
- 占位符使用 faint 颜色，降低对比度。
- caret 500ms 闪烁；关闭面板或切换输入时停止。
- 293 个测试通过。

## 2026-11-24 · 独立输入框 IME、稳定 caret、编辑器点击激活

- 搜索/命令面板各自维护 IME composition 与 marked text；输入目标按 active 状态路由。
- 搜索/命令框支持中文 IME，候选窗口 bounds 定位到各自输入框。
- caret 始终占据 1.5px：隐藏时颜色透明，不再导致文本移动。
- 编辑器 caret 仅点击正文后显示；打开搜索/命令时隐藏编辑器 caret。
- caret 500ms 闪烁；输入框空时 caret 在占位符前。
- 293 个测试通过。

## 2026-11-24 · 修复搜索/命令点击被正文抢焦点

- 搜索/命令面板加 id + occlude，点击不再穿透到正文。
- 面板打开时正文 mouse_down 直接返回，避免重置 editor_active/search_active。
- 实测：点击搜索框后输入 abc，查询框显示 abc，正文保持 0 字、无正文 caret。
- caret 始终占位，闪烁只改颜色。
- 搜索/命令/编辑器各自 IME 目标。
- 293 个测试通过。

## 2026-11-24 · 光标空闲闪烁与命令面板外部点击关闭

- 主编辑器 caret 也闪烁。
- 所有 caret 在输入后保持常亮 1 秒，之后每 500ms 闪烁；任何输入都会重置计时。
- 搜索/命令/编辑器共用 idle blink 状态。
- 点击正文会关闭命令面板并激活编辑器。
- 点击菜单/大纲/主题时关闭命令面板。
- 293 个测试通过。

## 2026-11-24 · 修复闪烁任务重入 panic

- 闪烁改为构造时启动的单一常驻 100ms 计时器，事件处理中不再 spawn/drop Task。
- 点击/输入只更新 last_input、last_blink 时间戳。
- 输入后常亮 1 秒，之后每 500ms 闪烁；无激活目标时自动隐藏。
- 点击正文关闭命令面板并激活编辑器。
- 实测打开命令面板后点击正文，进程存活。
- 293 个测试通过。

## 2026-11-24 · 修复标题栏拖动 RefCell panic

- 根因：同步 `SendMessageW(WM_NCLBUTTONDOWN)` 在 GPUI 鼠标事件处理未返回时进入 Windows 模态移动循环，重入 App 借用。
- 改为 `PostMessageW` 异步投递，当前事件处理完成后再进入拖动。
- 实测拖动窗口从 (33,37) 到 (49,123)，进程存活。
- 293 个测试通过。

## 2026-11-24 · 列表项回车新建同级列表项

- 根因：Enter 把右侧内容作为 Paragraph 插入 ListItem 内部，序列化成缩进段落。
- 新增 `Operation::InsertListItem`，一次事务插入 ListItem + Paragraph。
- Enter 在列表项段落末尾时新建同级列表项，光标落到新项开头。
- 边界位置优先解析为前一个列表项末尾，支持列表中间项。
- 实测源码：`- a\n- b\n` 在 b 末尾回车输入 123 输出 `- 123`，不再缩进。
- 296 个测试通过。

## 2026-11-24 · 修复行末删字与空列表项删除

- 行末边界 Backspace 现在优先删除前一个 leaf 的最后一个 grapheme，不再因边界解析到下一块而空操作。
- 新增 `Operation::RemoveListItem`：
  - 多列表项：删除空项，光标落到前一项末尾/后一项开头。
  - 唯一列表项：删除整个列表，根级自动补空段落。
- 空列表项 Backspace 可删除；列表中间空项同样支持。
- 新增回归测试：边界删字、空列表项删除、列表回车序列化。
- 298 个测试通过。

## 2026-11-24 · 修复标题/列表末尾删字与边界亲和

- 鼠标点击按落点设置 Affinity：点击块尾得到 Upstream，点击块首得到 Downstream。
- 删除时使用 selection 的 affinity 解析位置：
  - 标题末尾 Backspace 删除最后一个字符，不再合并下个标题。
  - 列表末尾同样删除最后字符。
  - 块首 Backspace 仍合并前一块。
- 删除范围回退：跨越块边界时截断到起始块末尾，避免 “editing across blocks”。
- 新增真实教程文档回归测试，覆盖“教程/单元/代码块”末尾删除。
- 300 个测试通过。

## 2026-11-24 · 列表项开头 Backspace 合并/反列表

- 列表项开头 Backspace：
  - 非第一项：合并到前一个 ListItem 的最后段落，InlineContent normalize 后写回。
  - 第一项：反列表为 Paragraph；若列表还有其他项则保留列表。
  - 空项：继续走 RemoveListItem。
- 新增 MergeListItem、UnlistListItem 操作。
- 新增测试：第二项开头合并、第一项开头反列表。
- 302 个测试通过。

## 2026-11-24 · 空列表项回车退出列表

- Enter 在非空列表项末尾：仍创建同级新列表项。
- Enter 在当前列表项为空时：不再创建新项，删除空项并在列表后插入普通段落。
- 唯一空项：删除整个列表，原位置变为空段落。
- 新增 `ExitListItem` 操作与测试 `enter_on_empty_list_item_exits_list`。
- 303 个测试通过。

## 2026-11-24 · 修复连续删除在行尾合并下一行

- Backspace 删除后光标 affinity 从 Downstream 改为 Upstream。
- 非末尾行连续删除时，第二次仍在当前行删除，不会解析到下一块并合并。
- 新增 `repeated_backspace_at_line_end_does_not_join_next_line`。
- 304 个测试通过。

## 2026-11-24 · 修复空行 Backspace 直接跳到上一行

- 删除后根据块是否为空设置 affinity：
  - 仍非空：Upstream，连续删除留在当前行。
  - 已删空：Downstream，下一次 Backspace 命中空行本身并删除当前行。
- 新增 `backspace_on_emptied_line_removes_that_line`。
- 305 个测试通过。

## 2026-11-24 · 编辑规则全面审计：affinity 统一

- 点击：按落点设置 Upstream/Downstream（行尾/行首）。
- 左移：落到块首用 Downstream，跨到上一块末尾用 Upstream。
- 右移：落到当前块末尾用 Upstream，跨到下一块开头用 Downstream。
- DocStart=Downstream，DocEnd=Upstream。
- Enter、插入文本、缩进、标题输入规则均使用当前 selection 的 affinity 解析位置。
- Backspace 删除后：块非空保持 Upstream，删空改 Downstream。
- 列表边界合并/反列表/空项删除按 affinity 区分“行尾删字”和“块首合并”。
- 修复列表后普通段落开头 Backspace 误删上一列表项字符。
- 新增点击行尾 affinity、左右移动到列表边界、列表后段落边界、连续删除等测试。
- 311 个测试通过。

## 2026-11-24 · 修复行末点击偶发 Downstream 导致合并下一行

- 布局点击增加“纵向最近行”回退，块间空隙/行尾外侧不再命中失败而保留旧 selection。
- 点击 x 超过行 right 边界强制 Upstream。
- 左移/右移按落点区分 Upstream/Downstream。
- Backspace/Enter/InsertText/ChangeIndent 统一使用 selection affinity。
- 新增点击行尾外侧、左右移动列表边界、列表后段落边界测试。
- 312 个测试通过。

## 2026-11-24 · 用 caret 几何距离决定点击 affinity

- 同一 DocPos 同时计算 Upstream/Downstream 两个 caret 矩形。
- 点击后选择离鼠标更近的 caret，避免点在上一行行末却得到下一行开头。
- 布局新增 `caret_rect_with_affinity`，并验证边界两侧 caret 坐标不同。
- 保留纵向最近行回退与行尾外侧 Upstream。
- 313 个测试通过。

## 2026-11-24 · 飞书文档主题

- 新增默认 `Feishu` 主题：白底、深灰正文、蓝色光标/强调、浅灰代码块与表头、细边框、深灰列表标记。
- 主题新增 `quote_bar`、`code_border`、`list_marker` 语义色，引用/代码/列表不再共用 accent。
- Feishu 布局指标：15px/26px、内容宽 800、块间距 10、代码内边距 12、图片高度 180。
- 主题切换同步字体与行高；默认配置改为 Feishu。
- `Ctrl+Shift+T` 与标题栏主题按钮统一走 `cycle_theme`。
- 新增主题测试与 `docs/THEMES.md`。
- 314 个测试通过。

## 2026-11-24 · 表格/滚动/范围删除/列表样式修复

- 表格：补齐上下左右外边框，横向线按表格宽度绘制，边框闭合。
- 滚动：sync_view 保留 scroll.offset_y，点击不再回到顶部。
- 范围删除：支持跨列表项/跨容器，中间叶子按类型清空（inline 置空、raw 置空），不再报错。
- 列表：无序标记改为 `•` 文本，不再绘制方点；列表缩进 22。
- 新增测试：跨列表范围删除、sync_view 保留滚动。
- 316 个测试通过。

## 2026-11-24 · 修复表格单元格范围删除

- 同一块内范围删除不再通过 primary_local_range 重新用 Downstream 解析终点。
- 直接使用已按 Upstream 解析好的 start/end 构造 TextRange，避免表格单元格末尾解析到下一格导致失败。
- 新增 `delete_range_inside_table_cell` 测试。
- 317 个测试通过。

## 2026-11-24 · 表格竖线、末列重复线、窗口 resize 重排

- 表格竖线改为按列边界绘制，贯穿整行高度；最后一列不再绘制内侧竖线，由外框负责右边。
- 表头背景先绘制，外框与网格线后绘制，避免边框被表头覆盖。
- 窗口 resize 时按当前窗口尺寸重新计算内容 viewport（扣除标题栏/状态栏/大纲侧栏），并重新布局。
- 大纲打开时点击坐标扣除 240px 侧栏偏移。
- 初始 viewport 高度修正为窗口高度减标题栏与状态栏。
- 新增表格网格测试、resize viewport 测试。
- 321 个测试通过。

## 2026-11-24 · 缩小列表标记与文本间距

- 无序列表标记位置从 `item.left - 16` 改为 `item.left - 10`，宽度 12 → 10。
- 有序列表编号位置从 `item.left - 26` 改为 `item.left - 18`，宽度 24 → 18。
- 标记与文本间距约缩小一半。
- 321 个测试通过。

## 2026-11-24 · 滚动方向与列表标记再次收紧

- 滚轮 delta 取反，方向改为标准：向下滚动内容向上移动。
- 无序列表标记从 `item.left - 10` 收紧到 `item.left - 6`，宽度 6。
- 有序列表编号从 `item.left - 18` 收紧到 `item.left - 12`，宽度 12。
- 321 个测试通过。

## 2026-11-24 · 行内代码样式

- Cluster 携带 Marks；行内代码按 monospace 测量。
- 行内代码绘制浅灰背景 + 代码文字色，保持圆角前的基础背景。
- 链接使用 accent 颜色，粗体使用 600 字重并按该字重测量。
- 同一行按 marks 分组绘制，避免样式互相污染。
- 新增 inline code 样式测试。
- 322 个测试通过。

## 2026-11-24 · 行内代码圆角背景

- 行内代码背景改为 `RoundRect`：圆角 4px。
- 水平左右各 4px padding，垂直上下各 2px padding。
- 圆角矩形通过 GPUI `PaintQuad::corner_radii` 实际绘制。
- 322 个测试通过。

## 2026-11-24 · 行内代码背景与引用背景区分

- Theme 新增 `inline_code_background`。
- 飞书主题：引用背景 `#F7F8FA`，行内代码背景 `#EFF0F2`，明显区分。
- Ink 使用深色 inline code 背景，Contrast 使用 `#E8E8E8`。
- 322 个测试通过。

## 2026-11-24 · 接入绘制视口裁剪（虚拟渲染第一层）

- DocumentElement prepaint 只处理“当前视口 ± 一屏”内的绘制命令。
- 离屏 Text 不再调用 `shape_line`；离屏图片不加载；离屏背景/网格线不生成 PreparedItem。
- 视口范围按 `scroll.offset_y` 与内容 viewport 高度计算，上下各预取一屏。
- IME 预编辑文本也按视口裁剪。
- 布局仍保留完整文档几何，属于 paint 级虚拟渲染，不是分块窗口布局。
- 322 个测试通过。

## 2026-11-24 · 绘制计划级虚拟渲染

- 新增 `build_paint_plan_visible` / `RenderView::paint_visible`。
- 绘制计划本身只生成视口 ± 一屏内的 block 内容：背景、装饰、文本、图片、列表、表格行均提前裁剪。
- GPUI prepaint 使用可见计划，离屏内容不再进入 PaintPlan，也不再 shape 文本或解码图片。
- 保留原有 prepaint 命令级裁剪作为安全层。
- 布局仍保留完整文档高度（scroll/caret 需要），但绘制与 shaping 已虚拟化。
- 新增 visible plan 裁剪测试。
- 323 个测试通过。

## 2026-11-24 · 行内代码间距与列表跨项删除合并

- 行内代码 run 前后各增加 2px 布局间距；背景圆角仍保留 4px 内 padding。
- 同列表跨项范围删除改为合并：保留起始项前缀 + 结束项后缀，删除中间与结束列表项。
- 更新跨列表范围删除测试为 `- af` 合并结果。
- 323 个测试通过。

## 2026-11-24 · Quote 上下留白与空行显示

- Quote/Footnote 布局增加 vertical padding：默认 8px，飞书主题 10px。
- 解析时按相邻根块 source range 计算额外空行，写入 `StyleHints::blank_lines_after`（最多 8）。
- 布局按空行数增加 `line_height` 间距，打开文档时连续空行不再消失。
- 保持 canonical serializer 稳定，fixture 全部通过。
- 新增空行间距布局测试。
- 323 个测试通过。

## 2026-11-24 · Quote 留白、空行显示、空段落光标

- Quote/Footnote 上下 padding：默认 8px，飞书主题 10px。
- 解析根块间距，把额外空行写入 `blank_lines_after`，布局按行高补出空白间距。
- 空段落没有 lines 时，caret_rect 回退到块矩形起点，修复换行后光标跑到下一行首的问题。
- 新增空段落 caret 测试。
- 324 个测试通过。
