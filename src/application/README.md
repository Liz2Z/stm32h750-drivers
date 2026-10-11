# Application：采集组件、流程调度和显示转换

采集由 `Acquisition` 完整负责。主循环决定何时采集，把结果快照交给显示转换；
它不持有或修改温度、湿度、气压的内部记录。

| 文件 | 代码职责 |
|---|---|
| [acquisition/mod.rs](acquisition/mod.rs) | 持有驱动、记录及计数；一次 `sample` 完成读取、失败处理和更新；提供结果快照 |
| [acquisition/record.rs](acquisition/record.rs) | 采集组件私有的记录实现，校验数值并维护极值、历史、状态和成功时间戳 |
| [runtime.rs](runtime.rs) | 调度采集、动画、显示恢复、底栏更新和可选通信 |
| [presentation.rs](presentation.rs) | 将采集快照转换为 UI 声明的卡片输入，将连接状态转换为底栏标签 |
| [scheduling.rs](scheduling.rs) | 判断周期期限，统计成功刷新率 |
| [connection.rs](connection.rs) | 连接与上传流程状态枚举 |
| [config.rs](config.rs) | 应用配置，普通固件默认关闭 ESP 通信 |

```text
runtime：期限到了
  → Acquisition.sample：读取、记录、统计、计数
  → Acquisition.snapshot：取得结果副本
  → presentation：转为控件输入
  → UI：比较输入、绘图、刷新
```

`record` 模块私有，`Record` 仅对父模块可见。外部只能调用采集组件，不能单独写记录、
标记失败或递增采样次数。`ReadingSnapshot` 提供只读查询；取得的是副本，后续采样不会
改变已取得的快照。[采集说明](acquisition/README.md) 详细解释接口、单位与失败语义。

UI 在控件中定义自己需要的 `TempHumidCardData`、`PressureCardData` 和 `StatusData`。
`presentation` 同时认识采集快照和控件输入，承担转换；UI 无需导入采集组件、记录实现
或 ESP 状态。卡片输入不携带历史和时间戳，因此仅这些信息变化不会触发重绘。

采集与转换可在宿主机测试，硬件配置和运行入口由 ARM 条件编译隔离。
`make test` 同时运行模块单元测试和根目录集成测试，`make check-host` 检查宿主机库与全部测试，
`make check` 检查固件。调度和显示转换单元测试内联在各自源码末尾的 `tests` 子模块中，
跨模块采集流程位于 [tests/acquisition.rs](../../tests/acquisition.rs)，详见 [测试说明](../../tests/README.md)。
