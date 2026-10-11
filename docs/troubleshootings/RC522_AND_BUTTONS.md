# RC522、按键与偶发停机验证记录

## 1. 模块 SDA 引脚在 SPI 模式下是片选

原因：RC522 模块排针沿用 SDA 标记，但实际使用 SPI 模式。将其当 I2C 数据线会接错。
实现：SDA 接 PA4，SCK / MISO / MOSI 分别接 PA5 / PA6 / PA7，RST 接 PA3，IRQ 留空。
SPI1 与屏幕 SPI2、传感器 I2C1 分开，寄存器交换有 1ms 超时，失败释放 CS 并恢复 SPI1。

2026-10-11 初版实板：OpenOCD 写入和校验成功；模块 VersionReg=0xB2，
先后读到两个 4 字节 UID：`7C 22 40 2E`、`D3 F6 D9 53`。不是仅凭版本值判断读卡成功。
首次完整刷新率窗口约 59.00Hz，温湿度和气压均为 Fresh，CFSR=0。

## 2. 无卡不应等同通信失败或永久等待

实现：40kHz 定时器、200 个计数，空场 5ms 返回 None；软件计时 10ms 兜底。
单卡选卡验证 BCC 和 SAK CRC；碰撞、坏校验不发布 UID。初始化失败按 1s 退避重试。
测试覆盖 4 / 7 / 10 字节级联、重复 WUPA、HALT、碰撞、坏校验、计时回绕与总线错误。

初版故障注入：运行中通过 OpenOCD 清 SPI1 SPE，观察到 ReaderState 从 Waiting
进入 ReadFailed，随后约 1s 自动重新初始化回到 Waiting；SPI1 CR1 恢复为 0x1201。
期间采样计数继续递增、CFSR=0。原始日志位于本地忽略的 `target/rc522-timeout-recovery.log`。

## 3. 用户报告整屏停住、必须复位；根因尚未确认

2026-10-11 用户明确报告偶发永久停机，不能把读卡一次成功作为稳定性通过。
一分钟无断点检查中，采样计数由 40 连续递增至 52、TIM2 计数前进、CFSR 始终为 0、
OpenOCD 的目标状态为 running；本轮未复现永久停机。
原始记录位于本地忽略的 `target/rc522-live-check.log`，只能说明该分钟的观测结果。

诊断步骤：冻结后先不重启，保留同版 ELF，`make debug` 后执行 `make diagnose`。
脚本读取调用栈、寄存器、SCB、TIM2、SPI1 / SPI2 / I2C1 / DMA1 状态到 `target/hang.log`，
不读取可能弹出数据的 FIFO，不烧录或复位。读取后删除断点并显式恢复 CPU。
调试暂停也会冻结画面，必须与固件永久停机分开；当前未将调试器认定为根因。

## 4. 三个按钮只有两个能作为普通输入

依据项目原理图第 1 页：K1=PE3，K2=PC5，按下接地；RST 直接连 NRST。
第 3 页物料表也列出 K1、K2、RST 三个开关。BSP 为 K1 / K2 配内部上拉，应用消抖 30ms。
最终按用户要求改为 K1 上一页、K2 下一页，首尾循环；长按不重复触发，两键同时按住不切页。
默认环境页，切页不重置传感器记录或读卡器状态。

原理图渲染的 Poppler 输出缺少 Adobe-GB1 / SimSun 字体警告。
处理：结合渲染图中可辨识的 ASCII 网络名与 pdfplumber 提取的物料表交叉确认，
没有凭缺失的中文标签推测引脚。按键消抖、回绕和切页画面由自动测试验证；实物按键另验。

2026-10-11 分页固件验证：`make check-host`、`make test`（初版 31 项单元 + 9 项集成）、
`make check`、`make check-features`、`make examples`、`make build` 均通过；
`make flash` 写入、校验、复位成功。用户实测确认 K2 进入 RFID 页、K1 返回环境页，
两个键及来回切页均正常。像素测试同时确认原配色摘要不变、读卡页内容不越过主体区域、
长 UID 换短 UID 无残留、切回环境页像素完整恢复。

`make diagnose` 已在正常运行的同版固件上执行验证：调用栈处于主循环 1ms 延时，
CFSR / HFSR=0，SPI1 / SPI2 SR=0x1002；保存 `target/hang.log` 后删除断点、恢复并退出。
这一次是验证诊断命令可用，没有捕获到用户报告的永久停机。

后续上下键版本：新增两个方向各自连续循环的测试，共 32 项单元 + 9 项集成测试通过；
固件、全特性、示例构建通过，重新烧录并校验。默认环境页，K1 向上一页循环、K2 向下一页循环，
两页底栏提示 `K1: Up   K2: Down`。此前分页版本切页后的实板采样三项仍为 Fresh，刷新率约 59Hz。

## 5. 读取实板帧缓冲必须等文件写完

