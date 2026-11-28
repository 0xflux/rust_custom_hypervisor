use core::{
    arch::{asm, naked_asm},
    ffi::c_void,
    ptr::null_mut,
    sync::atomic::Ordering::SeqCst,
};

use alloc::{boxed::Box, format, vec::Vec};
use anyhow::{Result, ensure};
use wdk::{paged_code, println};
use wdk_sys::{
    _POOL_TYPE::NonPagedPool,
    KAFFINITY, NTSTATUS, PAGE_SIZE, STATUS_INVALID_PARAMETER, STATUS_MEMORY_NOT_ALLOCATED,
    STATUS_NOT_SUPPORTED, STATUS_REQUEST_ABORTED, STATUS_SUCCESS, STATUS_UNSUCCESSFUL,
    ntddk::{
        DbgBreakPointWithStatus, ExAllocatePool2, KeQueryActiveProcessorCount,
        KeSetSystemAffinityThread, MmAllocateNonCachedMemory,
    },
};

use crate::{
    POOL_TAG_HYPERVISOR,
    error::HypervisorError::{self, InvalidPtr},
    globals::GLOBALS,
    memory::{
        AllocationType, VirtualMachineState, VmxRegionTypeForAllocation, allocate_vmx_region,
        virtual_to_physical_address,
    },
    processor::{
        clear_vmcs_state, enable_vmx_operation, is_vmx_supported, load_vmcs, print_cpu_id,
        vm_launch,
    },
};

static mut G_STACK_POINTER_FOR_RETURNING: u64 = 0;
static mut G_BASE_POINTER_FOR_RETURNING: u64 = 0;

const VMM_STACK_SIZE: u64 = 0x8000;

//
// Primary processor-based VM execution controls
//
const CPU_BASED_VIRTUAL_INTR_PENDING: u32 = 0x00000004;
const CPU_BASED_USE_TSC_OFFSETING: u32 = 0x00000008;
const CPU_BASED_HLT_EXITING: u32 = 0x00000080;
const CPU_BASED_INVLPG_EXITING: u32 = 0x00000200;
const CPU_BASED_MWAIT_EXITING: u32 = 0x00000400;
const CPU_BASED_RDPMC_EXITING: u32 = 0x00000800;
const CPU_BASED_RDTSC_EXITING: u32 = 0x00001000;
const CPU_BASED_CR3_LOAD_EXITING: u32 = 0x00008000;
const CPU_BASED_CR3_STORE_EXITING: u32 = 0x00010000;
const CPU_BASED_CR8_LOAD_EXITING: u32 = 0x00080000;
const CPU_BASED_CR8_STORE_EXITING: u32 = 0x00100000;
const CPU_BASED_TPR_SHADOW: u32 = 0x00200000;
const CPU_BASED_VIRTUAL_NMI_PENDING: u32 = 0x00400000;
const CPU_BASED_MOV_DR_EXITING: u32 = 0x00800000;
const CPU_BASED_UNCOND_IO_EXITING: u32 = 0x01000000;
const CPU_BASED_ACTIVATE_IO_BITMAP: u32 = 0x02000000;
const CPU_BASED_MONITOR_TRAP_FLAG: u32 = 0x08000000;
const CPU_BASED_ACTIVATE_MSR_BITMAP: u32 = 0x10000000;
const CPU_BASED_MONITOR_EXITING: u32 = 0x20000000;
const CPU_BASED_PAUSE_EXITING: u32 = 0x40000000;
const CPU_BASED_ACTIVATE_SECONDARY_CONTROLS: u32 = 0x80000000;

//
// Secondary processor-based VM execution controls
//
const CPU_BASED_CTL2_ENABLE_EPT: u32 = 0x2;
const CPU_BASED_CTL2_RDTSCP: u32 = 0x8;
const CPU_BASED_CTL2_ENABLE_VPID: u32 = 0x20;
const CPU_BASED_CTL2_UNRESTRICTED_GUEST: u32 = 0x80;
const CPU_BASED_CTL2_ENABLE_VMFUNC: u32 = 0x2000;

