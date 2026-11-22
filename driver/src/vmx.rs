use core::{arch::asm, ffi::c_void, ptr::null_mut, sync::atomic::Ordering::SeqCst};

use alloc::{boxed::Box, format, vec::Vec};
use anyhow::{Result, ensure};
use wdk::{paged_code, println};
use wdk_sys::{
    _POOL_TYPE::NonPagedPool,
    KAFFINITY, NTSTATUS, PAGE_SIZE, STATUS_NOT_SUPPORTED, STATUS_SUCCESS, STATUS_UNSUCCESSFUL,
    ntddk::{
        ExAllocatePool2, KeQueryActiveProcessorCount, KeSetSystemAffinityThread,
        MmAllocateNonCachedMemory,
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
    },
};

const VMM_STACK_SIZE: u64 = 0x8000;

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
pub fn launch_vm(processor_id: u32, p_eptp: *const c_void) -> Result<()> {
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
    ensure!(
        !vmm_stack_va.is_null(),
        HypervisorError::AllocationFailed(Some("Failed to allocate vmm stack.".into()))
    );

    let state = GLOBALS.guest_state.load(SeqCst);
    ensure!(
        !state.is_null(),
        HypervisorError::InvalidPtr(Some("Failed to get guest state".into()))
    );

    let entry = unsafe { &mut *state }
        .get_mut(processor_id as usize)
        .ok_or(HypervisorError::NotFound(format!(
            "Index {processor_id} not found"
        )))?;

    // Memory is already zeroed, no need to repeat
    entry.set_vmm_stack_address(vmm_stack_va as _, AllocationType::Pool);

    //
    // Allocate memory for the MSR bitmap
    //

    // Note, in case of BSODs in the future, this is likely not contiguous - the blog uses this fn so
    // lets go with it, but we are getting its physical address which makes me think maybe this should
    // be contiguous?
    let p_bitmap = unsafe { MmAllocateNonCachedMemory(PAGE_SIZE as _) };
    ensure!(
        !p_bitmap.is_null(),
        HypervisorError::AllocationFailed(Some("Failed to allocate msr bitmap.".into()))
    );

    // Zero init the memory. The rust wdk does not have RtlZeroMemory :'(
    unsafe { core::ptr::write_bytes(p_bitmap, 0u8, PAGE_SIZE as _) };
    entry.set_msr_bitmap(
        p_bitmap as _,
        AllocationType::NonCachedMemory(PAGE_SIZE as u64),
    );

    let physical_msr = virtual_to_physical_address(p_bitmap);
    ensure!(
        !physical_msr.is_null(),
        InvalidPtr(Some("Failed to find physical address of msr bitmap".into()))
    );

    entry.set_msr_physical(physical_msr as _);

    //
    // Clear the VMCS state and load VMCS (set the current VMCS)
    //
    clear_vmcs_state(entry)?;
    load_vmcs(entry)?;

    println!("[i] Setting up VMCS.");
    // TODO setup_vmcs();

    Ok(())
}
