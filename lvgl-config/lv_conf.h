/**
 * @file lv_conf.h
 * @brief LVGL 配置文件 for STM32H750
 *
 * 这个文件配置了 LVGL 在 STM32H750 上的运行参数。
 * STM32H750 有 1MB Flash 和 864KB RAM，可以支持较复杂的 UI。
 */

#ifndef LV_CONF_H
#define LV_CONF_H

#include <stdint.h>

/*====================
   颜色设置
 *====================*/

/* 颜色深度: 1, 8, 16 or 32 */
#define LV_COLOR_DEPTH 16

/* 交换 RGB565 颜色的 2 字节。
 * 如果屏幕上红色和蓝色显示反了，启用这个 */
#define LV_COLOR_16_SWAP 0

/* 启用功能更强的颜色混合例程（需要更多 RAM） */
#define LV_COLOR_SCREEN_TRANSP 0

/* 图像像素预乘 Alpha 值 */
#define LV_COLOR_PREMULTIPLY 0

/*====================
   内存设置
 *====================*/

/* LVGL 使用的内存大小（字节）
 * STM32H750 有充足 RAM，可以设置较大值 */
#define LV_MEM_SIZE (64U * 1024U)  /* 64KB */

/* 启用自定义内存管理器 */
#define LV_MEM_CUSTOM 0

/* 最大同时可以旋转的内存块数 */
#define LV_MEM_BUF_MAX_NUM 16

/* 使用标准库 `memcpy` 和 `memset` */
#define LV_MEMCPY_MEMSET_STD 0

/*====================
   显示设置
 *====================*/

/* 显示刷新周期（毫秒） */
#define LV_DISP_DEF_REFR_PERIOD 30

/* 输入设备读取周期（毫秒） */
#define LV_INDEV_DEF_READ_PERIOD 30

/* 使用自定义 tick 源 - 禁用，使用 Rust 代码提供 tick */
#define LV_TICK_CUSTOM 0

/* 默认字体 */
#define LV_FONT_MONTSERRAT_8  0
#define LV_FONT_MONTSERRAT_10 0
#define LV_FONT_MONTSERRAT_12 1
#define LV_FONT_MONTSERRAT_14 1
#define LV_FONT_MONTSERRAT_16 1
#define LV_FONT_MONTSERRAT_18 0
#define LV_FONT_MONTSERRAT_20 0
#define LV_FONT_MONTSERRAT_22 0
#define LV_FONT_MONTSERRAT_24 1
#define LV_FONT_MONTSERRAT_26 0
#define LV_FONT_MONTSERRAT_28 0
#define LV_FONT_MONTSERRAT_30 0
#define LV_FONT_MONTSERRAT_32 0
#define LV_FONT_MONTSERRAT_34 0
#define LV_FONT_MONTSERRAT_36 0
#define LV_FONT_MONTSERRAT_38 0
#define LV_FONT_MONTSERRAT_40 0
#define LV_FONT_MONTSERRAT_42 0
#define LV_FONT_MONTSERRAT_44 0
#define LV_FONT_MONTSERRAT_46 0
#define LV_FONT_MONTSERRAT_48 0

/* 特殊字体 */
#define LV_FONT_MONTSERRAT_12_SUBPX 0
#define LV_FONT_MONTSERRAT_28_COMPRESSED 0
#define LV_FONT_DEJAVU_16_PERSIAN_HEBREW 0
#define LV_FONT_SIMSUN_16_CJK 0
#define LV_FONT_UNSCII_8  0
#define LV_FONT_UNSCII_16 0

/* 自定义字体声明 */
#define LV_FONT_CUSTOM_DECLARE

/* 始终开启的字体（即使没有任何控件使用） */
#define LV_FONT_DEFAULT &lv_font_montserrat_14

/* 启用字体压缩 */
#define LV_USE_FONT_COMPRESSED 1

/* 启用子像素渲染 */
#define LV_USE_FONT_SUBPX 0

/*====================
   文本设置
 *====================*/

/* 支持的字符编码 */
#define LV_TXT_ENC LV_TXT_ENC_UTF8

/* 文本换行符 */
#define LV_TXT_BREAK_CHARS " ,.;:-_"

/* 文本行长提示 */
#define LV_TXT_LINE_BREAK_LONG_LEN 0

/* 长文本换行长度 */
#define LV_TXT_LINE_BREAK_LONG_PRE_MIN_LEN 3
#define LV_TXT_LINE_BREAK_LONG_POST_MIN_LEN 3

/* 文本选择 */
#define LV_USE_TEXT_SELECTION 1

/* 文本插入光标闪烁时间（毫秒） */
#define LV_USE_TEXT_CURSOR_PLACEHOLDER 0

/*====================
   控件设置
 *====================*/

/* 基础控件 */
#define LV_USE_ARC        1
#define LV_USE_BAR        1
#define LV_USE_BTN        1
#define LV_USE_BTNMATRIX  1
#define LV_USE_CANVAS     1
#define LV_USE_CHECKBOX   1
#define LV_USE_DROPDOWN   1
#define LV_USE_IMG        1
#define LV_USE_LABEL      1
#define LV_USE_LINE       1
#define LV_USE_ROLLER     1
#define LV_USE_SLIDER     1
#define LV_USE_SWITCH     1
#define LV_USE_TEXTAREA   1
#define LV_USE_TABLE      1

