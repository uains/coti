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
