use core::{arch::asm};

use wdk::println;
use wdk_sys::BOOLEAN;

pub fn enable_vmx_operation() {
    unsafe {
        asm!(
            "xor rax, rax",
            "mov rax, cr4",
            "or rax, 02000h",
            "mov cr4, rax",
        )
    }

    println!("[+] VMX Operation enabled.");
}

pub fn vmx_support() -> BOOLEAN {
    let mut vmx: i32 = 0;

    unsafe {
        asm!(
            "xor eax, eax",
            "inc eax",
            "cpuid",
            "bt ecx, 5h",
            "jc 4f",
            "jmp 5f",
            "4:",
            "mov {0:e}, 1h",
            "jmp 6f",
            "5:",
            "mov {0:e}, 0",
            "6:",
            "NOP",
            out(reg) vmx,
        )
    }

    println!("[i] VMX Support: {vmx}");

    vmx as _
}

pub fn print_cpu_id() {
    let mut sys_type = [0u8; 13];

    unsafe {
        asm!(
            "xor eax, eax",
            "cpuid",

            // mov ebx to eax and get the chars 1 by 1 using >>
            "mov eax, ebx",
            "mov {}, al",
            "shr eax, 8",
            "mov {}, al",
            "shr eax, 8",
            "mov {}, al",
            "shr eax, 8",
            "mov {}, al",

            // part 2 in edx
            "mov eax, edx",
            "mov {}, al",
            "shr eax, 8",
            "mov {}, al",
            "shr eax, 8",
            "mov {}, al",
            "shr eax, 8",
            "mov {}, al",

            // part 3 in ecx
            "mov eax, ecx",
            "mov {}, al",
            "shr eax, 8",
            "mov {}, al",
            "shr eax, 8",
            "mov {}, al",
            "shr eax, 8",
            "mov {}, al",
            // "mov {}, 0h",

            // -- part 1
            out(reg_byte) sys_type[0],
            out(reg_byte) sys_type[1],
            out(reg_byte) sys_type[2],
            out(reg_byte) sys_type[3],
            // -- part 2
            out(reg_byte) sys_type[4],
            out(reg_byte) sys_type[5],
            out(reg_byte) sys_type[6],
            out(reg_byte) sys_type[7],
            // -- part 3
            out(reg_byte) sys_type[8],
            out(reg_byte) sys_type[9],
            out(reg_byte) sys_type[10],
            out(reg_byte) sys_type[11],
            // // null terminator
            // out(reg_byte) sys_type[12],
        );
    }

    let s = str::from_utf8(&sys_type).unwrap_or("unknown");
    println!("[i] CPU ID: {s:?}");
}