//
// VM-entry Control Bits
// The VM-entry controls constitute a 32-bit vector that governs the basic operation of VM entries
//
const VM_ENTRY_IA32E_MODE: u32 = 0x00000200;
const VM_ENTRY_SMM: u32 = 0x00000400;
const VM_ENTRY_DEACT_DUAL_MONITOR: u32 = 0x00000800;
const VM_ENTRY_LOAD_GUEST_PAT: u32 = 0x00004000;

//
// VM-exit Control Bits
//
const VM_EXIT_IA32E_MODE: u32 = 0x00000200;
const VM_EXIT_ACK_INTR_ON_EXIT: u32 = 0x00008000;
const VM_EXIT_SAVE_GUEST_PAT: u32 = 0x00040000;
const VM_EXIT_LOAD_HOST_PAT: u32 = 0x00080000;

//
// PIN-Based Execution Control
//
const PIN_BASED_VM_EXECUTION_CONTROLS_EXTERNAL_INTERRUPT: u32 = 0x00000001;
const PIN_BASED_VM_EXECUTION_CONTROLS_NMI_EXITING: u32 = 0x00000008;
const PIN_BASED_VM_EXECUTION_CONTROLS_VIRTUAL_NMI: u32 = 0x00000020;
const PIN_BASED_VM_EXECUTION_CONTROLS_ACTIVE_VMX_TIMER: u32 = 0x00000040;
const PIN_BASED_VM_EXECUTION_CONTROLS_PROCESS_POSTED_INTERRUPTS: u32 = 0x00000080;

#[unsafe(naked)]
extern "C" fn asm_save_state_for_vmx() {
    naked_asm!(
        "mov [rip + {stack}], rsp",
        "mov [rip + {base}], rbp",
        "ret",
        stack = sym G_STACK_POINTER_FOR_RETURNING,
        base = sym G_BASE_POINTER_FOR_RETURNING,
    )
}

#[unsafe(naked)]
extern "C" fn asm_vmx_off_and_restore_state() {
    naked_asm!(
        "VMXOFF",
        "mov rsp, [rip + {stack}]",
        "mov rbp, [rip + {base}]",
        // make rsp point to a correct return point
        "add rsp, 8",
        // return true
        "xor rax, rax",
        "mov rax, 1",
        // return section
        // TODO this must match the disasm of the end of LaunchVm fn
        // which may be .. challenging with the compiler
        "mov rbx, [rsp + 28h + 8h]",
        "mov rsi, [rsp + 28h + 10h]",
        "add rsp, 020h",
        "pop rdi",
        "ret",
        stack = sym G_STACK_POINTER_FOR_RETURNING,
        base = sym G_BASE_POINTER_FOR_RETURNING,
    )
}

#[unsafe(naked)]
extern "C" fn get_gdt_base() -> u64 {
    naked_asm!(
        // we can use the shadow space for this..
        "sgdt [rsp + 8]",
        // + 10 decimal not hex..
        "mov rax, qword ptr [rsp + 10]",
        "ret",
    )
}

#[unsafe(naked)]
extern "C" fn get_cs() -> u64 {
    naked_asm!("mov rax, cs", "ret")
}

#[unsafe(naked)]
extern "C" fn get_ds() -> u64 {
    naked_asm!("mov rax, ds", "ret")
}

#[unsafe(naked)]
extern "C" fn get_es() -> u64 {
    naked_asm!("mov rax, es", "ret")
}

#[unsafe(naked)]
extern "C" fn get_ss() -> u64 {
    naked_asm!("mov rax, ss", "ret")
}

#[unsafe(naked)]
extern "C" fn get_fs() -> u64 {
    naked_asm!("mov rax, fs", "ret")
}

#[unsafe(naked)]
extern "C" fn get_gs() -> u64 {
    naked_asm!("mov rax, gs", "ret")
}

