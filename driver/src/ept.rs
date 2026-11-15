use bitfield::bitfield;

bitfield::bitfield! {
    #[derive(Clone, Copy)]
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
    #[derive(Clone, Copy)]
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
    #[derive(Clone, Copy)]
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
    #[derive(Clone, Copy)]
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
    #[derive(Clone, Copy)]
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
