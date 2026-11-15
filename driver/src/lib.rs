#![no_std]

extern crate alloc;

#[cfg(not(test))]
extern crate wdk_panic;

use core::{
    iter::once,
    ptr::null_mut,
    sync::atomic::{AtomicPtr, Ordering::SeqCst},
};

use alloc::vec::Vec;
use shared::{
    DOS_DEVICE_NAME, IOCTL_SIOCTL_METHOD_BUFFERED, IOCTL_SIOCTL_METHOD_IN_DIRECT,
    IOCTL_SIOCTL_METHOD_NEITHER, IOCTL_SIOCTL_METHOD_OUT_DIRECT, NT_DEVICE_NAME,
};
use wdk::{nt_success, paged_code, println};
#[cfg(not(test))]
use wdk_alloc::WdkAllocator;

#[cfg(not(test))]
#[global_allocator]
static GLOBAL_ALLOCATOR: WdkAllocator = WdkAllocator;

use wdk_sys::{
    DEVICE_OBJECT, DO_BUFFERED_IO, DO_DEVICE_INITIALIZING, DRIVER_OBJECT, FILE_DEVICE_SECURE_OPEN,
    FILE_DEVICE_UNKNOWN, IO_NO_INCREMENT, IO_TYPE_DEVICE, IRP, IRP_MJ_CLOSE, IRP_MJ_CREATE,
    IRP_MJ_DEVICE_CONTROL, IRP_MJ_READ, IRP_MJ_WRITE, NTSTATUS, PCUNICODE_STRING, PDEVICE_OBJECT,
    PIO_STACK_LOCATION, PIRP, PUNICODE_STRING, STATUS_INVALID_DEVICE_REQUEST,
    STATUS_INVALID_PARAMETER, STATUS_SUCCESS, STATUS_UNSUCCESSFUL, UNICODE_STRING,
    ntddk::{
        IoCreateDevice, IoCreateSymbolicLink, IoDeleteDevice, IoDeleteSymbolicLink,
        IofCompleteRequest, RtlInitUnicodeString,
    },
};

use crate::{
    globals::GLOBALS,
    vmx::{start_vmx, stop_vmx},
};

mod globals;
mod memory;
mod processor;
mod vmx;

#[unsafe(export_name = "DriverEntry")]
pub unsafe extern "system" fn driver_entry(
    driver: &mut DRIVER_OBJECT,
    registry_path: PCUNICODE_STRING,
) -> NTSTATUS {
    println!("[i] Starting Hypervisor POC.");

    let status = init_wdk_driver(driver, registry_path as *mut _);
    if !nt_success(status) {
        driver_exit(driver);
        return status;
    }

    println!("[+] Driver initialised correctly.");

    let status = start_vmx();
    if !nt_success(status) {
        println!("[-] Failed to start VMX");
        driver_exit(driver);
        return status;
    }

    println!("");
    println!("========================================================");
    println!("[+] VMX operation started successfully.");

    STATUS_SUCCESS
}

fn init_wdk_driver(driver: &mut DRIVER_OBJECT, _registry_path: PUNICODE_STRING) -> NTSTATUS {
    let mut dos_name = UNICODE_STRING::default();
    let mut nt_name = UNICODE_STRING::default();

    let dos_name_u16: Vec<u16> = DOS_DEVICE_NAME.encode_utf16().chain(once(0)).collect();
    let device_name_u16: Vec<u16> = NT_DEVICE_NAME.encode_utf16().chain(once(0)).collect();

    unsafe { RtlInitUnicodeString(&mut dos_name, dos_name_u16.as_ptr()) };
    unsafe { RtlInitUnicodeString(&mut nt_name, device_name_u16.as_ptr()) };

    let mut device_object: PDEVICE_OBJECT = null_mut();

    let res = unsafe {
        IoCreateDevice(
            driver,
            0,
            &mut nt_name,
            FILE_DEVICE_UNKNOWN,
            FILE_DEVICE_SECURE_OPEN,
            0,
            &mut device_object,
        )
    };
    if !nt_success(res) || device_object.is_null() {
        println!("[-] Unable to create device via IoCreateDevice. Failed with code: {res}.");
        return res;
    }

    let res = unsafe { IoCreateSymbolicLink(&mut dos_name, &mut nt_name) };
    if res != 0 {
        println!("[-] Failed to create driver symbolic link. Error: {res}");

        return STATUS_UNSUCCESSFUL;
    }

    driver.MajorFunction[IRP_MJ_CREATE as usize] = Some(create_close);
    driver.MajorFunction[IRP_MJ_CLOSE as usize] = Some(create_close);
    driver.MajorFunction[IRP_MJ_DEVICE_CONTROL as usize] = Some(ioctl_handler);
    driver.MajorFunction[IRP_MJ_READ as usize] = Some(irp_read);
    driver.MajorFunction[IRP_MJ_WRITE as usize] = Some(irp_write);

    driver.DriverUnload = Some(driver_exit);

    unsafe {
        (*device_object).Flags |= DO_BUFFERED_IO | IO_TYPE_DEVICE;
        (*device_object).Flags &= !DO_DEVICE_INITIALIZING;
    }

    STATUS_SUCCESS
}

