.section ".text.boot"
.global _start

_start:

    // MMU setup
    ldr     x7, =0x3FFFFFFF // i have setup mmu for EL1 kernel space such that physical address = virtual address & 0x3FFFFFFF.
                           // this is because the paging configuration essentially offsets entire memory space to 
                          // virtual address 0xFFFF_FF80_0000_0000.

    // setting configurations 
    ldr     x0, =TCR_EL1_VALUE
    and     x0, x0, x7          // link time addresses are virtual. i have things designed so & 3FFFFFFF gives physical address (for ttbr1)
    ldr     x0, [x0]
    msr     TCR_EL1, x0
    ldr     x0, =MAIR_EL1_VALUE
    and     x0, x0, x7          // to physical address
    ldr     x0, [x0]
    msr     MAIR_EL1, x0
    isb

    // linking up the kernel page tables from paging.rs
    ldr     x1, =PAGE_TABLE_KERNEL_L2_0
    and     x1, x1, x7          // to physical address
    orr     x1, x1, #0b11
    ldr     x2, =PAGE_TABLE_KERNEL_L2_1  
    and     x2, x2, x7          // to physical address
    orr     x2, x2, #0b11
    ldr     x0, =PAGE_TABLE_KERNEL_L1
    and     x0, x0, x7          // to physical address
    str     x1, [x0]
    str     x1, [x0, #8]
    dsb     ishst

    // now loading the page tables
    msr     TTBR1_EL1, x0       // load page table!
    msr     TTBR0_EL1, x0       // load the identical page table for ttbr0!
    isb

    // we just loaded the same page table in ttbr0. because even though our final objective is for kernel to 
    // work in ttbr1 virtual address space, right now once we turn mmu on, the PC is STILL at the old physical address. 
    // so we need to load the same page table in ttbr0 so that the current physical address is still valid after mmu is turned on. 
    // then we can do a really huge jump to the correct place in ttbr1 virtual address space after mmu is turned on.

    // turning on the MMU
    mrs     x0, SCTLR_EL1
    ldr     x1, =SCTLR_EL1_ENABLE_MMU
    and     x1, x1, x7          // to physical address
    ldr     x1, [x1]
    orr     x0, x0, x1
    msr     SCTLR_EL1, x0
    isb

    // now, accoeding to our mmu design, kernel is supposed to be at 0xffffff8000080000.
    // ttbr0 is for user space we just set it right now to facilitate what we're about to do.
    // we'll now jump to the correct place!

    adr x16, transition_to_el1
    br  x16 

transition_to_el1:

    // checking current exception level
    mrs     x0, CurrentEL
    cmp     x0, #0x8 // EL2?
    b.ne    el1_start
    
    // EL stack pointer
    ldr     x0, =_stack_top
    msr     SP_EL1, x0
    
    mov     x0, #(1 << 31)      // bit 31 selects Aarch32/64
    msr     HCR_EL2, x0         // HCR_EL2 is like "a rulebook that EL2 writes for EL1"

    // set SPSR_EL2 to EL1h, basically once we ERET, we must switch to EL1h mode
    mov     x0, #(0b00101)      // EL1h 
    orr     x0, x0, #(0b1111 << 6) // mask all exceptions (D, A, I, F)
    msr     SPSR_EL2, x0

    // finally set the ERET PC
    adr     x0, el1_start
    msr     ELR_EL2, x0

    eret // insane

el1_start:

    // set stack pointer
    ldr     x0, =_stack_top
    mov     sp, x0

    // load exception vector table for EL1
    ldr     x0, =el1_vectors
    msr     VBAR_EL1, x0
    isb

    // diables fp/simd interrupts 
    mrs     x0, CPACR_EL1
    orr     x0, x0, #(3 << 20)
    msr     CPACR_EL1, x0
    isb

    ldr     x16, =_rust_main // without this roundabout way, it would do a relative jump, allegedly.
    br      x16                  // Absolute jump directly into higher-half TTBR1 space!