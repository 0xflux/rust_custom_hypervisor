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