#[unsafe(naked)]
extern "C" fn get_ldtr() -> u64 {
    naked_asm!("sldt rax", "ret")
}

#[unsafe(naked)]
extern "C" fn get_tr() -> u64 {
    naked_asm!("str rax", "ret")
}

#[unsafe(naked)]
extern "C" fn get_idt_base() -> u64 {
    naked_asm!(
        // we can use the shadow space for this..
        "sidt [rsp + 8]",
        // + 10 decimal not hex..
        "mov rax, qword ptr [rsp + 10]",
        "ret",
    )
}

#[unsafe(naked)]
extern "C" fn get_gdt_limit() -> u64 {
    naked_asm!(
        // we can use the shadow space for this..
        "sgdt [rsp + 8]",
        "mov ax, word ptr [rsp + 8]",
        "ret",
    )
}

#[unsafe(naked)]
extern "C" fn get_idt_limit() -> u64 {
    naked_asm!(
        // we can use the shadow space for this..
        "sidt [rsp + 8]",
        "mov ax, word ptr [rsp + 8]",
        "ret",
    )
}

#[unsafe(naked)]
extern "C" fn get_rflags() -> u64 {
    naked_asm!(
        // we can use the shadow space for this..
        "pushfq", "pop rax", "ret",
    )
}

pub fn start_vmx() -> NTSTATUS {
    paged_code!();

    print_cpu_id();

    if is_vmx_supported() != 1 {
        println!("[-] VMX not supported");
        return STATUS_NOT_SUPPORTED;
    }

    let processor_count = unsafe { KeQueryActiveProcessorCount(null_mut()) };
    println!("[i] Processor count: {processor_count}");

    if processor_count == 0 {
        println!("[-] Invalid processor count, got 0.");
        return STATUS_UNSUCCESSFUL;
    }

    //
    // Each processor needs its own region, reserve memory for `processor_count` slots,
    // this can then be stored into the globals
    //
    let mut porocessor_regions: Vec<VirtualMachineState> =
        Vec::with_capacity(processor_count as usize);
    unsafe { porocessor_regions.set_len(processor_count as _) };

    for i in 0..processor_count {
        let affinity_mask = 2u64.pow(i);
        unsafe { KeSetSystemAffinityThread(affinity_mask) };

        println!("==============================================================");
        println!("[i] Current thread is executing in {i}th logical processor.");

        enable_vmx_operation();
        println!("[+] VMX Operation enabled.");

        let Some(vm_state) = porocessor_regions.get_mut(i as usize) else {
            println!("[-] Processor region size mismatch. Something went wrong...?");
            return STATUS_UNSUCCESSFUL;
        };

        allocate_vmx_region(vm_state, VmxRegionTypeForAllocation::VmxOn);
        allocate_vmx_region(vm_state, VmxRegionTypeForAllocation::Vmcs);

        println!(
            "[i] VMCS Region is allocated at (virtual) {:p}, (physical) {:p}",
            vm_state.get_virtual_address_vmcs_region(),
            vm_state.get_physical_address_vmcs_region() as *const c_void,
        );
        println!(
            "[i] VMXON Region is allocated at (virtual) {:p}, (physical) {:p}",
            vm_state.get_virtual_address_vmxon_region(),
            vm_state.get_physical_address_vmxon_region() as *const c_void,
        );
    }

    //
    // Only store the globals once we are set up correctly; raii should manage the failure state
    // on return if anything above went wrong
    //
    GLOBALS.processor_counts.store(processor_count, SeqCst);

    let p_porocessor_regions = Box::into_raw(Box::new(porocessor_regions));
    GLOBALS.guest_state.store(p_porocessor_regions, SeqCst);

    GLOBALS.vmx_enabled.store(true, SeqCst);

    STATUS_SUCCESS
}

