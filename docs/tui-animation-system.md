# `packages/tui` 动画系统设计

## 文档定位

本文档描述 `packages/tui` 的完整动画系统，而不是仅描述当前已经落地的组件级轻量动画。

目标是让框架具备统一的动画模型，覆盖：

- 组件属性动画，例如 progress value、switch checked、button focus。
- 持续动画，例如 loading spinner、activity indicator。
- 布局动画，例如高度、宽度、位置、列表重排、面板展开/收起。
- 进入/退出动画，例如 dialog、toast、collapsible content、list item insert/remove。
- 样式动画，例如前景色、背景色、边框强调、文本 attribute 过渡。
- 动画编排，例如 delay、sequence、parallel、stagger、shared transition。
- 全局策略，例如 reduced motion、测试模式、禁用动画、默认动画主题。

动画系统应是框架能力，而不是每个业务应用重复维护的 `on_frame` 状态机。

## 当前实现状态

截至本次实现，`packages/tui` 已完成动画系统的核心运行时和公开 API 骨架：

- [x] `AnimationSpec`：支持 `duration`、`delay`、`easing`、`repeat`、`direction`。
- [x] `Easing`：支持 `Linear`、`EaseIn`、`EaseOut`、`EaseInOut`、`CubicBezier`、`Steps`。
- [x] 全局 `MotionPolicy`：支持禁用动画、reduced motion、速度缩放、deterministic frame step。
- [x] `AnimationTheme` / `AnimationSlot`：支持主题级动画默认值。
- [x] `AnimationStore` value track：按 `WidgetPath + channel` 存储数值过渡。
- [x] `AnimationStore` pulse track：支持 loading spinner 这类持续视觉计数器。
- [x] `AnimationStore` style track：支持 `Style` 颜色和文本 attribute 过渡。
- [x] `AnimationStore` layout track：支持 `AreaF`、`LayoutSnapshot`、target/displayed area 过渡。
- [x] runtime scheduler：`AppRuntime` 会在存在 active animation 时继续请求下一帧。
- [x] widget hook：`Widget::animate(&mut AnimationCtx)` 已接入 widget tree 遍历。
- [x] render context：`RenderCtx` 可读取 `animation_value`、`animation_style`、`layout_animation`，并可通过 `track_layout` 在 render 阶段登记布局动画 target。
- [x] 组件级动画：`ProgressBar`、`LoadingIndicator`、`Switch`、`Button` 已使用统一 store。
- [x] 通用 wrapper：新增 `animate(child)` / `Animated`，支持 `.layout(...)`、`.layout_transition(...)`、`.enter(...)`、`.exit(...)`、`.presence(...)`。
- [x] Transition / Timeline / Presence API：已提供 `Transition`、`TransitionEffect`、`Timeline`、`Presence`、`InitialAnimation` 类型并导出到 prelude。
- [x] timeline executor：`AnimationStore` 已支持 timeline track，`Sequence`、`Parallel`、`Stagger` 会统一展开、调度、推进并纳入 active animation 判断。
- [x] wrapper enter runtime：`Animated` 已通过 `RenderCtx::track_timeline` 接入 enter timeline，并支持 expand/collapse、center scale 这类 TUI cell-area 过渡。
- [x] 状态清理：当前 live widget tree 消失的路径会从 animation store 中清理。
- [x] 测试覆盖：已覆盖 value retarget、delay、disabled motion、pulse 清理、layout retarget、style interpolation、timeline sequence/stagger 等核心行为。

仍然预留、尚未完整运行时化的部分：

- [ ] 完整 exit presence runtime：widget 从 view tree 消失后，旧 visual node 的保留、渲染和最终 prune 还未落地。
- [ ] 全局 layout diff pipeline：当前 layout transition 通过 wrapper/render 阶段 target area 驱动，尚未实现文档中完整的 layout 前后 tree snapshot diff。
- [ ] shared transition：跨 widget path 的 shared element/area transition 仍为后续扩展。

## 设计目标

1. 语义 state 和视觉 state 分离。

   业务 state 表示真实状态，例如 `checked: bool`、`progress: f64`、`expanded: bool`。动画系统负责从旧视觉状态过渡到新视觉状态。

