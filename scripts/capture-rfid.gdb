# 先运行 make debug；必须使用与板上固件一致的 ELF，不烧录、不复位。
set pagination off
set confirm off
set print pretty off
set print elements 0
set print repeats 0
set logging file target/rfid-read.log
set logging overwrite on
set logging enabled on
monitor halt
# 栈深度随内联变化，逐帧寻找局部变量；-s 省略无该变量的栈帧。
frame apply all -q -s print reader.state
frame apply all -q -s print reader.last_card
frame apply all -q -s print reader.last_error
frame apply all -q -s print reader.memory
x/wx 0xe000ed28
delete breakpoints
monitor resume
detach
set logging enabled off
