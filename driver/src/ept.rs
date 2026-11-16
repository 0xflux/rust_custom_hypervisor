use core::{ffi::c_void, ptr::null_mut, sync::atomic::Ordering::SeqCst};

use alloc::boxed::Box;
use bitfield::bitfield;
use wdk::{paged_code, println};
use wdk_sys::{
    NTSTATUS, PAGE_SIZE, POOL_FLAG_NON_PAGED, STATUS_INSUFFICIENT_RESOURCES, STATUS_SUCCESS,
    ntddk::{ExAllocatePool2, ExFreePoolWithTag},
};

use crate::{POOL_TAG, globals::GLOBALS, memory::virtual_to_physical_address};

bitfield! {
    #[derive(Clone, Copy, Default)]
    pub struct Eptp(u64);
    impl Debug;

    pub memory_type, set_memory_type: 2, 0;
    pub page_walk_length, set_page_walk_length: 5, 3;
    pub dirty_and_access_enabled, set_dirty_and_access_enabled: 6;
    pub pml4_address, set_pml4_address: 47, 12;
}

//
// Page tables
//

bitfield! {
    #[derive(Clone, Copy, Default)]
    pub struct EptPML4E(u64);
    impl Debug;

    pub read, set_read : 0;
    pub write, set_write : 1;
    pub execute, set_execute : 2;
    pub accessed, set_accessed : 8;
    pub execute_for_user_mode, set_execute_for_user_mode : 10;
    pub physical_address, set_physical_address : 47, 12;
}

bitfield! {
    #[derive(Clone, Copy, Default)]
    pub struct EptPdPte(u64);
    impl Debug;

    pub read, set_read : 0;
    pub write, set_write : 1;
    pub execute, set_execute : 2;
    pub accessed, set_accessed : 8;
    pub execute_for_user_mode, set_execute_for_user_mode : 10;
    pub physical_address, set_physical_address : 47, 12;
}

bitfield! {
    #[derive(Clone, Copy, Default)]
    pub struct EptPde(u64);
    impl Debug;

    pub read, set_read : 0;
    pub write, set_write : 1;
    pub execute, set_execute : 2;
    pub accessed, set_accessed : 8;
    pub execute_for_user_mode, set_execute_for_user_mode : 10;
    pub physical_address, set_physical_address : 47, 12;
}

bitfield! {
    #[derive(Clone, Copy, Default)]
    pub struct EptPte(u64);
    impl Debug;

    pub read, set_read : 0;
    pub write, set_write : 1;
    pub execute, set_execute : 2;
    pub eptmemory_type, set_eptmemory_type : 5, 3;
    pub ignore_pat, set_ignore_pat : 6;
    pub accessed_flag, set_accessed_flag : 8;
    pub dirty_flag, set_dirty_flag : 9;
    pub execute_for_user_mode, set_execute_for_user_mode : 10;
    pub physical_address, set_physical_address : 47, 12;
    pub suppress_ve, set_suppress_ve : 63;
}

#[derive(Debug, Default)]
pub struct EptTables {
    pub p_eptp: *mut Eptp,
    pub p_ept_pml_4: *mut EptPML4E,
    pub p_ept_pdpt: *mut EptPdPte,
    pub p_ept_pd: *mut EptPde,
    pub p_ept_pt: *mut EptPte,
    pub p_guest_memory: *mut c_void,
}

impl Drop for EptTables {
    fn drop(&mut self) {
        if !self.p_eptp.is_null() {
            unsafe {
                ExFreePoolWithTag(self.p_eptp as _, POOL_TAG);
            }
            self.p_eptp = null_mut();
        }

        if !self.p_ept_pml_4.is_null() {
            unsafe {
                ExFreePoolWithTag(self.p_ept_pml_4 as _, POOL_TAG);
            }
            self.p_ept_pml_4 = null_mut();
        }

        if !self.p_ept_pdpt.is_null() {
            unsafe {
                ExFreePoolWithTag(self.p_ept_pdpt as _, POOL_TAG);
            }
            self.p_ept_pdpt = null_mut();
        }

        if !self.p_ept_pd.is_null() {
            unsafe {
                ExFreePoolWithTag(self.p_ept_pd as _, POOL_TAG);
            }
            self.p_ept_pd = null_mut();
        }

        if !self.p_ept_pt.is_null() {
            unsafe {
                ExFreePoolWithTag(self.p_ept_pt as _, POOL_TAG);
            }
            self.p_ept_pt = null_mut();
        }

        if !self.p_guest_memory.is_null() {
            unsafe {
                ExFreePoolWithTag(self.p_guest_memory, POOL_TAG);
            }
            self.p_guest_memory = null_mut();
        }
    }
}

