use core::{
    ptr::null_mut,
    sync::atomic::{AtomicPtr, AtomicU32, Ordering::SeqCst},
};

use alloc::{boxed::Box, vec::Vec};

use crate::memory::VirtualMachineState;

pub static GLOABLS: Globals = Globals {
    guest_state: AtomicPtr::new(null_mut()),
    processor_counts: AtomicU32::new(0),
};

#[derive(Default)]
pub struct Globals {
    pub guest_state: AtomicPtr<Vec<VirtualMachineState>>,
    pub processor_counts: AtomicU32,
}

impl Globals {
    /// For each dynamically allocated bit of data, ensure we free it
    pub fn free_all(&self) {
        {
            let ptr = self.guest_state.load(SeqCst);
            if !ptr.is_null() {
                drop(unsafe { Box::from_raw(ptr) });
            }
        }
    }
}