/* 额外控件 */
#define LV_USE_ANIMIMG    1
#define LV_USE_CALENDAR   1
#define LV_USE_CHART      1
#define LV_USE_COLORWHEEL 1
#define LV_USE_IMGBTN     1
#define LV_USE_KEYBOARD   1
#define LV_USE_LED        1
#define LV_USE_LIST       1
#define LV_USE_MENU       1
#define LV_USE_METER      1
#define LV_USE_MSGBOX     1
#define LV_USE_SPINBOX    1
#define LV_USE_SPINNER    1
#define LV_USE_TABVIEW    1
#define LV_USE_TILEVIEW   1
#define LV_USE_WIN        1
#define LV_USE_SPAN       1

/*====================
   主题设置
 *====================*/

/* 一个简单的出色主题 */
#define LV_USE_THEME_DEFAULT 1
#define LV_THEME_DEFAULT_DARK 0
#define LV_THEME_DEFAULT_GROW 1
#define LV_THEME_DEFAULT_TRANSITION_TIME 80

/* 一个非常简单的主题 */
#define LV_USE_THEME_BASIC 1

/* 一个单色的主题 */
#define LV_USE_THEME_MONO 1

/*====================
   布局设置
 *====================*/

/* 类似 Flexbox 的布局 */
#define LV_USE_FLEX 1

/* 类似 CSS Grid 的布局 */
#define LV_USE_GRID 1

/*====================
   3D 绘制支持
 *====================*/

#define LV_USE_SDL 0
#define LV_USE_THORVG 0
#define LV_USE_LZ4  0

/*====================
   日志设置
 *====================*/

/* 启用日志模块 */
#define LV_USE_LOG 1
#define LV_LOG_LEVEL LV_LOG_LEVEL_WARN
#define LV_LOG_PRINTF 0
#define LV_LOG_TRACE_MEM 1
#define LV_LOG_TRACE_TIMER 1
#define LV_LOG_TRACE_INDEV 1
#define LV_LOG_TRACE_DISP_REFR 1
#define LV_LOG_TRACE_EVENT 1
#define LV_LOG_TRACE_OBJ_CREATE 1
#define LV_LOG_TRACE_LAYOUT 1
#define LV_LOG_TRACE_ANIM 1
#define LV_LOG_TRACE_MSG 1

/*====================
   断言设置
 *====================*/

/* 如果值为假，则停止执行 */
#define LV_USE_ASSERT_NULL          1
#define LV_USE_ASSERT_MALLOC        1
#define LV_USE_ASSERT_STYLE         0
#define LV_USE_ASSERT_MEM_INTEGRITY 0
#define LV_USE_ASSERT_OBJ           0
#define LV_USE_ASSERT_MALLOC        1

/* 在断言时输出消息 */
#define LV_ASSERT_HANDLER_INCLUDE <stdint.h>
#define LV_ASSERT_HANDLER while(1);   /* 在失败时停止 */

/*====================
   其他设置
 *====================*/

/* 1: 使用浮点数运算 */
#define LV_USE_FLOAT 0

/* 1: 启用大数组优化 */
#define LV_USE_LARGE_COORD 0

/* 编译时字体名称 */
#define LV_USE_PERF_MONITOR 0
#define LV_USE_MEM_MONITOR  0

/* 绘制复杂形状支持 */
#define LV_USE_DRAW_MASKS 1
#define LV_USE_DRAW_SW    1

/* 阴影 */
#define LV_USE_SHADOW 1
#define LV_SHADOW_CACHE_SIZE 0

/* 圆角矩形优化 */
#define LV_USE_RADIUS 1

/* 图案（pattern）缓存 */
#define LV_USE_IMG_TRANSFORM 1

/* 用户数据类型 */
#define LV_USE_USER_DATA 1

/* 垃圾回收（用于 MicroPython） */
#define LV_ENABLE_GC 0

/*====================
   编译器设置
 *====================*/

/* 对于大数组/对象使用未初始化变量 */
#define LV_ATTRIBUTE_LARGE_CONST
#define LV_ATTRIBUTE_LARGE_RAM_ARRAY

/* 将性能关键函数放入 RAM */
#define LV_ATTRIBUTE_FAST_MEM

/* 前缀用于声明为 `static` 的变量 */
#define LV_ATTRIBUTE_STATIC_VAR

/* 对齐设置 */
#define LV_ATTRIBUTE_MEM_ALIGN

/* 导出到 C 的函数属性 */
#define LV_EXPORT_CONST_INT(int_value) struct _silence_gcc_warning

/* 扩展属性 */
#define LV_USE_LARGE_COORD 0

/* 格式化属性 */
#if defined(_MSC_VER)
#  define LV_FORMAT_ATTRIBUTE(fmt, start)
#else
#  define LV_FORMAT_ATTRIBUTE(fmt, start) __attribute__((format(printf, fmt, start)))
#endif

/* 输入设备设置 */
#define LV_USE_INDEV_TOUCHPAD 1
#define LV_USE_INDEV_MOUSE    0
#define LV_USE_INDEV_KEYPAD   0
#define LV_USE_INDEV_ENCODER  0
#define LV_USE_INDEV_BUTTON   0

/* 输入设备读取定时器 */
#define LV_INDEV_DEF_READ_PERIOD 30

/* 触摸屏校准 */
#define LV_USE_INDEV_TOUCHPAD_CALIBRATION 0

/* 更多触摸点 */
#define LV_USE_INDEV_TOUCHPAD_MULTI_TOUCH 0

/* 手势识别 */
#define LV_USE_INDEV_GESTURE 1

/* 输入设备触摸反馈 */
#define LV_USE_INDEV_FEEDBACK 0

#endif /* LV_CONF_H */
