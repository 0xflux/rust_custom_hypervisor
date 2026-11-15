use core::{arch::asm, ffi::c_void};

use wdk::println;
use wdk_sys::{BOOLEAN, FALSE};

pub const MSR_IA32_FEATURE_CONTROL: i32 = 0x03A;
pub const MSR_IA32_VMX_BASIC: i32 = 0x480;

#[derive(Debug)]
#[allow(unused)]
pub struct Ia32VmxBasicMsr {
    pub revision_identifier: u32,
    // 12 bits
    pub region_size: u16,
    // represented 1 bit
    pub region_clear: u8,
    // represented 1 bit
    pub supported_ia_64: u8,
    // represented 1 bit
    pub supported_dual_monitor: u8,
    // represented 4 bits
    pub memory_type: u8,
    // represented 1 bit
    pub vm_exit_report: u8,
    // represented 1 bit
    pub vmx_capability_hint: u8,
}

impl Ia32VmxBasicMsr {
    pub fn new(bitfields: u64) -> Self {
        let revision_identifier: u32 = bitfields as u32;
        let mut lower = (bitfields >> 32) as u32;
        let region_size = (lower & 0b111111111111) as u16;
        lower >>= 12;
        let region_clear = (lower & 1) as u8;
        lower >>= 0b1111;
        let supported_ia_64 = (lower & 1) as u8;
        let supported_dual_monitor = (lower & 0b10) as u8;
        let memory_type = (lower & 0b111100) as u8;
        let vm_exit_report = (lower & 0b1000000) as u8;
        let vmx_capability_hint = (lower & 0b10000000) as u8;

        Self {
            revision_identifier,
            region_size,
            region_clear,
            supported_ia_64,
            supported_dual_monitor,
            memory_type,
            vm_exit_report,
            vmx_capability_hint,
        }
    }
}

///  [in] A pointer to a 64-bit, 4KB-aligned physical address that points to a VMXON region.
///
/// The [`__vmx_on`] function corresponds to the `VMXON` machine instruction. This function supports
/// the interaction of a host's virtual machine monitor with a guest operating system and its applications.
///
/// # Returns
/// | Value | Meaning |
/// | ---- | ---- |
/// | 0 | The operation succeeded. |
/// | 1 | The operation failed with extended status available in the VM-instruction error field of the current VMCS. |
/// | 2 | The operation failed without status available. |
pub fn vmx_on(physical_addr: *mut *mut c_void) -> (u8, u8) {
    let rflags: u64;

    unsafe {
        asm!(
            "vmxon [{0}]",
            "pushfq",
            "pop {1}",
            in(reg) physical_addr,
            out(reg) rflags
        );
    };

    (rflags as u8 & 1u8, ((rflags >> 6) & 1) as u8)
}

pub fn vmptrld(physical_address: *mut *mut c_void) -> (u8, u8) {
    let rflags: u64;

    unsafe {
        asm!(
            "vmptrld [{0}]",
            "pushfq",
            "pop {1}",
            in(reg) physical_address,
            out(reg) rflags
        );
    };

    (rflags as u8 & 1u8, ((rflags >> 6) & 1) as u8)
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

pub fn rdmsr(input: i32) -> u64 {
    let lo: u32;
    let hi: u32;

    unsafe {
        asm!("rdmsr",
             in("rcx") input, out("rax") lo, out("rdx") hi,
             options(nostack));
    }

    let msr_read = ((hi as u64) << 32) | lo as u64;

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
