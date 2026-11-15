use core::{ptr::null_mut, sync::atomic::Ordering::SeqCst};

use alloc::{boxed::Box, vec::Vec};
use wdk::{paged_code, println};
use wdk_sys::{
    NTSTATUS, STATUS_NOT_SUPPORTED, STATUS_SUCCESS, STATUS_UNSUCCESSFUL,
    ntddk::{KeQueryActiveProcessorCount, KeSetSystemAffinityThread},
};

use crate::{
    globals::GLOABLS,
    memory::{
        VirtualMachineState, VmxAllocatedRegion, VmxRegionTypeForAllocation, allocate_vmx_region,
    },
    processor::{enable_vmx_operation, is_vmx_supported, print_cpu_id},
};

pub fn start_vmx() -> NTSTATUS {
    paged_code!();

    print_cpu_id();

    if is_vmx_supported() != 1 {
        println!("[-] VMX not supported");
        return STATUS_NOT_SUPPORTED;
    }

    let processor_count = unsafe { KeQueryActiveProcessorCount(null_mut()) };
    println!("[i] Processor count: {processor_count}");

    //
    // Each processor needs its own region, reserve memory for `processor_count` slots,
    // this can then be stored into the globals
    //
    let mut porocessor_regions: Vec<VirtualMachineState> = Vec::with_capacity(processor_count as _);

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
            "[i] VMCS Region is allocated at {:p}",
            vm_state.vmcs_region.p_aligned_virtual_buffer
        );
        println!(
            "[i] VMXON Region is allocated at {:p}",
            vm_state.vmxon_region.p_aligned_virtual_buffer
        );
    }

    //
    // Only store the globals once we are set up correctly; raii should manage the failure state
    // on return if anything above went wrong
    //
    GLOABLS.processor_counts.store(processor_count, SeqCst);

    let p_porocessor_regions = Box::into_raw(Box::new(porocessor_regions));
    GLOABLS.guest_state.store(p_porocessor_regions, SeqCst);

    STATUS_SUCCESS
}
