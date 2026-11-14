use core::arch::asm;

use wdk::println;
use wdk_sys::{
    BOOLEAN, FALSE,
    ntddk::{KeQueryActiveProcessors, KeSetSystemAffinityThread},
};

const MSR_IA32_FEATURE_CONTROL: i32 = 0x03A;

pub fn run_on_each_logical_processor() {
    unsafe {
        let max = KeQueryActiveProcessors();

        for i in 0..max {
            let affinity_mask = 2 ^ i;
            KeSetSystemAffinityThread(affinity_mask);

            println!("==================================================");
            println!("[i] Current thread is executing in {i}th logical processor.");

            // run code here
        }
    }
}

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

fn wrmsr(msr: u32, value: u64) {
    let lo: u32 = value as u32;
    let hi: u32 = (value >> 32) as u32;

    unsafe {
        asm!("wrmsr",
             in("rcx") msr, in("rax") lo, in("rdx") hi,
             options(nostack));
    }
}

fn rdmsr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;

    unsafe {
        asm!("rdmsr",
             in("rcx") msr, out("rax") lo, out("rdx") hi,
             options(nostack));
    }

    let mut msr_read = ((hi as u64) << 32) | lo as u64;

    println!("[i] MSR read result: {msr_read:b}");

    msr_read
}

pub fn is_vmx_supported() -> BOOLEAN {
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

    if vmx == 0 {
        return FALSE as _;
    }

    println!("[i] VMX Support: {vmx}");

    let mut msr_read = rdmsr(MSR_IA32_FEATURE_CONTROL as _);

    /*
    typedef union _IA32_FEATURE_CONTROL_MSR
    {
        ULONG64 All;
        struct
        {
            ULONG64 Lock : 1;               // [0]
            ULONG64 EnableSMX : 1;          // [1]
            ULONG64 EnableVmxon : 1;        // [2]
            ULONG64 Reserved2 : 5;          // [3-7]
            ULONG64 EnableLocalSENTER : 7;  // [8-14]
            ULONG64 EnableGlobalSENTER : 1; // [15]
            ULONG64 Reserved3a : 16;        //
            ULONG64 Reserved3b : 32;        // [16-63]
        } Fields;
    } IA32_FEATURE_CONTROL_MSR, *PIA32_FEATURE_CONTROL_MSR;
    */

    // if (Control.Fields.Lock == 0)
    if msr_read & 0b1 == 0 {
        msr_read |= 0b1;
        msr_read |= 0b100;
        wrmsr(MSR_IA32_FEATURE_CONTROL as _, msr_read);
        println!("[+] MSR written");
    // else if (Control.Fields.EnableVmxon == FALSE)
    } else if msr_read & 0b100 == 0 {
        println!("[i] VMX locked off in BIOS.");
        return FALSE as _;
    }

    1
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
