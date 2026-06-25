.section ".text.boot"
.global _start

_start:


    // set stack pointer
    ldr     x0, =_stack_top
    mov     sp, x0

    // load exception vector table for EL1
    adr     x0, el1_vectors
    msr     VBAR_EL1, x0
    isb

    // diables fp/simd interrupts 
    mrs     x0, CPACR_EL1
    orr     x0, x0, #(3 << 20)
    msr     CPACR_EL1, x0
    isb


    // MMU setup
    ldr     x2, =0x3FFFFFFF // i have setup mmu for EL1 kernel space such that physical address = virtual address & 0x3FFFFFFF.
                           // this is because the paging configuration essentially offsets entire memory space to 
                          // virtual address 0xFFFF_FF80_0000_0000.

    // setting configurations    
    ldr     x0, =TCR_EL1_VALUE
    and     x0, x0, x2
    ldr     x0, [x0]
    msr     TCR_EL1, x0
    ldr     x0, =MAIR_EL1_VALUE
    and     x0, x0, x2
    ldr     x0, [x0]
    msr     MAIR_EL1, x0
    isb

    // linking up the kernel page tables from paging.rs
    ldr     x1, =PAGE_TABLE_KERNEL_L2
    and     x1, x1, x2 // link time addresses are virtual. i have things designed so & 3FFFFFFF gives physical address (for ttbr1)
    orr     x1, x1, #0b11
    ldr     x0, =PAGE_TABLE_KERNEL_L1
    and     x0, x0, x2
    str     x1, [x0]
    dsb     ishst

    msr     ttbr1_el1, x0 // load page table!
    isb

    // turning on the MMU
    mrs     x0, sctlr_el1
    ldr     x1, =SCTLR_EL1_ENABLE_MMU
    and     x1, x1, x2
    ldr     x1, [x1]
    orr     x0, x0, x1
    msr     sctlr_el1, x0
    isb

    // checking current exception level
    mrs     x0, CurrentEL
    cmp     x0, #0x8 // EL2?
    b.ne    el1_start
    
    // EL stack pointer
    ldr     x0, =_stack_top
    msr     SP_EL1, x0
    
    mov     x0, #(1 << 31) // bit 31 selects Aarch32/64
    msr     HCR_EL2, x0 // HCR_EL2 is like "a rulebook that EL2 writes for EL1"

    // set SPSR_EL2 to EL1h, basically once we ERET, we must switch to EL1h mode
    mov     x0, #(0b00101)  // EL1h 
    orr     x0, x0, #(0b1111 << 6) // mask all exceptions (D, A, I, F)
    msr     SPSR_EL2, x0

    // finally set the ERET PC
    adr     x0, _rust_main
    msr     ELR_EL2, x0

    eret // insane

el1_start:
    bl _rust_main

1:
    wfe
    b 1b
