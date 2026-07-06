.section ".text.vectors"
.global el1_vectors

.equ E_SYNC,   0 // to tell rust handler what the exception is
.equ E_IRQ,    1
.equ E_FIQ,    2
.equ E_SERROR, 3

.equ FROM_EL1t, 0
.equ FROM_EL1h, 1
.equ FROM_EL064, 2
.equ FROM_EL032, 3

// VECTOR TABLE FOR EXCEPTIONS AT EL1

.align 11              // 2^11 = 2048 alignment

el1_vectors:

b   el11t_sync
.space 124
b   el11t_irq
.space 124
b   el11t_fiq
.space 124
b   el11t_serror
.space 124

b   el11h_sync
.space 124
b   el11h_irq
.space 124
b   el11h_fiq
.space 124
b   el11h_serror
.space 124

b   el1064_sync
.space 124
b   el1064_irq
.space 124
b   el1064_fiq
.space 124
b   el1064_serror
.space 124

b   el1032_sync
.space 124
b   el1032_irq
.space 124
b   el1032_fiq
.space 124
b   el1032_serror
.space 124


// HANDLERS FOR EL1 EXCEPTIONS

.macro SAVE_REG
    stp     x0,  x1,  [sp, #0x08]
    stp     x2,  x3,  [sp, #0x18]
    stp     x4,  x5,  [sp, #0x28]
    stp     x6,  x7,  [sp, #0x38]
    stp     x8,  x9,  [sp, #0x48]
    stp     x10, x11, [sp, #0x58]
    stp     x12, x13, [sp, #0x68]
    stp     x14, x15, [sp, #0x78]
    stp     x16, x17, [sp, #0x88]
    stp     x18, x19, [sp, #0x98]
    stp     x20, x21, [sp, #0xA8]
    stp     x22, x23, [sp, #0xB8]
    stp     x24, x25, [sp, #0xC8]
    stp     x26, x27, [sp, #0xD8]
    stp     x28, x29, [sp, #0xE8]
    str     x30,      [sp, #0xF8]
    
    mrs     x0, ELR_EL1
    str     x0, [sp, #0x100]

    mrs     x0, SPSR_EL1
    str     x0, [sp, #0x108]

    mrs     x0, ESR_EL1
    str     x0, [sp, #0x110]

    mrs     x0, FAR_EL1
    str     x0, [sp, #0x118]

    mrs     x0, SP_EL0
    str     x0, [sp, #0x120]

    mrs     x0, TTBR0_EL1
    str     x0, [sp, #0x128]
.endm

.macro LOAD_REG
    // load these first so x1 won't be needed after
    ldr     x1, [sp, #0x100]
    msr     ELR_EL1, x1

    ldr     x1, [sp, #0x108]
    msr     SPSR_EL1, x1

    ldr     x1, [sp, #0x120]
    msr     SP_EL0, x1

    ldr     x1, [sp, #0x128]
    msr     TTBR0_EL1, x1
    // we do not need to load back the other two registers

    ldp     x0,  x1,  [sp, #0x08]
    ldp     x2,  x3,  [sp, #0x18]
    ldp     x4,  x5,  [sp, #0x28]
    ldp     x6,  x7,  [sp, #0x38]
    ldp     x8,  x9,  [sp, #0x48]
    ldp     x10, x11, [sp, #0x58]
    ldp     x12, x13, [sp, #0x68]
    ldp     x14, x15, [sp, #0x78]
    ldp     x16, x17, [sp, #0x88]
    ldp     x18, x19, [sp, #0x98]
    ldp     x20, x21, [sp, #0xA8]
    ldp     x22, x23, [sp, #0xB8]
    ldp     x24, x25, [sp, #0xC8]
    ldp     x26, x27, [sp, #0xD8]
    ldp     x28, x29, [sp, #0xE8]
    ldr     x30,      [sp, #0xF8]
.endm

// handling them
.macro SET_EXCEPTION_ARG type source
    mov     w9, #\type
    strb    w9, [sp]
    mov     w9, #\source
    strb    w9, [sp, #1]
    mov     x0, sp
.endm

// \TODO it is inefficient to reset tlb cache after every exception. better to do it in rust only at context switches.
.macro RESET_TLB_CACHE
    dsb     ish
    tlbi    vmalle1
    dsb     ish
    isb
.endm 

.macro HANDLE_EXCEPTION type source
    sub     sp, sp, #0x130 // allocating space for etype + esource + gprs + 6 u64 reg
    // make sure sp is aigned to 16 bytes for rust handler according to arm standard

    // save registers
    SAVE_REG

    // call rust handler with correct arg
    SET_EXCEPTION_ARG \type \source
    bl      handle_exception_el1

    // load back the registers
    LOAD_REG

    // the translation table may have changed
    RESET_TLB_CACHE

    add     sp, sp, #0x130 // restore sp

    eret // handling completed :)
.endm

el11t_sync:
    HANDLE_EXCEPTION E_SYNC FROM_EL1t
    
el11t_irq:
    HANDLE_EXCEPTION E_IRQ FROM_EL1t

el11t_fiq:
    HANDLE_EXCEPTION E_FIQ FROM_EL1t

el11t_serror:
    HANDLE_EXCEPTION E_SERROR FROM_EL1t


el11h_sync:
    HANDLE_EXCEPTION E_SYNC FROM_EL1h

el11h_irq:
    HANDLE_EXCEPTION E_IRQ FROM_EL1h

el11h_fiq:
    HANDLE_EXCEPTION E_FIQ FROM_EL1h

el11h_serror:
    HANDLE_EXCEPTION E_SERROR FROM_EL1h


el1064_sync:
    HANDLE_EXCEPTION E_SYNC FROM_EL064

el1064_irq:
    HANDLE_EXCEPTION E_IRQ FROM_EL064

el1064_fiq:
    HANDLE_EXCEPTION E_FIQ FROM_EL064

el1064_serror:
    HANDLE_EXCEPTION E_SERROR FROM_EL064


el1032_sync:
    HANDLE_EXCEPTION E_SYNC FROM_EL032

el1032_irq:
    HANDLE_EXCEPTION E_IRQ FROM_EL032

el1032_fiq:
    HANDLE_EXCEPTION E_FIQ FROM_EL032

el1032_serror:
    HANDLE_EXCEPTION E_SERROR FROM_EL032