2. 组件身份稳定。

   动画状态按 `WidgetPath + channel` 存储。开发者可以通过 `.key("stable-id")` 让动画在列表重排、条件渲染中保持连续。

3. 默认不改变现有视觉行为。

   组件动画必须显式开启，或者由明确的动画 wrapper/动画策略开启。

4. 覆盖 render、layout、lifecycle 三类动画。

   只做 render 数值动画是不完整的。完整系统必须能参与 layout 测量和 widget 生命周期。

5. 动画状态随 widget tree 自动清理。

   组件消失后，动画系统需要判断是立即清理，还是进入 exit animation 的保留阶段。

6. 可测试、可禁用、可预测。

   动画系统应支持 deterministic clock、禁用动画、跳到结束状态、固定 frame step。

7. 保持 TUI 约束。

   终端动画不应依赖像素级运动。布局动画应以 cell、line、area 为基本单位，样式动画应尊重终端颜色和 attribute 能力。

## 与 tick / frame 的边界

`tick`、`on_frame`、`animation` 都与时间有关，但职责不同。

### `App::on_tick`

应用层周期事件。

适合：

- 后台轮询。
- 倒计时。
- 定时刷新任务状态。
- 定时切换业务数据。

不适合：

- 每个 progress bar 自己维护 displayed value。
- 每个 loading indicator 自己维护 frame index。

### `App::on_frame`

应用层逐帧更新。

适合：

- 游戏。
- canvas 场景。
- 粒子、波形、火焰、starfield。
- 用户明确想控制整套逐帧模拟。

不适合：

- 普通组件根据 prop 变化做视觉过渡。
- dialog 进入/退出、accordion 展开/收起这类通用 UI motion。

### `animation`

框架层视觉状态系统。

适合：

- prop target 改变后的过渡。
- 组件生命周期动画。
- 布局重排过渡。
- focus/hover/selection 等 UI state 动画。
- loading 等纯视觉持续动画。

常见组合是：

```rust
App::new(state)
    .on_tick(Duration::from_secs(1), || Msg::Refresh)
    .run(update, view)
```

`tick` 改变业务 target，动画系统把旧视觉状态平滑过渡到新 target。

## 动画系统分层

完整动画系统分为五层。

### 1. Clock / Scheduler

统一时间源和帧调度。

职责：

- 提供 `now`、`delta`、`frame`。
- 判断是否还有 active animation。
- 决定 render event loop 是否继续请求下一帧。
- 支持测试中的 deterministic clock。
- 支持全局暂停、禁用、跳到结束状态。

### 2. AnimationStore

持久化动画状态。

key 结构：

```text
WidgetPath + AnimationChannel
```

channel 示例：

```text
"value"
"checked"
"focus"
"spinner"
"layout.x"
"layout.y"
"layout.width"
"layout.height"
"enter.opacity"
"exit.height"
```

Store 需要管理多类轨道：

- value track：数值 target 过渡。
- style track：颜色、attribute、border emphasis。
- pulse track：持续计数器。
- layout track：从 previous area 到 current area。
- lifecycle track：enter / exit / presence。
- timeline track：sequence / parallel / stagger。

### 3. Widget Hooks

widget 生命周期应包含动画相关 hook：

```rust
trait Widget<M> {
    fn animate(&self, ctx: &mut AnimationCtx) -> bool { false }
    fn render(&self, chunk: &mut Chunk, ctx: &RenderCtx);
    fn constraints(&self) -> Constraints;
}
```

完整系统还需要更细的 hook：

```rust
fn animation_style(&self) -> AnimationStyle
fn layout_transition(&self) -> Option<LayoutTransition>
fn presence(&self) -> PresenceMode
```

这些 hook 不一定都放进 `Widget` trait；也可以由 wrapper widget 或 builder API 注入。

### 4. Layout Integration

布局动画是完整系统的关键。

当前 layout 流程是：

```text
view(state) -> widget tree -> constraints -> layout -> render
```

布局动画需要在 layout 前后保留 area 快照：

```text
previous layout area
current target layout area
animated display area
```

