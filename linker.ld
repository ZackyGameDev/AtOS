ENTRY(_start)

SECTIONS
{
    . = 0xFFFFFF8000080000; /* higher range TTBR1 offsetted*/

    .text : ALIGN(4096) 
    {
        *(.text.boot)
        *(.text.vectors)
        *(.text*)
    }

    .boot.constants : ALIGN(8)
    {
        KEEP(*(.boot.constants))
    }

    .rodata : ALIGN(4096) 
    { 
        *(.rodata*) 
    }
    
    .data : ALIGN(4096)
    { 
        *(.data*) 
    }
    
    .page_tables : ALIGN(4096)
    {
        _page_tables_start = .;

        KEEP(*(.page_tables.kernel.l1))
        . = ALIGN(4096);

        KEEP(*(.page_tables.kernel.l2))
        . = ALIGN(4096);

        _page_tables_end = .;
    }

    .bss : ALIGN(4096) 
    { 
        *(.bss*)     
        *(COMMON)
    }

    _kernel_top = .;

    . = ALIGN(16);
    _stack_top = . + 0x8000;
}
