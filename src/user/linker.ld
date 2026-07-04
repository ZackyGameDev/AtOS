ENTRY(_start)

SECTIONS
{
    . = 0x000000;

    .text : {
        *(.text*)
    }

    .rodata : {
        *(.rodata*)
    }

    .data : {
        *(.data*)
    }

    .bss : {
        *(.bss*)
    }
    
    . = ALIGN(16);
    _user_stack_top = . + 0x4000; /* unused */
}