# 板载按键与 RC522 读卡演示

## 操作

普通固件默认显示环境监测。K1 上一页，K2 下一页，首尾循环，RST 是硬件复位。
K1 对应 PE3、K2 对应 PC5，均为按下接地，BSP 开启内部上拉。
接线依据本项目 `STM32H7XX_M_schematics.pdf` 第 1 页的按键区域和复位电路；
物料表第 3 页列出 K1、K2、RST 三个开关。RST 直接接 NRST，无法当普通 GPIO 键使用。

按键连续稳定 30ms 才接受，长按只触发一次；两键同时按住时不切页。
环境页保留原三张卡片和配色；RFID 页独立显示状态与卡号。
两页底栏都有按键提示、采样次数与刷新率，顶部继续显示动画。
切页只改变画面，传感器每 5s 采样、读卡每 200ms 查询，均继续在后台执行。

## 接线

断电接线，RC522 使用 3.3V 供电，与 STM32 共地。

| RC522 排针标记 | STM32H750 |
|---|---|
| 3.3V | 3.3V |
| GND | GND |
| SCK | PA5 / SPI1 SCK |
| MISO | PA6 / SPI1 MISO |
| MOSI | PA7 / SPI1 MOSI |
| SDA | PA4 / 软件 CS |
| RST | PA3 |
| IRQ | 不接，轮询寄存器 |

模块的 SDA 在 SPI 模式下是片选；不用接 I2C SDA。PA3 已占用，不能同时用于预留 NTC。
SPI1 使用 MODE_0、8 位、MSB-first，400MHz 内核 / 256，实际 1.5625MHz。
屏幕继续独占 SPI2 / DMA1，AHT20 与 BMP280 继续使用 I2C1。

## 读卡行为与模块职责

- `bsp/buttons.rs` 读物理电平，`application/navigation.rs` 消抖并选择页面。
- `drivers/rc522.rs` 管理寄存器帧、WUPA、单卡防碰撞、SELECT、BCC / CRC_A 和 HALT。
- `bsp/rc522.rs` 独占 SPI1、CS、RST，执行有期限的交换和控制器恢复。
- `application/rfid.rs` 管理重试、在场状态和最近成功 UID。
- `application/presentation.rs` 转换标签和卡号文字；`ui/rfid_page.rs` 只接收 UI 输入。

支持 ISO14443A 的 4 / 7 / 10 字节 UID；每次成功后 HALT，下次 WUPA 可再次读取停留的卡。
当前演示为单卡读号，碰撞返回错误；不读写卡片数据区，不执行密钥认证。
卡在场显示 `Card detected` 和 `Card UID`；移开后显示 `Waiting for card` 和 `Last UID`，
保留上次成功卡号。未读到卡时显示占位符。
初始化失败显示 `RC522: check wiring`；读卡失败显示 `Read error / retrying`。

复位拉低 2ms、拉高后等待 50ms，仅初始化或通信故障恢复时执行，失败至少 1s 后再试。
SPI 寄存器交换上限 1ms；射频帧硬件定时 5ms，软件兜底 10ms。
成功与失败均释放 CS，结束射频帧时清理命令与 StartSend；总线错误后重新初始化。
这些调用有界阻塞，初始化及每 5s 的 AHT20 测量仍可能使动画短暂跳帧。

协议依据：[NXP MFRC522 数据手册 Rev.3.9](https://www.nxp.com/docs/en/data-sheet/MFRC522.pdf)
第 8.1.2、8.5、9、10 章，以及 [miguelbalboa/rfid](https://github.com/miguelbalboa/rfid/blob/master/src/MFRC522.cpp)
的选卡流程。软件 CRC 使用初值 0x6363、多项式 0x8408，低字节先传输。
版本寄存器不限定为原厂 0x91 / 0x92；当前模块实测 0xB2，仍需实际读卡验证兼容性。

## 构建与验收

```sh
make check-host
make test
make check
make check-features
make examples
make flash
```

上板后检查：默认环境页按 K2 进入独立读卡页，卡贴近天线显示 UID；移开后保留最近卡号；
K1 返回环境页，三项读数仍更新、卡片没有残留；重复切页和长按不应反复跳动。
真实 7 / 10 字节卡未提供时，只能确认其协议 mock 测试，不能声称已实板验证。

2026-10-11 实测：4 字节 UID 已读取，用户确认初版 K1 / K2 与两页来回切换正常；随后按要求改为上下键首尾循环。
全部 41 项主机测试、固件和全特性检查通过，OpenOCD 烧录校验成功。
长期稳定性仍有用户报告的偶发永久停机未定位，详见 [验证记录](troubleshootings/RC522_AND_BUTTONS.md)。

偶发永久停机另行追踪：再现时先保留现场，不要复位。另一个终端启动 `make debug`，
执行 `make diagnose` 保存调用栈、CPU / SCB / 总线状态到 `target/hang.log`。
必须使用与板上固件一致的 ELF。脚本不烧录、不复位；读取后删除断点、恢复 CPU 并退出。
这只是诊断入口，不能代替查清停机原因。
