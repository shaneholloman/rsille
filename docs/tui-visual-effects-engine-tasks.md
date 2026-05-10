# TUI Visual Effects Engine 子任务拆分

本文档把 [tui-visual-effects-engine.md](./tui-visual-effects-engine.md) 中的设计路线拆成可执行子任务。原设计文档保留为总体方向，本目录下的 task 文档用于实现、评审和验收。

## 推荐执行顺序

1. [x] [Task 01: VisualCtx 与 CellSample 模型](./tui-visual-effects-task-01-visual-ctx-cell-sample.md)
2. [x] [Task 02: VisualConfig 与 cell aspect](./tui-visual-effects-task-02-visual-config-cell-aspect.md)
3. [x] [Task 03: Offscreen / blit 共享 helper](./tui-visual-effects-task-03-offscreen-blit-helper.md)
4. [Task 04: MotionPolicy 与 reduced motion 降级](./tui-visual-effects-task-04-reduced-motion.md)
5. [Task 05: 效果组合器与 stagger](./tui-visual-effects-task-05-composition-stagger.md)
6. [Task 06: 内置效果库扩展](./tui-visual-effects-task-06-built-in-effects.md)
7. [Task 07: 示例与视觉回归场景](./tui-visual-effects-task-07-examples.md)
8. [Task 08: Visual enter / exit 生命周期 API](./tui-visual-effects-task-08-enter-exit-lifecycle.md)
9. [Task 09: Theme effect preset](./tui-visual-effects-task-09-theme-presets.md)
10. [Task 10: 性能优化与大区域降级](./tui-visual-effects-task-10-performance.md)
11. [Task 11: 用户自定义 effect 与 profiling hooks](./tui-visual-effects-task-11-custom-effects-profiling.md)

## 里程碑映射

| 原设计 Phase | 对应任务 |
| --- | --- |
| Phase 1: 基础引擎稳固 | Task 01, 02, 03, 04 |
| Phase 2: 效果库扩展 | Task 05, 06, 07 |
| Phase 3: 生命周期与主题 | Task 08, 09 |
| Phase 4: 性能与用户扩展 | Task 10, 11 |

## 全局验收原则

- 普通 widget 不需要知道视觉效果存在。
- 几何效果必须考虑 terminal cell 高宽比。
- 所有大幅运动效果都必须有 reduced motion 替代。
- exit 效果默认不改变事件语义，hit test 仍以逻辑目标区域为准。
- 新效果优先复用 mask、timeline、颜色插值、offscreen/blit 等基础设施。
- 大区域、低性能终端或 disabled motion 策略下应能自动跳过或降级。
