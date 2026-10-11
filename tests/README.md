# 测试组织与运行

项目区分模块单元测试和外部集成测试，统一通过根目录的 `make test` 运行。
测试在电脑上执行，真实 SPI/I2C 时序、DMA 和缓存行为仍由实板验证。

## 放在哪里

| 测试对象 | 位置 |
|---|---|
| 私有记录、输入校验与极值 | `src/application/acquisition/record.rs` 的 `tests` 子模块 |
| 采集组件内部拒绝非法值 | `src/application/acquisition/mod.rs` 的 `tests` 子模块 |
| 周期期限、回绕与刷新率计算 | `src/application/scheduling.rs` 的 `tests` 子模块 |
| 底栏状态到显示标签的转换 | `src/application/presentation.rs` 的 `tests` 子模块 |
| AHT20、BMP280 协议与失败恢复 | `src/drivers/aht20.rs`、`src/drivers/bmp280.rs` 的 `tests` 子模块 |
| 显示驱动协议、像素打包与错误清理 | `src/drivers/display/tests.rs` |
| 私有文字格式化 | `src/ui/sensor_format.rs` 的 `tests` 子模块 |
| 驱动采集到显示转换、失败恢复与多余刷新 | [acquisition.rs](acquisition.rs) |
| 控件组合画面、局部刷新、颜色与动画范围 | [ui_rendering.rs](ui_rendering.rs) |
| 集成测试共享的模拟屏幕与布局 | [common/mod.rs](common/mod.rs) |

单元测试由对应模块通过 `#[cfg(test)] mod tests` 声明，允许访问该模块的内部实现。
单文件模块的测试直接写在源码末尾，不单独建立只存放测试的同名目录。
本身已有实现目录的模块（如 `drivers/display/`），可将测试放在目录内的 `tests.rs`。

根目录 `tests/` 的两个 `.rs` 文件由 Cargo 自动发现，各自作为独立测试程序编译。
它们通过 `use stm32h750_drivers::...` 使用库的公开接口，不导入私有记录或 UI 格式化模块，
也不重复挂载生产源码。辅助代码放在 `common/mod.rs`，避免被识别成额外测试程序。
传感器单元测试使用各器件所需的精简模拟；采集集成测试使用可独立断连的共享总线模拟。

## 运行方式

在项目根目录执行：

```sh
make test          # 宿主机单元测试 + 集成测试
make check-host    # 同时检查宿主机库、单元测试与集成测试
make check         # 格式与 ARM 固件检查
```

`make test` 显式选择 rustc 的 host target，并同时选择 `--lib --tests`。
只用 `--lib` 会漏掉根目录集成测试；只改变测试目录不会解决编译目标错误。

固件入口要求 `firmware` 特性，默认启用，因此 `make build`、`make flash` 的行为保持一致。
宿主机测试使用 `--no-default-features` 跳过裸机可执行文件，避免 Cargo 为集成测试自动
构建 STM32 入口。该特性只选择固件入口，不改变库的采集或显示逻辑。
无需关闭测试诊断、为私有函数增加 `pub`，或引入宿主机假的固件入口。

CI 的 `host-tests` 作业复用 `make check-host` 和 `make test`。
当前覆盖 23 项单元测试与 7 项集成测试；原有十组 RGB565 画面摘要保持一致。