这允许：

- panel 展开/收起时高度动画。
- list item 插入删除时其它 item 平滑移动。
- tab content 切换时内容区域过渡。
- overlay/dialog 从小 area 过渡到最终 area。

TUI 中布局动画的单位是 cell area：

```rust
pub struct AnimatedArea {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
```

最终 render 时再 round / floor 到 `u16` cell 坐标。

### 5. Presence / Lifecycle

进入/退出动画需要 presence layer。

普通 widget tree rebuild 会让不存在的 widget 立即消失。但 exit animation 需要让旧 widget 在视觉层短暂保留。

完整系统应区分：

- mounted：当前 view tree 中存在。
- exiting：当前 view tree 已不存在，但 exit animation 尚未结束。
- unmounted：exit animation 结束，可以清理状态。

这要求 runtime 不只 prune inactive paths，还要允许特定 path 进入 exiting set。

## 公开 API 设计

### 基础配置

```rust
pub struct AnimationSpec {
    pub duration: Duration,
    pub delay: Duration,
    pub easing: Easing,
    pub repeat: Repeat,
    pub direction: Direction,
}

pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(f64, f64, f64, f64),
    Steps(u16),
}

pub enum Repeat {
    Never,
    Count(u16),
    Forever,
}

pub enum Direction {
    Normal,
    Reverse,
    Alternate,
}
```

当前实现只需要 `duration + easing`，但完整系统应预留 delay、repeat、direction。

### 组件级属性动画

```rust
progress_bar(value)
    .animation(AnimationSpec::default())

switch("Deploy")
    .checked(done)
    .animated()
```

适合组件内部知道如何解释 target 的场景。

### 通用 animate wrapper

完整系统应提供 wrapper，用于不改组件源码也能添加动画。

```rust
animate(child)
    .layout(AnimationSpec::fast())
    .enter(Enter::fade_in())
    .exit(Exit::collapse())
```

或者：

```rust
animated_panel()
    .layout_transition(LayoutTransition::size_and_position())
    .child(content)
```

wrapper 负责声明：

- layout transition。
- enter/exit。
- style transition。
- presence mode。

### Layout Transition API

```rust
pub struct LayoutTransition {
    pub position: Option<AnimationSpec>,
    pub size: Option<AnimationSpec>,
    pub clip: ClipMode,
}

pub enum ClipMode {
    None,
    ClipToAnimatedBounds,
    ClipToTargetBounds,
}
```

使用示例：

```rust
collapsible("Advanced")
    .expanded(open)
    .layout_animation(LayoutTransition::height())
```

```rust
list(items)
    .keyed()
    .layout_animation(LayoutTransition::position())
```

### Presence API

```rust
pub struct Presence {
    pub enter: Option<Transition>,
    pub exit: Option<Transition>,
    pub initial: InitialAnimation,
}

pub enum InitialAnimation {
    Play,
    Skip,
}
```

使用示例：

```rust
presence(show_dialog, || {
    dialog()
        .title("Confirm")
        .child(...)
})
.enter(Transition::scale_from_center())
.exit(Transition::fade_out())
```

TUI 中的 scale 不应理解为像素缩放，而应理解为 area 从中心扩展、裁剪区域变化、边框逐步显现。

### Timeline / 编排 API

```rust
pub enum Timeline {
    Single(Transition),
    Sequence(Vec<Timeline>),
    Parallel(Vec<Timeline>),
    Stagger {
        delay: Duration,
        children: Vec<Timeline>,
    },
}
```

使用示例：

```rust
animate(menu)
    .enter(Timeline::Stagger {
        delay: Duration::from_millis(24),
        children: item_transitions,
    })
```

适合 command palette、menu、toast stack、list insert 等场景。

## Runtime Pipeline

完整 runtime pipeline 应是：

```text
1. drain events/tasks/ticks/frames
2. run app update
3. build new widget tree
4. analyze focus and live paths
5. diff previous tree snapshot with new tree snapshot
6. collect animation declarations
7. resolve presence: entering / mounted / exiting
8. compute target layout
9. update animation tracks
10. compute animated layout / render values
11. render mounted + exiting visual nodes
12. prune completed animation state
```

