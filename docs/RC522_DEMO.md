# 板载按键与 RC522 读卡演示

## 操作

普通固件默认显示环境监测。K1 上一页，K2 下一页，首尾循环，RST 是硬件复位。
K1 对应 PE3、K2 对应 PC5，均为按下接地，BSP 开启内部上拉。
接线依据本项目 `STM32H7XX_M_schematics.pdf` 第 1 页的按键区域和复位电路；
物料表第 3 页列出 K1、K2、RST 三个开关。RST 直接接 NRST，无法当普通 GPIO 键使用。

按键连续稳定 30ms 才接受，长按只触发一次；两键同时按住时不切页。
环境页保留原三张卡片和配色；RFID 页独立显示状态、卡号、兼容卡型、ATQA/SAK 与内存扫描进度。
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
- `application/rfid.rs` 管理重试、在场状态、最近卡片参数及逐块内存快照。
- `application/presentation.rs` 转换标签和卡号文字；`ui/rfid_page.rs` 只接收 UI 输入。

支持 ISO14443A 的 4 / 7 / 10 字节 UID；每次成功后 HALT，下次 WUPA 可再次读取停留的卡。
当前为单卡检查，碰撞返回错误；支持 Classic Mini / 1K / 4K 布局的只读检查，其他卡型只显示公开参数。
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

## 卡片信息与只读内存检查

`read_card` 保留 UID、ATQA（空口字节序）和最后一级 SAK。SAK 仅用于初步判断兼容类型，
不证明卡片品牌、芯片真伪或物业使用的数据格式。Classic 1K 推定为 16 扇区、64 个 16 字节块。
`read_uid` 保留为只返回 UID 的兼容入口。

`read_classic_block` 由调用者传入块地址、Key A/B 类型和 6 字节密钥。驱动每次重新选卡、
核对 UID/ATQA/SAK，再通过 MFRC522 的 MFAuthent 认证，发 READ 0x30，验证 16 字节数据的 CRC_A。
成功后在 Crypto1 开启时 HALT，再关闭 Crypto1；失败也尝试停止命令、清理位帧配置与加密状态。
硬件/总线故障要求重新初始化。卡移开、换卡、认证失败、4 位 NAK 和 CRC 错误分别返回，
驱动不保存、猜测密钥，也没有写卡命令。

应用每 200ms 最多读取一个块、尝试一次认证。只使用出厂密钥 `FF FF FF FF FF FF`：
先试 Key A，无法读取时下一轮试同一密钥的 Key B；读取失败保留明确状态，不用全零代替成功。
卡一直贴近时，1K 卡约 13 秒完成全部 A 读取；全部需要 A/B 两次尝试时约 26 秒，另计通信和调度开销。
卡移开暂停并保留快照，同一 UID 再放回继续；换一张卡重新开始。完成后不反复扫描，复位可重新读取。
两张卡即使 UID/ATQA/SAK 完全相同也无法靠这些参数区分，不能把此核对当作身份认证。
Classic 4K 的容量根据 SAK 推定，尚无对应实卡验证；7 字节 UID 的认证有协议 mock 覆盖，尚无实卡验证。

屏幕 `Read` 表示实际读取成功的块数，`Scanned` 包含成功和已尝试失败的块；
`Default key failed` 表示该块的 A/B 默认密钥认证均未成功，不表示已确认其实际密钥。
原始快照中的状态分为 Pending、ReadWithA、ReadWithB、DefaultKeyFailed、ReadDenied 和 ReadFailed。
总容量含制造商块与扇区控制块，不等于用户数据容量。
扇区控制块的 Key A 不能被直接读取，返回的零值是掩码；Key B 是否可见由访问位决定。
读到原始数据也不等于能解释房号、姓名或有效期，门禁权限可能只保存在管理系统中。

先在另一个终端运行 `make debug`，使用与板上固件一致的 ELF 执行 `make rfid-read`，
保存卡片参数、状态、全部块结果和 CFSR 到本地 `target/rfid-read.log`。
命令短暂停机取得一致快照，结束后恢复 CPU；它不复位、不重新烧录，也不是连续串口监听。
主机必须能通过 SWD 找到运行中的应用栈帧。内联导致栈深度变化时，脚本逐帧寻找 `reader`。
读卡日志只留在已忽略的 `target/`，不自动提交真实卡片数据。

协议依据另见 [NXP MIFARE Classic EV1 1K](https://www.nxp.com/docs/en/data-sheet/MF1S50YYX_V1.pdf)
的认证、READ、扇区控制块与访问位说明，以及 MFRC522 数据手册 10.3.1.9 的 MFAuthent。

2026-10-11 本次实卡检查：一张 4 字节 UID、ATQA `04 00`、SAK `08` 的 1K 兼容卡，
64/64 块通过默认 Key A 认证并读取，CRC 校验通过，47 个普通数据块均为零；
16 个控制块的访问位均为 `FF 07 80`、通用字节为 `69`。未发现可识别的门禁应用数据。
这与按 UID 在后台查询权限的用法相符，但不能据此断言物业系统的实现。
现场报告在本地 `target/rfid-card-report.txt` 与 `.json`，其中包含真实 UID 和逐块原始内容。

本次共 46 项测试通过（37 项单元、9 项集成），宿主机、ARM 与全特性 Clippy、示例构建通过。
最终固件已重新烧录校验，重新扫描仍为 64/64 块读取成功，CFSR=0；偶发停机问题仍需另行捕获。
