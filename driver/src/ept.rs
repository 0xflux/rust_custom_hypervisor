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

    pub read, _ : 0;
    pub write, _ : 1;
    pub execute, _ : 2;
    pub accessed, _ : 8;
    pub execute_for_user_mode, _ : 10;
    pub physical_address, _ : 47, 12;
}

bitfield! {
    #[derive(Clone, Copy)]
    pub struct EptPdPte(u64);
    impl Debug;

    pub read, _ : 0;
    pub write, _ : 1;
    pub execute, _ : 2;
    pub accessed, _ : 8;
    pub execute_for_user_mode, _ : 10;
    pub physical_address, _ : 47, 12;
}

bitfield! {
    #[derive(Clone, Copy)]
    pub struct EptPde(u64);
    impl Debug;

    pub read, _ : 0;
    pub write, _ : 1;
    pub execute, _ : 2;
    pub accessed, _ : 8;
    pub execute_for_user_mode, _ : 10;
    pub physical_address, _ : 47, 12;
}

bitfield! {
    #[derive(Clone, Copy)]
    pub struct EptPte(u64);
    impl Debug;

    pub read, _ : 0;
    pub write, _ : 1;
    pub execute, _ : 2;
    pub eptmemory_type, _ : 5, 3;
    pub ignore_pat, _ : 6;
    pub accessed_flag, _ : 8;
    pub dirty_flag, _ : 9;
    pub execute_for_user_mode, _ : 10;
    pub physical_address, _ : 47, 12;
    pub suppress_ve, _ : 63;
}