当前实现只有：

```text
build tree -> animate widget channels -> render animated values
```

完整系统需要在 layout 前后都有动画参与点。

## Layout 动画设计

### Area 快照

每个 widget path 保存：

```rust
pub struct LayoutSnapshot {
    pub path: WidgetPath,
    pub target: Area,
    pub previous: Option<Area>,
    pub displayed: AreaF,
}
```

`AreaF` 使用浮点数保存中间态，最终渲染时转换成 `Area`。

```rust
pub struct AreaF {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
```

### FLIP 思路

布局重排动画可以使用 FLIP 思路：

```text
First: 记录旧 area
Last: 计算新 target area
Invert: displayed 从旧 area 开始
Play: displayed 过渡到新 target area
```

在 TUI 中，这不是 transform，而是实际传给 render 的 animated area。

### 高度动画

折叠面板需要两种高度：

- target height：layout 计算出来的最终高度。
- displayed height：动画过程中的高度。

渲染时需要 clip 内容：

```text
displayed height = 0..target height
clip child render to displayed bounds
```

这要求 layout/render 层支持 clipping。没有 clipping 时，内容会溢出到后续区域。

### 列表重排动画

列表动画依赖 stable key。

```rust
row.key(item.id)
```

没有 stable key 时，框架只能按 index 判断身份，插入/删除会造成错误复用。

列表动画需要处理：

- insert：新 item enter。
- remove：旧 item exit。
- move：已有 item position transition。
- resize：item height transition。

## Enter / Exit 设计

### Enter

enter animation 在 widget 首次出现时播放。

常见 TUI enter：

- height 从 0 到 target height。
- border 从轻到重。
- 内容从 clipped 到完整。
- 颜色从 muted 到 normal。
- overlay 从 center small area 到 target area。

### Exit

exit animation 在 widget 从 view tree 中消失后播放。

runtime 必须保留旧 visual node：

```text
new tree 不含 path
presence policy 允许 exit
runtime 将 path 放入 exiting set
exit animation 完成后 prune
```

exit 期间不能再参与事件处理和 focus 导航，除非组件明确声明 modal exit 仍阻塞输入。

### Focus 与 Exit

如果 focused widget 进入 exiting：

- 默认立即把 focus 移到下一个 live focus target。
- 如果是 modal/dialog exit，可以允许 focus 暂时清空或回到 opener。

这需要 focus manager 与 presence manager 协作。

## Style 动画

终端样式动画不能假设 RGB 插值总是可用。

应支持三层能力：

1. Boolean attribute transition。

   例如 bold、underlined、reverse，在进度超过阈值时切换。

2. Semantic style transition。

   在 `normal`、`focused`、`selected`、`disabled` 等 style token 之间选择中间态。

3. Color interpolation。

   如果 `Color` 支持 RGB，则可插值；如果是 indexed/named color，则使用离散 step 或最近色。

API 示例：

```rust
style_transition("focus", normal, focused, spec)
```

组件不应直接假设所有终端都能展示细腻颜色动画。

## Clipping 和 Overdraw

布局动画、enter/exit 都需要 clipping。

需要在 render 层提供：

```rust
chunk.with_clip(area, |chunk| {
    child.render(chunk, ctx);
});
```

或者让 `Chunk::from_area` 支持 clip mode。

没有 clipping 时：

- height 动画会让子内容溢出。
- dialog scale/expand 会绘制到目标外。
- list exit collapse 会覆盖后续 item。

完整动画系统应把 clipping 当作基础设施，而不是某个组件的私有 hack。

## Input / Focus / Hit Testing

动画影响视觉区域，但事件命中和 focus 需要稳定规则。

建议规则：

- mounted widget：事件命中使用 target area，或使用 displayed area，由组件配置决定。
- entering widget：默认可 focus，但可以配置 `focus_on_enter_end`。
- exiting widget：默认不参与 focus，不响应事件。
- layout moving widget：mouse hit testing 可使用 displayed area，keyboard focus 仍使用 logical path。

这些规则应写进 `PresenceMode` 和 `HitTestMode`：

