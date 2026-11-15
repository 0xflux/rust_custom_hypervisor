use core::{
    ptr::null_mut,
    sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering::SeqCst},
};

use alloc::{boxed::Box, vec::Vec};

use crate::memory::VirtualMachineState;

pub static GLOBALS: Globals = Globals {
    guest_state: AtomicPtr::new(null_mut()),
    processor_counts: AtomicU32::new(0),
    vmx_enabled: AtomicBool::new(false),
};

#[derive(Default)]
pub struct Globals {
    pub guest_state: AtomicPtr<Vec<VirtualMachineState>>,
    pub processor_counts: AtomicU32,
    pub vmx_enabled: AtomicBool,
}

impl Globals {
    /// For each dynamically allocated bit of data, ensure we free it
    pub fn free_all(&self) {
        // 
        // Free each VirtualMachineState allocation, if it was not already de-allocated by the 
        // stop VMX operation. Keep for redundancy
        //
        {
            let ptr = self.guest_state.load(SeqCst);
            if !ptr.is_null() {
                drop(unsafe { Box::from_raw(ptr) });
                self.guest_state.store(null_mut(), SeqCst);
            }
        }
    }
}