pub fn stop_vmx() {
    println!("[i] Stopping VMX..");

    if !GLOBALS.vmx_enabled.load(SeqCst) {
        println!("[-] VMX was not enabled.");
        return;
    }

    let processor_count = GLOBALS.processor_counts.load(SeqCst);

    for i in 0..processor_count {
        let affinity_mask = 2u64.pow(i);
        unsafe { KeSetSystemAffinityThread(affinity_mask) };

        println!("==============================================================");
        println!("[i] Current thread is executing in {i}th logical processor.");

        unsafe {
            asm!("vmxoff");
        };
    }

    {
        let ptr = GLOBALS.guest_state.load(SeqCst);
        if !ptr.is_null() {
            drop(unsafe { Box::from_raw(ptr) });
            GLOBALS.guest_state.store(null_mut(), SeqCst);
        }
    }

    GLOBALS.vmx_enabled.store(false, SeqCst);
}

// TODO thread safety mut access for guest_state[i]?
pub fn launch_vm(processor_id: u32, p_eptp: *const c_void) -> NTSTATUS {
    println!("================================================");
    println!("Launching VM!");

    let affinity_mask: KAFFINITY = 2u64.pow(processor_id);
    unsafe { KeSetSystemAffinityThread(affinity_mask) };

    paged_code!();

    //
    // Allocate stack for the VM Exit handler
    //
    let vmm_stack_va =
        unsafe { ExAllocatePool2(NonPagedPool as _, VMM_STACK_SIZE, POOL_TAG_HYPERVISOR) };
    if vmm_stack_va.is_null() {
        println!("[-] Failed to allocate vmm stack.");
        return STATUS_MEMORY_NOT_ALLOCATED;
    }

    let state = GLOBALS.guest_state.load(SeqCst);
    if state.is_null() {
        println!("[-] Failed to get guest state");
        return STATUS_INVALID_PARAMETER;
    }

    let Some(entry) = unsafe { &mut *state }.get_mut(processor_id as usize) else {
        return STATUS_INVALID_PARAMETER;
    };

    // Memory is already zeroed, no need to repeat
    entry.set_vmm_stack_address(vmm_stack_va as _, AllocationType::Pool);

    //
    // Allocate memory for the MSR bitmap
    //

    // Note, in case of BSODs in the future, this is likely not contiguous - the blog uses this fn so
    // lets go with it, but we are getting its physical address which makes me think maybe this should
    // be contiguous?
    let p_bitmap = unsafe { MmAllocateNonCachedMemory(PAGE_SIZE as _) };
    if p_bitmap.is_null() {
        println!("[-] Failed to allocate msr bitmap");
        return STATUS_INVALID_PARAMETER;
    }

    // Zero init the memory. The rust wdk does not have RtlZeroMemory :'(
    unsafe { core::ptr::write_bytes(p_bitmap, 0u8, PAGE_SIZE as _) };
    entry.set_msr_bitmap(
        p_bitmap as _,
        AllocationType::NonCachedMemory(PAGE_SIZE as u64),
    );

    let physical_msr = virtual_to_physical_address(p_bitmap);
    if physical_msr.is_null() {
        println!("[-] Failed to find physical address of msr bitmap");
        return STATUS_INVALID_PARAMETER;
    }

    entry.set_msr_physical(physical_msr as _);

    //
    // Clear the VMCS state and load VMCS (set the current VMCS)
    //
    if let Err(e) = clear_vmcs_state(entry) {
        println!("[-] Launch vm error: {e:?}");
        return STATUS_UNSUCCESSFUL;
    }
    if let Err(e) = load_vmcs(entry) {
        println!("[-] Load vmcs error: {e:?}");
        return STATUS_UNSUCCESSFUL;
    };

    println!("[i] Setting up VMCS.");
    // TODO setup_vmcs();

    asm_save_state_for_vmx();

    println!("[i] Executing VMLAUNCH");
    vm_launch();

    //
    // If vm_launch failed we should not get here..
    //
    unsafe {
        asm!("VMXOFF");
    }
    unsafe {
        DbgBreakPointWithStatus(STATUS_UNSUCCESSFUL as _);
    }
    return STATUS_REQUEST_ABORTED;
}