```rust
pub enum HitTestMode {
    TargetArea,
    DisplayedArea,
}

pub enum PresenceInteractivity {
    InteractiveDuringEnter,
    InteractiveAfterEnter,
    DisabledDuringExit,
}
```

## 全局 Motion Policy

完整系统需要全局动画策略：

```rust
pub struct MotionPolicy {
    pub enabled: bool,
    pub reduced_motion: bool,
    pub speed: f64,
    pub deterministic: bool,
}
```

来源可以包括：

- app builder 配置。
- 环境变量。
- 测试配置。
- 用户偏好。

策略行为：

- `enabled = false`：所有动画直接跳到最终状态。
- `reduced_motion = true`：禁用布局移动和大范围 enter/exit，只保留必要状态变化。
- `speed`：开发调试时加速或减速。
- `deterministic`：测试中使用固定 clock。

## Animation Theme

动画参数不应散落在每个组件里。

应支持 theme 级默认：

```rust
pub struct AnimationTheme {
    pub fast: AnimationSpec,
    pub normal: AnimationSpec,
    pub slow: AnimationSpec,
    pub focus: AnimationSpec,
    pub layout: AnimationSpec,
    pub enter: AnimationSpec,
    pub exit: AnimationSpec,
}
```

组件默认动画从 theme 读取：

```rust
button("Save").animated()
```

等价于使用 `theme.animations.focus`。

## 当前实现与目标系统的关系

当前已经落地的是完整系统的第一层能力，以及 Phase 2 的全局动画策略基础。

已完成：

- `AnimationSpec`
  - 已扩展 `duration`、`delay`、`easing`、`repeat`、`direction`。
- `Easing`
  - 已支持 `Linear`、`EaseIn`、`EaseOut`、`EaseInOut`、`CubicBezier`、`Steps`。
- `Repeat`
- `Direction`
- `AnimationStore`
- `AnimationCtx`
  - 保留 `AnimationCtx::new(...)` 兼容入口。
  - 新增 runtime 使用的 `AnimationCtx::with_policy(...)`，注入 `MotionPolicy` 和 `AnimationTheme`。
- `Widget::animate`
- `RenderCtx::animation_value`
- `ProgressBar` value animation
- `Switch` checked animation
- `LoadingIndicator` pulse animation
- `Button` focus animation
- `AnimationTheme`
  - 已接入 `Theme::animations` 和 `ThemeBuilder::animations(...)`。
  - `button(...).animated()` 使用 `theme.animations.focus`。
  - `progress_bar(...).animated()` 和 `switch(...).animated()` 使用 `theme.animations.normal`。
- `MotionPolicy`
  - 已接入 `App::with_motion_policy(...)`。
  - 支持 `enabled = false`、`reduced_motion`、`speed`、`deterministic`、固定 frame step。
  - `track_value` 会按 policy 跳过或缩放动画。
  - `pulse` 在禁用 / reduced motion 时会停止并清理对应 channel。
- `AnimationConfig` / `AnimationSlot`
  - 组件可声明使用 theme slot 或自定义 `AnimationSpec`。
- `AreaF`
  - 已提供 layout 动画后续需要的浮点 area 表示、插值和 cell area 转换。
- `LayoutTransition`
  - 已提供 position / size / size_and_position 声明结构。
- `ClipMode`
  - 已提供 layout transition 需要的 clip mode 类型。
- render `Chunk::with_clip(...)`
  - 已提供基础 clipping 能力。
- `Style::interpolate(...)`
  - RGB 颜色可插值，named / indexed color 和文本 attribute 使用离散切换。

它证明了：

- runtime 能在不要求业务层写 `on_frame` 的情况下推进组件动画。
- animation store 可以与 widget store 分离。
- `WidgetPath + channel` 可以作为动画身份。
- 组件可以在 `animate()` 写状态，在 `render()` 读状态。
- 全局 motion policy 可以统一影响组件动画，不需要每个组件自己判断测试模式或 reduced motion。
- theme 级动画参数可以被 `.animated()` 默认动画消费。

但它还不是完整系统。

尚未实现：

