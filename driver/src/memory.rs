use core::{
    ffi::c_void,
    ptr::{null, null_mut},
};

use wdk::println;
use wdk_sys::{
    DISPATCH_LEVEL, NTSTATUS, PHYSICAL_ADDRESS, PVOID, SIZE_T, STATUS_INSUFFICIENT_RESOURCES,
    STATUS_INVALID_LEVEL, STATUS_SUCCESS, STATUS_UNSUCCESSFUL, ULONG,
    ntddk::{
        ExFreePool, KeGetCurrentIrql, MmAllocateContiguousMemory, MmFreeContiguousMemory,
        MmFreeNonCachedMemory, MmGetPhysicalAddress, MmGetVirtualForPhysical,
    },
};

use crate::{
    memory::AllocationType::Freed,
    processor::{Ia32VmxBasicMsr, MSR_IA32_VMX_BASIC, rdmsr, vmptrld, vmx_on},
};

const VMX_ON_SIZE: usize = 4096;
const ALIGNMENT_PAGE_SIZE: usize = 4096;

/// Safe RAII wrapper around the raw API, storing a pointer to the `T` and the length
/// of the allocation in BYTES.
///
/// This must be memory allocated that can be freed with `MmFreeContiguousMemory`.
#[derive(Default)]
pub struct VmxAllocatedRegion {
    p_raw_data: *mut c_void,
    p_aligned_virtual_buffer: *mut c_void,
    p_aligned_physical_buffer: u64,
    /// In BYTES
    len: usize,
}

impl VmxAllocatedRegion {
    pub fn new(num_bytes: SIZE_T, highest_acceptable_addr: PHYSICAL_ADDRESS) -> Option<Self> {
        unsafe {
            let ptr = MmAllocateContiguousMemory(num_bytes, highest_acceptable_addr);
            if ptr.is_null() {
                return None;
            }

            let mut this = Self {
                p_raw_data: ptr,
                len: num_bytes as usize,
                p_aligned_virtual_buffer: null_mut(),
                p_aligned_physical_buffer: 0,
            };

            let physical_buffer = virtual_to_physical_address(ptr);

            //
            // Zero out memory
            //
            core::ptr::write_bytes(ptr as *mut u8, 0, this.len);
            let aligned_physical_buffer = (((physical_buffer as usize + ALIGNMENT_PAGE_SIZE) - 1)
                & !(ALIGNMENT_PAGE_SIZE - 1)) as u64;

            let aligned_virtual_buffer =
                (((this.p_raw_data as *mut u8).add(ALIGNMENT_PAGE_SIZE) as usize - 1)
                    & !(ALIGNMENT_PAGE_SIZE - 1)) as *mut c_void;

            println!(
                "[+] Virtual allocated buffer for VMXON at {:p}",
                this.p_raw_data
            );
            println!(
                "[+] Virtual aligned allocated buffer for VMXON at {:p}",
                aligned_virtual_buffer
            );
            println!(
                "[+] Aligned physical buffer allocated for VMXON at {:p}",
                aligned_physical_buffer as *const c_void
            );

            this.p_aligned_physical_buffer = aligned_physical_buffer;
            this.p_aligned_virtual_buffer = aligned_virtual_buffer;

            Some(this)
        }
    }
}

impl Drop for VmxAllocatedRegion {
    fn drop(&mut self) {
        if !self.p_raw_data.is_null() {
            unsafe { MmFreeContiguousMemory(self.p_raw_data) };
        }
    }
}

pub fn virtual_to_physical_address(p_va: PVOID) -> PVOID {
    unsafe { MmGetPhysicalAddress(p_va).QuadPart as _ }
}

pub fn physical_to_virtual_address(p_addr: PVOID) -> PVOID {
    let mut physical = PHYSICAL_ADDRESS::default();
    physical.QuadPart = p_addr as _;

    unsafe { MmGetVirtualForPhysical(physical) }
}

/// VMX resources owned by a single logical processor.
///
/// Each logical processor participating in VMX operation requires its own
/// VMXON region and VMCS region. The regions contain both their allocated
/// virtual-memory mappings and the physical addresses required by VMX
/// instructions.
#[derive(Default)]
pub struct VirtualMachineState {
    /// Page-aligned VMXON region used to enter VMX operation.
    ///
    /// Its **physical address** is supplied to `VMXON`.
    vmxon_region: VmxAllocatedRegion,
    /// Page-aligned Virtual-Machine Control Structure (VMCS) region.
    ///
    /// Its **physical address** is supplied to instructions such as `VMCLEAR`
    /// and `VMPTRLD`. Once loaded, its fields are accessed through `VMREAD`
    /// and `VMWRITE`.
    vmcs_region: VmxAllocatedRegion,
    /// Extended-Page-Table Pointer
    // TODO do I actually free this o.0
    eptp: *const u64,
    /// Stack for VMM in VM-Exit State
    vmm_stack: ManagedPtr,
    /// MSR Bitmap Virtual Address
    msr_bitmap: ManagedPtr,
    /// MSR Bitmap Virtual Address
    msr_bitmap_physical: *const u64,
}

