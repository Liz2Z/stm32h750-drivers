# UI：控件声明输入，负责绘制和刷新

本模块负责控件布局、主题、文字格式化、动画和脏区域刷新。
每个控件声明自己需要的数据；UI 不依赖采样记录、应用流程或具体 MCU HAL。

## 控件输入

| 定义位置 | 输入类型 | 控件需要什么 |
|---|---|---|
| [widgets/card.rs](widgets/card.rs) | `TempHumidCardData` | 当前/最高/最低值和提示状态；由卡片模式选择温度或湿度单位 |
| [widgets/pressure_card.rs](widgets/pressure_card.rs) | `PressureCardData` | hPa 数值范围和提示状态 |
| [widgets/value.rs](widgets/value.rs) | `ValueRange`、`ValueStatus` | 数值控件共用的输入结构与显示提示 |
| [status.rs](status.rs) | `StatusData` | 连接标签、上传标签、颜色提示、采样次数和刷新率 |
| [status.rs](status.rs) | `StatusMessage` | 调用方提供的一行文字与提示色；用于底栏第一行，UI 无需知道文字来自读卡器 |
| [rfid_page.rs](rfid_page.rs) | `RfidPageData` | 状态文字、提示色、卡号文字与标题；不依赖 RC522 或应用的页面枚举 |

卡片的 `values: None` 表示显示占位符，`Some` 可以包含真实的零值。
`ValueStatus` 决定显示 WAIT、STALE、ERROR 或空提示；采样状态如何映射，由应用完成。
控件不接收历史数组、采样时间戳或传感器驱动，也不计算累计极值。

## 数据从哪里来

[application/presentation.rs](../application/presentation.rs) 将采集快照和连接状态
转换为上述类型，再交给 `Screen::update_sensors` 或 `draw_status`。
`draw_status_with_message` 在现有 320×20 底栏内显示两行，保留采样次数和刷新率；
传入 `None` 时保留原底栏布局和像素。每次清理整个底栏，长卡号换成短提示不会留残字。
普通固件的底栏提示 K1 / K2；读卡内容放在独立的 y=20..219 页面主体，
`draw_rfid_page` 只绘图，由应用选择页面并刷新。切回环境页调用原 `Screen` 完整重画。
转换使用 UI 声明的接口，不能在 UI 中加入 `From<ReadingSnapshot>` 或应用模块的导入。
控件也可以由测试、演示或其他应用直接构造输入，不需要创建采样记录。

`Screen` 比较新旧卡片输入，变化时标记数值区域；历史或时间戳变化不会单独触发重绘。
刷新失败保留脏区域供下次重试。格式化和单位由 [sensor_format.rs](sensor_format.rs) 处理，
布局和配色留在控件与 [theme.rs](theme.rs) 中。

## 显示接口和验证

UI 通过 embedded-graphics 的 `DrawTarget` 绘图，通过 `FlushTarget` 刷新画面。
[screen.rs](screen.rs) 为通用显示驱动实现刷新适配；SPI2、DMA 和缓冲存储由 BSP 实现。

运行 `make test` 检查控件输入、局部刷新、失败重试和动画范围。
私有格式化单元测试留在 `sensor_format.rs`；[tests/ui_rendering.rs](../../tests/ui_rendering.rs)
通过公开 UI 输入验证完整画面。像素快照对比改动前画面，覆盖等待、失败、正常、旧值、恢复及底栏模式。