- layout area snapshot。
- animated layout。
- layout/render bridge 中使用 clipped child。
- 组件级 clipping 动画，例如 collapsible height animation。
- presence / exit retention。
- style token transition。
- timeline / choreography。
- wrapper-level `animate(child)` API。
- layout position/size track 的 runtime 更新。
- key-based list move detection。
- deterministic clock 的端到端 runtime 测试。

## 分阶段实现计划

### Phase 1：属性动画基础

状态：已完成。

目标：组件内部数值动画。

内容：

- `AnimationStore`
- `AnimationCtx`
- `Widget::animate`
- `RenderCtx::animation_value`
- `track_value`
- `pulse`
- 首批组件接入

验证：

```bash
cargo test -p tui
cargo check -p tui --examples
```

### Phase 2：Animation Theme 和 Motion Policy

状态：已完成基础 runtime 接入。

目标：让动画参数可统一配置，可在测试和 reduced motion 下禁用。

内容：

- [x] `AnimationTheme`
- [x] `MotionPolicy`
- [x] `App::with_motion_policy`
- [x] `Theme::animations`
- [x] deterministic clock runtime 入口
- [ ] deterministic clock 端到端测试覆盖

### Phase 3：Layout Snapshot 和 Animated Area

状态：部分完成 API 基础，runtime pipeline 尚未接入。

目标：支持位置、尺寸、重排动画。

内容：

- [ ] layout 前后 area snapshot。
- [x] `AreaF`
- [x] `LayoutTransition`
- [ ] position/size track。
- [ ] render 使用 displayed area。
- [ ] key-based list move detection。

### Phase 4：Clipping

状态：部分完成 render 基础设施，组件 / layout 集成尚未完成。

目标：支持高度动画、展开/收起、dialog scale。

内容：

- [x] render `Chunk` clip 支持。
- [ ] layout/render bridge 支持 clipped child。
- [x] `ClipMode`
- [ ] collapsible height animation。

### Phase 5：Presence

状态：未开始。

目标：支持 enter/exit。

内容：

- mounted / entering / exiting / unmounted 状态。
- exiting visual node retention。
- focus 与 exiting path 协作。
- overlay/dialog/toast/list item exit animation。

### Phase 6：Style 和 Timeline

状态：部分完成 style 插值基础，timeline 未开始。

目标：支持样式过渡和复杂编排。

内容：

- [ ] style token transition。
- [x] color interpolation / discrete fallback。
- [ ] sequence / parallel / stagger。
- [ ] wrapper-level `animate(child)` API。

## 测试策略

完整动画系统需要四类测试。

### Store 测试

- easing 边界。
- target retarget。
- duration zero。
- delay。
- repeat。
- completion 后停止 redraw。
- inactive path prune。

### Layout 测试

- area 从 previous 到 target 推进。
- list insert/remove/move。
- height collapse 不溢出。
- key 变化导致新 identity。

### Presence 测试

- enter 首帧和终态。
- exit retention。
- exit 完成后 prune。
- exiting widget 不参与 focus。

### Render 测试

- clipping 不覆盖相邻内容。
- reduced motion 直接终态。
- deterministic clock 输出稳定。
- 当前未开启动画的组件渲染保持兼容。

## 示例规划

当前示例：

```bash
cargo run -p tui --example animation
```

后续完整系统示例应覆盖：

- `animation`：综合展示。
- `layout_animation`：列表重排、面板展开、split resize。
- `presence_animation`：dialog、toast、conditional content enter/exit。
- `motion_policy`：禁用动画、reduced motion、speed 调试。

如果希望示例数量少，也可以保留一个综合示例，通过 tabs 分区展示上述能力。

## 设计原则

1. 业务层声明状态，动画层管理视觉连续性。
2. 组件默认静态，动画显式开启。
3. layout 动画必须进入 layout pipeline，不能伪装成 render hack。
4. exit 动画必须有 presence model，不能靠延迟删除业务 state。
5. TUI 动画以 cell/area/clip/style 为基本单位，不追求像素级 motion。
6. 所有动画都必须可禁用、可测试、可跳到终态。
7. 没有 stable key 的动态列表，不承诺正确重排动画。