pub unsafe fn IoGetCurrentIrpStackLocation(irp: PIRP) -> PIO_STACK_LOCATION {
    unsafe {
        assert!((*irp).CurrentLocation <= (*irp).StackCount + 1); // todo maybe do error handling instead of an assert?
        (*irp)
            .Tail
            .Overlay
            .__bindgen_anon_2
            .__bindgen_anon_1
            .CurrentStackLocation
    }
}

unsafe extern "C" fn ioctl_handler(_device: *mut DEVICE_OBJECT, irp: *mut IRP) -> NTSTATUS {
    unsafe {
        paged_code!();

        let mut status = STATUS_SUCCESS;

        let irp_stack = IoGetCurrentIrpStackLocation(irp);
        if irp_stack.is_null() {
            println!("[!] IRP stack ptr was null");
            return STATUS_INVALID_PARAMETER;
        }

        let in_buf_len = (*irp_stack).Parameters.DeviceIoControl.InputBufferLength;
        let out_buf_len = (*irp_stack).Parameters.DeviceIoControl.OutputBufferLength;

        let data = "String from driver!";

        if in_buf_len == 0 || out_buf_len < data.len() as _ {
            return STATUS_INVALID_PARAMETER;
        }

        match (*irp_stack).Parameters.DeviceIoControl.IoControlCode {
            IOCTL_SIOCTL_METHOD_BUFFERED => {
                println!("Called IOCTL_SIOCTL_METHOD_BUFFERED");
            }
            IOCTL_SIOCTL_METHOD_NEITHER => {
                println!("Called IOCTL_SIOCTL_METHOD_NEITHER");
            }
            IOCTL_SIOCTL_METHOD_IN_DIRECT => {
                println!("Called IOCTL_SIOCTL_METHOD_IN_DIRECT");
            }
            IOCTL_SIOCTL_METHOD_OUT_DIRECT => {
                println!("Called IOCTL_SIOCTL_METHOD_OUT_DIRECT");
            }
            _ => {
                println!("[-] IOCTL method not handled.");
                status = STATUS_INVALID_DEVICE_REQUEST;
            }
        }

        (*irp).IoStatus.__bindgen_anon_1.Status = status;
        (*irp).IoStatus.Information = 0;
        IofCompleteRequest(irp, IO_NO_INCREMENT as _);

        status
    }
}

unsafe extern "C" fn irp_read(_device: *mut DEVICE_OBJECT, irp: *mut IRP) -> NTSTATUS {
    println!("[i] IRP Read not implemented");

    unsafe {
        (*irp).IoStatus.__bindgen_anon_1.Status = STATUS_SUCCESS;
        (*irp).IoStatus.Information = 0;
        IofCompleteRequest(irp, IO_NO_INCREMENT as _);
    }

    STATUS_SUCCESS
}

unsafe extern "C" fn irp_write(_device: *mut DEVICE_OBJECT, irp: *mut IRP) -> NTSTATUS {
    println!("[i] IRP Write not implemented");

    unsafe {
        (*irp).IoStatus.__bindgen_anon_1.Status = STATUS_SUCCESS;
        (*irp).IoStatus.Information = 0;
        IofCompleteRequest(irp, IO_NO_INCREMENT as _);
    }

    STATUS_SUCCESS
}

unsafe extern "C" fn create_close(_device: *mut DEVICE_OBJECT, pirp: PIRP) -> NTSTATUS {
    unsafe {
        (*pirp).IoStatus.__bindgen_anon_1.Status = STATUS_SUCCESS;
        (*pirp).IoStatus.Information = 0;
        IofCompleteRequest(pirp, IO_NO_INCREMENT as i8);
    }

    STATUS_SUCCESS
}

extern "C" fn driver_exit(driver: *mut DRIVER_OBJECT) {
    let mut dos_name = UNICODE_STRING::default();
    let dos_name_u16: Vec<u16> = DOS_DEVICE_NAME.encode_utf16().chain(once(0)).collect();
    unsafe {
        RtlInitUnicodeString(&mut dos_name, dos_name_u16.as_ptr());
    }
    let _ = unsafe { IoDeleteSymbolicLink(&mut dos_name) };

    unsafe {
        IoDeleteDevice((*driver).DeviceObject);
    }

    stop_vmx();
    GLOBALS.free_all();

    println!("[i] Hypervisor driver unloaded.")
}
