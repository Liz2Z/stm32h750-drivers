MEMORY {
    FLASH   : ORIGIN = 0x08000000, LENGTH = 128K
    RAM     : ORIGIN = 0x20000000, LENGTH = 128K
    AXISRAM : ORIGIN = 0x24000000, LENGTH = 512K
}

_stack_start = ORIGIN(RAM) + LENGTH(RAM);

/* AXISRAM section for DMA buffers - NOLOAD prevents inclusion in binary */
SECTIONS {
    .axisram (NOLOAD) : ALIGN(8) {
        *(.axisram .axisram.*);
        . = ALIGN(8);
    } > AXISRAM
};

_semihosting = 0;
