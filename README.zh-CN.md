# TDesign GPUI

<p align="center">
  使用 Rust 为 GPUI 提供原生 TDesign React PC 组件。
</p>

<p align="center">
  简体中文 · <a href="README.md">English</a>
</p>

<p align="center">
  <a href="https://github.com/wintopic/TDesign-GPUI/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/wintopic/TDesign-GPUI/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/wintopic/TDesign-GPUI/blob/main/LICENSE-MIT"><img alt="License" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg"></a>
  <a href="https://github.com/wintopic/TDesign-GPUI/blob/main/rust-toolchain.toml"><img alt="Rust 1.96" src="https://img.shields.io/badge/rust-1.96%2B-93450a.svg?logo=rust"></a>
  <a href="https://github.com/zed-industries/zed/tree/main/crates/gpui"><img alt="GPUI 0.2.2" src="https://img.shields.io/badge/GPUI-0.2.2-0052d9.svg"></a>
  <img alt="71 个组件" src="https://img.shields.io/badge/components-71-2ba471.svg">
  <img alt="2,354 个图标" src="https://img.shields.io/badge/icons-2%2C354-e37318.svg">
</p>

TDesign GPUI 是面向
[GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) 的
**非官方社区版** [TDesign React PC](https://tdesign.tencent.com/react/getting-started)
组件库。项目使用 GPUI 原生元素和符合 Rust 习惯的 builder，提供强类型状态与事件、
运行时主题和语言配置、虚拟化数据组件、原生文件上传，以及完整 TDesign 图标集。

> 本项目与腾讯、TDesign、Zed 及其维护团队没有隶属或官方背书关系。TDesign 和
> GPUI 的版权归各自权利人所有，详见[第三方声明](THIRD_PARTY_NOTICES.md)。

## 当前状态

项目目前处于 **0.1.x** 阶段。仓库已覆盖 TDesign React PC 官网 71 个组件表面，并
提供原生 `ConfigProvider` 适配；机器可读 parity 记录中没有未映射的上游字段。
但 `0.x` 仍可能调整 API，固定参考版本的完整 macOS 视觉验收和所有交互组件的
AccessKit 全量验收仍是进入 `1.0` 前的工作。

Parity 矩阵证明锁定的上游 API 快照已被完整分类，但它本身不能证明每个 Rust 方法
都已达到视觉与行为等价。`0.1.x` 仍在推进全组件 token 消费、带锚点的 deferred
浮层和 outside-click 协调、模态焦点陷阱、`InputNumber` 文本编辑、`RangeInput`
变更事件、完整 AccessKit、视觉基准，以及 71 个组件的真实 Story 覆盖。主题与语言
目前通过 GPUI 应用全局状态解析，因此需要多个窗口同时使用不同主题的场景，在根级
上下文完成前仍应视为实验能力。

当前范围包括：

- 71 个 TDesign React PC 组件表面和原生 `ConfigProvider` 映射
- Light、Dark、System 以及 token 覆盖主题
- `zh-CN`、`en-US` 和应用自定义文案
- 全部 2,354 个 TDesign SVG 图标，默认启用 `full-icons`
- Windows、macOS、Linux 原生 GPUI 桌面应用
- 每日检查 TDesign 上游与 Zed/GPUI `main` 的兼容性

本 workspace 不包含 TDesign Mobile、AI Chat/Pro、小程序、UniApp 和 WASM。

## 安装

首次源码发布可以直接使用 GitHub 依赖：

```toml
[dependencies]
gpui = "0.2.2"
tdesign-gpui = { git = "https://github.com/wintopic/TDesign-GPUI", branch = "main" }
```

用于正式构建时，应在发布标签可用后把 `branch` 换成固定 `tag` 或精确 `rev`。
发布到 crates.io 后可改为：

```toml
[dependencies]
gpui = "0.2.2"
tdesign-gpui = "0.1"
```

最低支持 Rust 1.96，并会随 GPUI 的稳定工具链要求显式升级。构建前还需安装 GPUI
对应平台的系统依赖，macOS 需要 Xcode 与命令行工具，Linux 则取决于使用的
Wayland/X11 后端。最新要求请查看
[GPUI README](https://github.com/zed-industries/zed/tree/main/crates/gpui)。

## 快速开始

下面是可直接运行的原生窗口，包含 `TDesignRoot`、`Button`、`Input` 和内嵌图标。
仓库中的 [`examples/basic.rs`](crates/tdesign-gpui/examples/basic.rs) 提供了持续编译的对应示例。

```rust
use gpui::{App, Application, Context, Render, Window, WindowOptions, div, prelude::*};
use tdesign_gpui::{
    Button, ButtonVariant, Icon, IconName, Input, InputState, TDesignAssetSource, TDesignRoot,
};

struct BasicExample {
    input: gpui::Entity<InputState>,
}

impl BasicExample {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            input: InputState::new(cx, ""),
        }
    }
}

impl Render for BasicExample {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        TDesignRoot::new().child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_6()
                .child("TDesign GPUI")
                .child(Input::new(self.input.clone()))
                .child(
                    Button::new("确定")
                        .variant(ButtonVariant::Primary)
                        .icon(Icon::new(IconName::Check)),
                ),
        )
    }
}

fn main() {
    Application::new()
        .with_assets(TDesignAssetSource::new())
        .run(|cx: &mut App| {
            tdesign_gpui::init(cx);
            cx.open_window(WindowOptions::default(), |_window, cx| {
                cx.new(BasicExample::new)
            })
            .expect("open TDesign GPUI example window");
            cx.activate(true);
        });
}
```

在仓库中运行：

```sh
cargo run -p tdesign-gpui --example basic
```

如果应用已有自己的 GPUI 资产源，可以使用
`TDesignAssetSource::with_fallback(existing_source)` 或
`with_shared_fallback(...)` 保留原资产。

## 根配置、主题与语言

`TDesignRoot` 对应 React 的 `ConfigProvider`，负责解析窗口主题和语言全局值、承载
应用内容，也可以接管 Popup、Dialog、Drawer、Message、Notification 等窗口级弹层。

```rust,no_run
use tdesign_gpui::{Locale, TDesignConfig, TDesignRoot, ThemeMode, ThemeOverrides};

fn configured_root() -> TDesignRoot {
    let config = TDesignConfig::new()
        .theme_mode(ThemeMode::System)
        .theme_overrides(ThemeOverrides::new().brand(gpui::rgb(0x7c3aed)))
        .locale(Locale::en_us())
        .message("confirm", "保存");
    TDesignRoot::with_config(config)
}
```

需要运行时切换时，在外层 View 中持有 `TDesignConfigState` 实体，通过
`.config_state(...)` 传给根节点，并在 GPUI 回调中更新：

```rust,no_run
use tdesign_gpui::{Locale, TDesignConfigState};

fn switch_to_chinese(state: &gpui::Entity<TDesignConfigState>, cx: &mut gpui::App) {
    state.update(cx, |state, cx| state.set_locale(Locale::zh_cn(), cx));
}
```

`TDesignRoot` 会在渲染时读取配置实体，GPUI 因而会自动追踪这项依赖；调用
`set_theme`、`set_locale` 或 `update` 后，配置实体会通知并在下一帧重绘受影响的 View。

## 状态与事件

交互组件使用 `Entity<XState>`，不依赖隐藏的 DOM 状态。Input、Textarea、
AutoComplete、TagInput、RangeInput 和 SelectInput 共享 `InputState` 的文本编辑模型，
包括键盘、选区、剪贴板、原生 IME、受控更新，以及强类型 `InputEvent::Change`。

```rust,no_run
use gpui::Context;
use tdesign_gpui::{InputEvent, InputState};

struct FormView {
    input: gpui::Entity<InputState>,
    value: String,
}

impl FormView {
    fn new(cx: &mut Context<Self>) -> Self {
        let input = InputState::new(cx, "");
        cx.subscribe(&input, |this, _input, event, cx| {
            let InputEvent::Change(change) = event;
            this.value = change.current.to_string();
            cx.notify();
        })
        .detach();
        Self {
            input,
            value: String::new(),
        }
    }
}
```

选择器、日期时间、表单、树和上传组件遵循同样的实体/事件模式。需要访问应用状态的
交互回调会收到强类型值以及 GPUI 的 `Window`、`App` 上下文。

## Upload

`Upload` 使用 GPUI 原生文件选择器。设置 `action` 即可启用内置 multipart HTTP
传输，并获得进度、协作取消、重试和 `UploadEvent` 生命周期通知：

```rust,no_run
use tdesign_gpui::{Upload, UploadState};

fn upload_control(cx: &mut gpui::App) -> Upload {
    Upload::new(UploadState::new(cx))
        .action("https://example.test/upload")
        .multiple(true)
}
```

需要自定义鉴权或存储时，实现 `UploadBackend`，再通过 `.backend(...)` 注入
`Arc<dyn UploadBackend>`。如果需要进度和取消，覆盖 `upload_with_request` 并使用
`UploadRequest`、`UploadCancellation`；简单后端只实现 `upload` 即可。

```rust,no_run
use gpui::{App, SharedString, Task};
use std::sync::Arc;
use tdesign_gpui::{Upload, UploadBackend, UploadState};

struct MyBackend;

impl UploadBackend for MyBackend {
    fn upload(&self, path: SharedString, _cx: &mut App) -> Task<anyhow::Result<SharedString>> {
        Task::ready(Ok(format!("stored://{path}").into()))
    }
}

fn custom_upload(cx: &mut App) -> Upload {
    Upload::new(UploadState::new(cx)).backend(Arc::new(MyBackend))
}
```

这个自定义后端示例需要在应用清单中添加 `anyhow = "1"`。

## 虚拟化 List、Table 与 Tree

大数据组件不会在每帧创建整个数据集。`List` 和 `Table` 通过 GPUI
`uniform_list` 仅向 delegate 请求可视行：

```rust,no_run
use gpui::{AnyElement, App, Window, div, prelude::*};
use std::sync::Arc;
use tdesign_gpui::{List, ListDelegate};

struct Rows(Vec<String>);

impl ListDelegate for Rows {
    fn row_count(&self) -> usize {
        self.0.len()
    }

    fn render_row(&self, index: usize, _window: &mut Window, _cx: &mut App) -> AnyElement {
        div().h_8().px_3().child(self.0[index].clone()).into_any_element()
    }
}

fn list() -> List {
    List::new("records", Arc::new(Rows((0..100_000).map(|i| i.to_string()).collect())))
}
```

`TableDelegate` 在 `ListDelegate` 基础上补充列信息。`Tree` 只展开当前可见节点并对其
进行虚拟化，同时保留强类型选择状态和键盘遍历。

参与编译检查的 [`examples/patterns.rs`](crates/tdesign-gpui/examples/patterns.rs)
在一个原生应用中组合了运行时配置、Input 事件、自定义上传后端、List、Table 和 Tree。

## Feature flags

| Feature | 默认 | 作用 |
| --- | --- | --- |
| `full-icons` | 是 | 内嵌全部 2,354 个 SVG 图标；关闭默认 feature 时只内嵌文档列出的核心图标子集。 |
| `serde` | 否 | 为支持的公共配置、主题、语言和图标类型增加 Serde。 |

最小图标构建：

```toml
tdesign-gpui = { version = "0.1", default-features = false }
```

## 组件覆盖

| 分类 | 组件 |
| --- | --- |
| 基础与布局 | Button、Icon、Link、Typography、Divider、Grid、Layout、Space |
| 导航与反馈 | Affix、Anchor、BackTop、Breadcrumb、Dropdown、Menu、Pagination、Steps、StickyTool、Tabs、Alert、Dialog、Drawer、Guide、Message、Notification、Popconfirm、Popup |
| 输入与表单 | AutoComplete、Cascader、Checkbox、ColorPicker、DatePicker、Form、Input、InputAdornment、InputNumber、TagInput、Radio、RangeInput、Select、SelectInput、Slider、Switch、Textarea、Transfer、TimePicker、TreeSelect、Upload |
| 数据展示 | Avatar、Badge、Calendar、Card、Collapse、Comment、Descriptions、Empty、Image、ImageViewer、List、Loading、Progress、QRCode、Skeleton、Statistic、Swiper、Table、Tag、Timeline、Tooltip、Tree、Watermark、Rate |

[`parity/component-parity.json`](parity/component-parity.json) 是 API 对齐的事实来源。
每个 React PC prop、event 和默认值都必须映射为 Rust 方法、类型、状态、事件、原生
适配或明确的“不适用”。存在空记录、不完整记录或未映射字段时，
`cargo xtask parity check` 会失败并阻止发布。

## 上游同步

[`upstream.lock.toml`](upstream.lock.toml) 固定已集成的 `tdesign-api`、
`tdesign-common`、`tdesign-react`、`tdesign-icons` 和 Zed 提交。

```sh
cargo xtask upstream check
cargo xtask upstream sync --apply
cargo xtask parity generate
cargo xtask parity check
```

每日工作流会生成 API、主题、图标、行为、视觉和 GPUI 兼容性报告。只有新增 token/
icon 等可安全生成的变化才可能标记为 `sync-safe`；删除、重命名、组件行为/API 变化
和 GPUI 破坏性更新始终创建草稿 PR 交由维护者审查。GPUI Canary 通过临时依赖覆盖
验证 Zed `main`，不会改写稳定版清单。

## 开发

```sh
cargo fmt --all -- --check
cargo check -p tdesign-gpui --all-targets --no-default-features --locked
cargo check -p tdesign-gpui-assets --all-targets --no-default-features --locked
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked
cargo test --workspace --locked
cargo doc --workspace --no-deps --locked
cargo run -p tdesign-gpui-story
cargo run -p xtask --locked -- parity check
cargo run -p xtask --locked -- release check
```

进行较大修改前请阅读[架构说明](docs/architecture.md)、
[从 TDesign React 迁移](docs/migration-from-react.md) 和
[贡献指南](CONTRIBUTING.md)。

## 1.0 路线图

- 持续保持 React PC API、事件和默认值的 100% 机器可读映射
- 完成固定参考版本在原生 macOS 渲染器上的 Light/Dark 视觉基准
- 达到项目定义的 SSIM 和一个逻辑像素布局边缘验收线
- 完成所有交互组件的 AccessKit role/name/value/state 审核
- 完成双语组件文档和画廊状态覆盖
- 稳定通过 Windows、macOS、Linux 的构建、测试、文档和画廊 smoke check

发布记录见 [CHANGELOG.md](CHANGELOG.md)。安全问题请按 [SECURITY.md](SECURITY.md)
私下报告；使用问题请查看 [SUPPORT.md](SUPPORT.md) 中的公开支持渠道。

## 许可证

项目原创代码由用户自行选择 [MIT](LICENSE-MIT) 或
[Apache-2.0](LICENSE-APACHE) 许可证。生成或再分发的 TDesign 资产继续保留其上游
MIT 版权声明，详见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
