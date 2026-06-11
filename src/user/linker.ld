ENTRY(_start)

SECTIONS
{
    /* once memory virtualization works, i will change this to start at 0x00000000 */
    . = 0x200000;

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
    _user_stack_top = . + 0x4000;
}