use wdk_sys::{
    DISPATCH_LEVEL, KIRQL, PHYSICAL_ADDRESS, PVOID, ntddk::{KeGetCurrentIrql, MmGetPhysicalAddress, MmGetVirtualForPhysical},
};

unsafe extern "system"
{
	pub fn KeRaiseIrqlToDpcLevel() -> KIRQL;
}

pub fn virtual_to_physical_address(p_va: PVOID) -> PVOID {
    unsafe { return MmGetPhysicalAddress(p_va).QuadPart as _ }
}

pub fn physical_to_virtual_address(p_addr: PVOID) -> PVOID {
    let mut physical = PHYSICAL_ADDRESS::default();
    physical.QuadPart = p_addr as _;

    unsafe { MmGetVirtualForPhysical(physical) }
}

pub struct VirtualMachineState {
    vmxon_region: u64, // VMXON region
    vmcs_region: u64,  // VMCS region
}

pub fn allocate_vmx_region(guest_state: &VirtualMachineState)
{
    unsafe { 
        if KeGetCurrentIrql() > DISPATCH_LEVEL as _ {
            // KeRaiseIrqlToDpcLevel();
            // todo from here...
        }
    }
}