impl EptTables {
    /// Allocates the approproate memory for the EPT, storing the result in
    /// the globals on success. If this failed, memory will be appropriately free'd
    /// and an error status returned
    pub fn new() -> NTSTATUS {
        paged_code!();

        //
        // Note, we are building a `EptPtrs` which, until we store it in the global at the end of the fn,
        // RAII will safely handle cleanup when we return from the fn if we have not given it the global
        // owner. Thus, no need to keep track of what to free and when.
        // Note, the below can probably be made a little more idiomatic.. but its ok for now
        //

        let mut ept_ptrs = EptTables::default();

        // 1 ------------
        let p_eptp = unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED as _, PAGE_SIZE as _, POOL_TAG) };
        if p_eptp.is_null() {
            return STATUS_INSUFFICIENT_RESOURCES;
        }
        ept_ptrs.p_eptp = p_eptp as _;

        // 2 --------------
        let p_ept_pml_4 =
            unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED as _, PAGE_SIZE as _, POOL_TAG) };
        if p_ept_pml_4.is_null() {
            return STATUS_INSUFFICIENT_RESOURCES;
        }
        ept_ptrs.p_ept_pml_4 = p_ept_pml_4 as _;

        // 3 ----------------
        let p_ept_pdpt =
            unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED as _, PAGE_SIZE as _, POOL_TAG) };
        if p_ept_pdpt.is_null() {
            return STATUS_INSUFFICIENT_RESOURCES;
        }
        ept_ptrs.p_ept_pdpt = p_ept_pdpt as _;

        // 4 -----------------
        let p_ept_pd =
            unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED as _, PAGE_SIZE as _, POOL_TAG) };
        if p_ept_pd.is_null() {
            return STATUS_INSUFFICIENT_RESOURCES;
        }
        ept_ptrs.p_ept_pd = p_ept_pd as _;

        // 5 -----------------
        let p_ept_pt =
            unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED as _, PAGE_SIZE as _, POOL_TAG) };
        if p_ept_pt.is_null() {
            return STATUS_INSUFFICIENT_RESOURCES;
        }
        ept_ptrs.p_ept_pt = p_ept_pt as _;

        println!("[+] Allocated pages for EPT");

        //
        // Setup PT by allocating two pages contiguously
        // We allocate two pages because we need 1 page for our RIP to start and 1 page for RSP
        //
        let num_pages = 10;
        let guest_sz_bytes: u64 = (num_pages * PAGE_SIZE) as _;
        let p_guest_memory =
            unsafe { ExAllocatePool2(POOL_FLAG_NON_PAGED as _, guest_sz_bytes, POOL_TAG) };
        if p_guest_memory.is_null() {
            return STATUS_INSUFFICIENT_RESOURCES;
        }
        ept_ptrs.p_guest_memory = p_guest_memory;
        println!("[+] Guest memory allocated");

        // Create 10 EPT mappings
        // When the guest accesses guest-physical page i, redirect that access to the host-physical page backing
        // GuestMemory[i], with read/write/execute permission
        for i in 0..num_pages {
            unsafe {
                // Select EPT leaf entry i
                let entry = ept_ptrs.p_ept_pt.add(i as _);

                // select the 4 KiB guest memory page
                let host_virtual_page =
                    (ept_ptrs.p_guest_memory as *mut u8).add((i * PAGE_SIZE) as _);

                // finmd the host-physical page
                let host_phys_page = virtual_to_physical_address(host_virtual_page as _);
                let page_frame_num = (host_phys_page as u64) / PAGE_SIZE as u64;

                (*entry).set_read(true);
                (*entry).set_write(true);
                (*entry).set_execute(true);
                // Note: EPTMemoryType can be either 0 (for uncached memory) or 6 (writeback) memory,
                // and as we want our memory to be cacheable, so put 6 on it.
                (*entry).set_eptmemory_type(6);
                (*entry).set_physical_address(page_frame_num);
            }
        }

        //
        // Setting up PDE
        // PDE should point to the PTE base address, so we just put the address of the first entry
        // from the EPT PTE as the physical address for Page Directory Entry.
        //
        unsafe {
            (*ept_ptrs.p_ept_pd).set_execute(true);
            (*ept_ptrs.p_ept_pd).set_read(true);
            (*ept_ptrs.p_ept_pd).set_write(true);
            let phys =
                virtual_to_physical_address(ept_ptrs.p_ept_pt as _) as u64 / PAGE_SIZE as u64;
            (*ept_ptrs.p_ept_pd).set_physical_address(phys);
        }

        //
        // Setting up PDPTE
        // Entry should point to the first entry of Page-Directory.
        //
        unsafe {
            (*ept_ptrs.p_ept_pdpt).set_execute(true);
            (*ept_ptrs.p_ept_pdpt).set_read(true);
            (*ept_ptrs.p_ept_pdpt).set_write(true);
            let phys =
                virtual_to_physical_address(ept_ptrs.p_ept_pd as _) as u64 / PAGE_SIZE as u64;
            (*ept_ptrs.p_ept_pdpt).set_physical_address(phys);
        }

        //
        // Setting up PML4E
        // Points to the first entry of the PDPT
        //
        unsafe {
            (*ept_ptrs.p_ept_pml_4).set_execute(true);
            (*ept_ptrs.p_ept_pml_4).set_read(true);
            (*ept_ptrs.p_ept_pml_4).set_write(true);
            let phys =
                virtual_to_physical_address(ept_ptrs.p_ept_pdpt as _) as u64 / PAGE_SIZE as u64;
            (*ept_ptrs.p_ept_pml_4).set_physical_address(phys);
        }

        //
        // Setting up EPTP
        // Just set up the EPTP for our VMCS by putting 0x6 as the memory type (which is writeback), and we walk four times,
        // so the page walk length is 4-1=3, and the PML4 address is the physical address of the first entry in the PML4 table.
        //
        unsafe {
            (*ept_ptrs.p_eptp).set_memory_type(6);
            (*ept_ptrs.p_eptp).set_page_walk_length(3); // 4 (tables walked) - 1 = 3
            (*ept_ptrs.p_eptp).set_dirty_and_access_enabled(true);

            let pml4 =
                virtual_to_physical_address(ept_ptrs.p_ept_pml_4 as _) as u64 / PAGE_SIZE as u64;
            (*ept_ptrs.p_eptp).set_pml4_address(pml4);
        }

        println!("[///////////////////////////////////////////////////////////");
        // im curious about the bitfields macro lets see
        unsafe {
            println!(
                "(*ept_ptrs.p_eptp) mem type: {}",
                (*ept_ptrs.p_eptp).memory_type()
            );
            println!(
                "(*ept_ptrs.p_eptp) page walk len: {}",
                (*ept_ptrs.p_eptp).page_walk_length()
            );
            println!(
                "(*ept_ptrs.p_eptp) dirty: {}",
                (*ept_ptrs.p_eptp).dirty_and_access_enabled()
            );
            println!(
                "(*ept_ptrs.p_eptp) whole thing?: {:b}",
                (*ept_ptrs.p_eptp).0
            );
        }

        //
        // Finally, update the globals - ownership is transferred, everything is gucci
        //
        GLOBALS
            .ept_tables
            .store(Box::into_raw(Box::new(ept_ptrs)), SeqCst);

        let tables = GLOBALS.ept_tables.load(SeqCst);
        if !tables.is_null() {
            let eptp = unsafe { (*tables).p_eptp };

            println!(
                "[+] EPT tables stored at {:p}; EPTP allocated at {:p}",
                tables, eptp
            );
        }

        STATUS_SUCCESS
    }
}
