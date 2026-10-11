# 只抓取现场，不复位。使用与板上固件一致的 ELF，先运行 make debug。
set pagination off
set confirm off
set logging file target/hang.log
set logging overwrite on
set logging enabled on
monitor halt
bt
info registers
# 损坏的 SP 不能直接解引用，避免抓取脚本在清理断点前报错退出。
if $sp >= 0x20000000 && $sp <= 0x2001ffc0
  x/16wx $sp
end
# SCB：SHCSR / CFSR / HFSR / DFSR / MMFAR / BFAR。
x/6wx 0xe000ed24
# 调试停机状态、TIM2 计数、SPI2 / SPI1 配置和状态，不读取数据 FIFO。
x/wx 0xe000edf0
x/wx 0x40000024
x/6wx 0x40003800
x/6wx 0x40013000
x/wx 0x40005418
x/wx 0x40020000
delete breakpoints
monitor resume
detach
set logging enabled off
