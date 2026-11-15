use bitfield::bitfield;

bitfield::bitfield! {
    #[derive(Clone, Copy)]
    pub struct Eptp(u64);
    impl Debug;

    // bit 2:0 (0 = Uncacheable (UC) - 6 = Write - back(WB))
    pub memory_type, set_memory_type: 2, 0;
    // bit 5:3 (This value is 1 less than the EPT page-walk length)
    pub page_walk_length, set_page_walk_length: 5, 3;
    // bit 6  (Setting this control to 1 enables accessed and dirty flags for EPT)
    pub dirty_and_access_enabled, set_dirty_and_access_enabled: 6;
    //
    // RESERVED - bits 11:7
    //
    // size 36.. aka 12 + 36 - 1 (account for base index 0) = end 47
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
    // pub reserved1, _ : 7, 3;
    pub accessed, _ : 8;
    // pub Ignored1, _ : 9;
    pub execute_for_user_mode, _ : 10;
    // pub Ignored2, _ : 11;
    pub physical_address, _ : 47, 12;
}