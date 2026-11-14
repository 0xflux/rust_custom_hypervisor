use shared::DOS_DEVICE_NAME;
use windows::{Win32::{Foundation::{GENERIC_READ, GENERIC_WRITE}, Storage::FileSystem::{CreateFileA, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING}}, core::PCSTR};

fn main() {
    println!("[i] Starting Hypervisor usermode element..");

    let h_wdn = unsafe {
        CreateFileA(
            PCSTR(DOS_DEVICE_NAME.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING, 
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED,
            None
        )
    }.unwrap();

    println!("[i] Opened handle to driver: {h_wdn:?}");

}