OpenOCD `dump_image` 读取 153,600 字节需要时间，仅按 socket 接收超时判断完成会提前处理半个文件，
导致 RGB565 解码报长度错误。应等待命令完成并确认文件长度为 320×240×2 后再解码。
本次等待写完后成功读取并检查独立 RFID 页，区域和文字没有重叠。

## 6. Classic 认证后的会话清理与 Key A 掩码

原因：UID 流程在 SELECT 后立即 HALT，不能直接在该接口后接 READ；认证成功时
MFRC522 的 Crypto1 状态还会影响下一次 WUPA。控制块读出的 Key A 区域为零掩码，
把它当作真实密钥或把认证失败当全零块，都会给出错误的检查结果。

修复：在驱动内部复用未 HALT 的选卡流程，只读会话核对卡片后认证并校验 READ CRC。
HALT 在加密仍开启时发送，之后清理 Idle、BitFraming 和 Crypto1；三项清理分别尝试，
不能因前一步失败就跳过后续清理。错误路径保留硬件故障并要求重新初始化。
应用每轮只尝试一个块和一个密钥槽，明确区分未读、读取成功、默认认证失败、NAK 和读取错误。

验证：mock 覆盖 4/7 字节 UID 的 A/B 认证命令、最后四字节 UID、READ CRC、NAK、
认证失败、卡片更换和认证计时回绕超时，检查清理后可再次读卡。
实际 1K 兼容卡的 64 块全部经默认 Key A 读取，普通数据块全零，控制块 Key A 区域为掩码；
现场 CFSR=0。没有写卡命令，无法据此推断物业权限数据格式。

## 7. GDB 栈深度与不含 Python 的工具链

原因：优化和内联会改变 `runtime::run` 的栈帧编号，固定 `frame 2` 可能选中时钟函数；
当前 arm-none-eabi-gdb 没有 Python 支持，不能使用 GDB Python 查找目标栈帧。

修复：使用原生 `frame apply all -q -s print reader...`，忽略没有局部变量的帧。
完整数组导出需 `set print elements 0` 和 `set print repeats 0`，否则重复数据会被省略。
`make rfid-read` 已实测从正在运行的主循环取得完整卡片参数与 256 项块槽位的快照，
其中只按 `total_blocks` 解释当前卡的有效范围，退出前恢复 CPU。

另一个 OpenOCD 进程占用 ST-Link 时，`make flash` 会报初始化失败。
应通过当前调试服务的 telnet 端口发送 `shutdown` 退出，再执行烧录；普通管道会话写入 Ctrl-C
未必会结束服务。本次关闭占用后烧录、校验、复位成功。


## 8. 读得到 Classic 块数据，不代表制造商块可改写

2026-10-11 用户要求把当前另一张卡的 UID 改为先前读取的卡号。
先用普通固件读取并备份目标卡完整块快照，再通过现有 OpenOCD/SWD 连接暂时停机，
使用 SPI1 的 8 位 TXDR/RXDR 和 PA4 片选执行本次检查；没有把写卡逻辑加入常规固件。
操作前检查 SPI1 空闲、RC522 Idle、目标 UID/ATQA/SAK，以及完整块 0 与备份一致。
只准备修改块 0 的四字节 UID 和对应 BCC，保留其他字节；没有操作其他块。

实际默认 Key A 认证及块 0 读取成功，但 WRITE 0xA0 / 块 0 第一阶段收到 4 位 NAK `0x04`，
因此没有发送 16 字节写入载荷。常见 Gen1 可改 UID 卡的 `0x40` 七位兼容入口也未应答，
没有继续解锁或写入。射频场重新开启后再次选卡并读取块 0，UID 和完整块内容与备份一致。
这证明本次两种访问方式均不能修改该卡，不能仅凭未响应就断言所有可改 UID 卡型都不支持。
普通正品 Classic 的制造商块由生产测试写保护，不能从普通数据块可读写推导 UID 可改。

结束时恢复 5ms RC522 定时器、Idle、BitFraming、Crypto1 和片选高电平，并恢复 CPU。
普通固件复读为 Present、原 UID，CFSR=0。本地记录为 `target/rfid-before-uid-write.log`、
`target/rfid-uid-inspect.json`、`target/rfid-uid-write.json` 和 `target/rfid-after-uid-write.log`。
本次操作脚本位于忽略的 `target/rc522_uid_once.py`，绑定现场目标 UID 和完整块 0，
不作为通用刷卡入口；源卡号和真实卡片快照均不自动提交。

协议依据：NXP MIFARE Classic EV1 1K 数据手册 8.6.1（制造商块写保护）、12.3（两阶段 WRITE）；
兼容入口参照 miguelbalboa/rfid 的 MIFARE_OpenUidBackdoor。直接调试 SPI 时必须保持 MCU 停机，
单次字节交换和射频轮询都设上界，退出前清理寄存器并恢复 CPU，避免与固件同时访问 SPI1。