#[derive(Debug, Default)]
pub enum AllocationType {
    #[default]
    Pool,
    /// Memory allocated that is cached memory must also define the size in bytes of
    /// the allocated region.
    NonCachedMemory(u64),
    /// Memory which has been freed must be marked as freed
    Freed,
}

#[derive(Default, Debug)]
struct ManagedPtr {
    address: *mut u64,
    allocation_type: AllocationType,
}

impl Drop for ManagedPtr {
    fn drop(&mut self) {
        if self.address.is_null() {
            return;
        }

        match self.allocation_type {
            AllocationType::Pool => unsafe {
                ExFreePool(self.address as _);
            },
            AllocationType::NonCachedMemory(sz) => unsafe {
                MmFreeNonCachedMemory(self.address as _, sz);
            },
            Freed => return,
        }

        self.allocation_type = Freed;
        self.address = null_mut();
    }
}

impl VirtualMachineState {
    pub fn set_vmm_stack_address(&mut self, addr: *mut u64, allocation_type: AllocationType) {
        self.vmm_stack.address = addr;
        self.vmm_stack.allocation_type = allocation_type;
    }

    pub fn get_vmm_stack_address(&self) -> Option<*mut u64> {
        if !self.vmm_stack.address.is_null() {
            Some(self.vmm_stack.address)
        } else {
            None
        }
    }

    pub fn set_msr_bitmap(&mut self, addr: *mut u64, allocation_type: AllocationType) {
        self.msr_bitmap.address = addr;
        self.msr_bitmap.allocation_type = allocation_type;
    }

    pub fn get_msr_bitmap(&self) -> Option<*mut u64> {
        if !self.msr_bitmap.address.is_null() {
            Some(self.msr_bitmap.address)
        } else {
            None
        }
    }

    pub fn set_msr_physical(&mut self, addr: *mut u64) {
        self.msr_bitmap_physical = addr
    }

    pub fn get_physical_address_vmxon_region(&self) -> u64 {
        self.vmxon_region.p_aligned_physical_buffer
    }

    pub fn get_physical_address_vmcs_region(&self) -> u64 {
        self.vmcs_region.p_aligned_physical_buffer
    }

    pub fn get_virtual_address_vmxon_region(&self) -> *mut c_void {
        self.vmxon_region.p_aligned_virtual_buffer
    }

    pub fn get_virtual_address_vmcs_region(&self) -> *mut c_void {
        self.vmcs_region.p_aligned_virtual_buffer
    }
}

pub enum VmxRegionTypeForAllocation {
    VmxOn,
    Vmcs,
}

pub fn allocate_vmx_region(
    guest_state: &mut VirtualMachineState,
    phase: VmxRegionTypeForAllocation,
) -> NTSTATUS {
    unsafe {
        if KeGetCurrentIrql() > DISPATCH_LEVEL as _ {
            return STATUS_INVALID_LEVEL;
        }

        let mut physical_max = PHYSICAL_ADDRESS::default();
        physical_max.QuadPart = ULONG::MAX as _;

        let vmx_on_size = 2 * VMX_ON_SIZE;
        // allocates a range of **contiguous**, nonpaged physical memory and maps it to
        // the system address space
        // TODO docs indicate MmAllocateContiguousMemoryStruct should be done at driver initialisation,
        // not on demand after load. Move it into a global?
        let Some(mut vmx_allocation_region) =
            VmxAllocatedRegion::new((vmx_on_size + ALIGNMENT_PAGE_SIZE) as _, physical_max)
        else {
            return STATUS_INSUFFICIENT_RESOURCES;
        };

        //
        // Update the revision identifier
        // From Intel’s manual (24.11.5 VMXON Region ):
        // Before executing VMXON, software should write the VMCS revision identifier to the VMXON region.
        // (Specifically, it should write the 31-bit VMCS revision identifier to bits 30:0 of the first 4 bytes of
        // the VMXON region; bit 31 should be cleared to 0.)
        // It need not initialize the VMXON region in any other way. Software should use a separate region for
        // each logical processor and should not access or modify the VMXON region of a logical processor
        // between the execution of VMXON and VMXOFF on that logical processor. Doing otherwise may lead
        // to unpredictable behavior.
        //
        let vmx_basic_msr = Ia32VmxBasicMsr::new(rdmsr(MSR_IA32_VMX_BASIC));
        println!("[i] MSR_IA32_VMX_BASIC: {vmx_basic_msr:?}");

        core::ptr::write(
            vmx_allocation_region.p_aligned_virtual_buffer as _,
            vmx_basic_msr.revision_identifier,
        );

        match phase {
            VmxRegionTypeForAllocation::VmxOn => {
                if vmx_on(&raw mut vmx_allocation_region.p_aligned_physical_buffer as _).is_err() {
                    return STATUS_UNSUCCESSFUL;
                }

                println!("[+] vmxon instruction executed successfully");
                guest_state.vmxon_region = vmx_allocation_region;
            }
            VmxRegionTypeForAllocation::Vmcs => {
                if vmptrld(&raw mut vmx_allocation_region.p_aligned_physical_buffer as _).is_err() {
                    return STATUS_UNSUCCESSFUL;
                }

                println!("[+] vmxon instruction executed successfully");
                guest_state.vmcs_region = vmx_allocation_region;
            }
        }
    }

    STATUS_SUCCESS
}
