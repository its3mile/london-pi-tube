//! HardFault handler: turn a hard fault into a panic, so it is logged, shown
//! on the display and followed by a restart (see `crate::panic`).
//!
//! The most likely hard fault is a stack overflow. The firmware is linked with
//! flip-link, which places the stack below the statics, so an overflow runs
//! off the start of RAM and faults instead of corrupting statics.
//!
//! cortex-m-rt's default HardFault handler is an infinite loop, which would
//! leave the device hung until someone physically resets it.

use core::arch::global_asm;

// Entry point, called directly from the vector table (cortex-m-rt only
// provides its own `HardFault` if this symbol is not defined).
//
// After a stack overflow the stack pointer is outside RAM, so no Rust code can
// run until it is fixed: the first thing a Rust function does is push to the
// stack. This never returns, so the whole stack can be reclaimed by pointing
// the stack pointer back at the top.
global_asm!(
    ".section .HardFault.user, \"ax\"",
    ".global HardFault",
    ".type HardFault, %function",
    ".thumb_func",
    "HardFault:",
    "mrs r0, msp",           // r0 (first argument) = stack pointer at the time of the fault
    "ldr r1, =_stack_start", // top of the stack
    "msr msp, r1",
    "b {handler}",
    ".pool",
    handler = sym hard_fault,
);

/// Size of the frame the CPU pushes on exception entry (without FPU state).
const EXCEPTION_FRAME_SIZE: u32 = 32;
/// Offset of the faulting program counter within that frame.
const EXCEPTION_FRAME_PC_OFFSET: u32 = 24;
/// Configurable Fault Status Register: which fault occurred.
const CFSR: *const u32 = 0xE000_ED28 as *const u32;

extern "C" fn hard_fault(faulting_sp: u32) -> ! {
    unsafe extern "C" {
        // Linker symbols: their *addresses* are the stack bounds.
        static _stack_start: u32;
        static _stack_end: u32;
    }
    let stack_top = &raw const _stack_start as u32;
    let stack_bottom = &raw const _stack_end as u32;

    // SAFETY: a fixed, always-readable system register.
    let cfsr = unsafe { CFSR.read_volatile() };

    // The exception frame is only readable if it lies within the stack.
    let frame_on_stack = faulting_sp >= stack_bottom
        && faulting_sp.saturating_add(EXCEPTION_FRAME_SIZE) <= stack_top;

    if frame_on_stack {
        // SAFETY: the frame is inside the stack region, checked above.
        let pc =
            unsafe { ((faulting_sp + EXCEPTION_FRAME_PC_OFFSET) as *const u32).read_volatile() };
        panic!("HardFault at pc={:#010x} (cfsr={:#010x})", pc, cfsr);
    } else {
        panic!(
            "HardFault: stack overflow (sp={:#010x}, cfsr={:#010x})",
            faulting_sp, cfsr
        );
    }
}
