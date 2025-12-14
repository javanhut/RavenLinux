//! Linux kernel loading and boot protocol

use alloc::vec::Vec;
use core::ffi::c_void;
use core::ptr;
use uefi::prelude::*;
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::table::boot::{AllocateType, LoadImageSource, MemoryType};
use uefi::{guid, CString16, Guid};

// Linux initrd protocol GUID - used by kernel to find initrd
const LINUX_EFI_INITRD_MEDIA_GUID: Guid = guid!("5568e427-68fc-4f3d-ac74-ca555231cc68");

// EFI Device Path Protocol GUID
const EFI_DEVICE_PATH_PROTOCOL_GUID: Guid = guid!("09576e91-6d3f-11d2-8e39-00a0c969723b");

// EFI Load File 2 Protocol GUID
const EFI_LOAD_FILE2_PROTOCOL_GUID: Guid = guid!("4006c0c1-fcb3-403e-996d-4a6c8724e06d");

/// Kernel loading errors
#[derive(Debug, Clone, Copy)]
pub enum KernelError {
    NotFound,
    KernelNotFound,
    InitrdNotFound,
    EfiAppNotFound,
    InvalidFormat,
    NotImplemented,
    MemoryAllocation,
    TooLarge,
    FileSystemError,
    LoadImageFailed,
    StartImageFailed,
}

/// Static storage for initrd data (needs to persist during kernel boot)
static mut INITRD_ADDRESS: u64 = 0;
static mut INITRD_SIZE: usize = 0;

// EFI_LOAD_FILE2_PROTOCOL for initrd
#[repr(C)]
struct LoadFile2Protocol {
    load_file: unsafe extern "efiapi" fn(
        this: *const LoadFile2Protocol,
        file_path: *const c_void,
        boot_policy: u8,
        buffer_size: *mut usize,
        buffer: *mut c_void,
    ) -> uefi::Status,
}

// LoadFile2 implementation for initrd
unsafe extern "efiapi" fn initrd_load_file(
    _this: *const LoadFile2Protocol,
    _file_path: *const c_void,
    _boot_policy: u8,
    buffer_size: *mut usize,
    buffer: *mut c_void,
) -> uefi::Status {
    if INITRD_SIZE == 0 {
        return uefi::Status::NOT_FOUND;
    }

    if buffer.is_null() || *buffer_size < INITRD_SIZE {
        *buffer_size = INITRD_SIZE;
        return uefi::Status::BUFFER_TOO_SMALL;
    }

    ptr::copy_nonoverlapping(INITRD_ADDRESS as *const u8, buffer as *mut u8, INITRD_SIZE);
    *buffer_size = INITRD_SIZE;
    uefi::Status::SUCCESS
}

// Static protocol instance
static mut INITRD_PROTOCOL: LoadFile2Protocol = LoadFile2Protocol {
    load_file: initrd_load_file,
};

// Vendor device path node for Linux initrd
// This is what the kernel looks for to find the initrd
#[repr(C, packed)]
struct VendorDevicePath {
    header: DevicePathHeader,
    vendor_guid: [u8; 16],
}

#[repr(C, packed)]
struct DevicePathHeader {
    device_type: u8,
    sub_type: u8,
    length: [u8; 2],
}

#[repr(C, packed)]
struct EndDevicePath {
    header: DevicePathHeader,
}

// Complete device path for initrd: Vendor node + End node
#[repr(C, packed)]
struct InitrdDevicePath {
    vendor: VendorDevicePath,
    end: EndDevicePath,
}

// Static device path for initrd
static mut INITRD_DEVICE_PATH: InitrdDevicePath = InitrdDevicePath {
    vendor: VendorDevicePath {
        header: DevicePathHeader {
            device_type: 0x04,  // MEDIA_DEVICE_PATH
            sub_type: 0x03,     // MEDIA_VENDOR_DP
            length: [24, 0],    // sizeof(VendorDevicePath) = 4 + 16 + 4 = 24... wait, it's 4 + 16 = 20
        },
        vendor_guid: [0; 16],   // Will be set at runtime
    },
    end: EndDevicePath {
        header: DevicePathHeader {
            device_type: 0x7f,  // END_DEVICE_PATH_TYPE
            sub_type: 0xff,     // END_ENTIRE_DEVICE_PATH_SUBTYPE
            length: [4, 0],     // sizeof(EndDevicePath) = 4
        },
    },
};

// Raw EFI Boot Services function types
type InstallMultipleProtocolInterfacesFn = unsafe extern "efiapi" fn(
    handle: *mut *mut c_void,
    ...
) -> uefi::Status;

// Partial raw EFI_BOOT_SERVICES structure
#[repr(C)]
struct RawBootServices {
    hdr: [u8; 24],                           // EFI_TABLE_HEADER (24 bytes)
    // Task Priority Services (2 functions)
    raise_tpl: usize,
    restore_tpl: usize,
    // Memory Services (5 functions)
    allocate_pages: usize,
    free_pages: usize,
    get_memory_map: usize,
    allocate_pool: usize,
    free_pool: usize,
    // Event & Timer Services (6 functions)
    create_event: usize,
    set_timer: usize,
    wait_for_event: usize,
    signal_event: usize,
    close_event: usize,
    check_event: usize,
    // Protocol Handler Services (10 functions)
    install_protocol_interface: usize,
    reinstall_protocol_interface: usize,
    uninstall_protocol_interface: usize,
    handle_protocol: usize,
    reserved: usize,
    register_protocol_notify: usize,
    locate_handle: usize,
    locate_device_path: usize,
    install_configuration_table: usize,
    // Image Services (5 functions)
    load_image: usize,
    start_image: usize,
    exit: usize,
    unload_image: usize,
    exit_boot_services: usize,
    // Miscellaneous Services (4 functions)
    get_next_monotonic_count: usize,
    stall: usize,
    set_watchdog_timer: usize,
    // Driver Support Services (2 functions)
    connect_controller: usize,
    disconnect_controller: usize,
    // Open/Close Protocol Services (3 functions)
    open_protocol: usize,
    close_protocol: usize,
    open_protocol_information: usize,
    // Library Services (3 functions)
    protocols_per_handle: usize,
    locate_handle_buffer: usize,
    locate_protocol: usize,
    // Multiple Protocol Interface functions
    install_multiple_protocol_interfaces: unsafe extern "efiapi" fn(
        handle: *mut *mut c_void,
        protocol1: *const Guid,
        interface1: *const c_void,
        protocol2: *const Guid,
        interface2: *const c_void,
        null: *const c_void,
    ) -> uefi::Status,
}

/// Boot an EFI stub kernel with proper initrd and cmdline support
pub fn boot_efi_stub(
    boot_services: &BootServices,
    image_handle: Handle,
    kernel_path: &str,
    initrd_path: Option<&str>,
    cmdline: &str,
) -> Result<(), KernelError> {
    // Get the device path of our bootloader image to find the ESP
    let loaded_image = boot_services
        .open_protocol_exclusive::<LoadedImage>(image_handle)
        .map_err(|_| KernelError::FileSystemError)?;

    let device_handle = loaded_image.device().ok_or(KernelError::FileSystemError)?;

    // Open the filesystem
    let mut fs = boot_services
        .open_protocol_exclusive::<SimpleFileSystem>(device_handle)
        .map_err(|_| KernelError::FileSystemError)?;

    let mut root = fs.open_volume().map_err(|_| KernelError::FileSystemError)?;

    // Read the kernel file into memory
    let kernel_data = read_file(&mut root, kernel_path).map_err(|err| match err {
        KernelError::NotFound => KernelError::KernelNotFound,
        other => other,
    })?;

    // Read initrd if specified and set up the protocol
    if let Some(initrd) = initrd_path {
        let initrd_data = read_file(&mut root, initrd).map_err(|err| match err {
            KernelError::NotFound => KernelError::InitrdNotFound,
            other => other,
        })?;
        let initrd_len = initrd_data.len();

        // Allocate persistent memory for initrd (won't be freed)
        let initrd_pages = (initrd_len + 4095) / 4096;
        let initrd_mem = boot_services
            .allocate_pages(
                AllocateType::AnyPages,
                MemoryType::LOADER_DATA,
                initrd_pages,
            )
            .map_err(|_| KernelError::MemoryAllocation)?;

        // Copy initrd data to allocated memory
        unsafe {
            ptr::copy_nonoverlapping(initrd_data.as_ptr(), initrd_mem as *mut u8, initrd_len);

            // Store initrd info in static storage for the protocol callback
            INITRD_ADDRESS = initrd_mem;
            INITRD_SIZE = initrd_len;

            // Set up the vendor device path with Linux initrd GUID
            // The GUID bytes need to be in the correct order
            let guid_bytes = LINUX_EFI_INITRD_MEDIA_GUID.to_bytes();
            INITRD_DEVICE_PATH.vendor.vendor_guid.copy_from_slice(&guid_bytes);
            // Fix the length field: VendorDevicePath = 4 (header) + 16 (guid) = 20 bytes
            INITRD_DEVICE_PATH.vendor.header.length = [20, 0];

            // Install both DevicePath and LoadFile2 protocols on a new handle
            // The kernel looks for LoadFile2 on handles that have the Linux initrd device path
            let bs_ptr = boot_services as *const BootServices as *const RawBootServices;

            // Create a null handle that will be allocated by install_multiple_protocol_interfaces
            let mut new_handle: *mut c_void = ptr::null_mut();

            // Install both protocols at once
            let status = ((*bs_ptr).install_multiple_protocol_interfaces)(
                &mut new_handle,
                &EFI_DEVICE_PATH_PROTOCOL_GUID,
                &INITRD_DEVICE_PATH as *const InitrdDevicePath as *const c_void,
                &EFI_LOAD_FILE2_PROTOCOL_GUID,
                &INITRD_PROTOCOL as *const LoadFile2Protocol as *const c_void,
                ptr::null::<c_void>(),
            );

            if status != uefi::Status::SUCCESS {
                // Protocol installation failed - this is critical for initrd
                // But continue anyway and hope the kernel can boot without it
            }
        }
    }

    // Drop fs handle before loading kernel
    drop(fs);
    drop(loaded_image);

    // Load the kernel as an EFI image from memory
    let kernel_image = boot_services
        .load_image(
            image_handle,
            LoadImageSource::FromBuffer {
                buffer: &kernel_data,
                file_path: None,
            },
        )
        .map_err(|_| KernelError::LoadImageFailed)?;

    // Set the command line (LoadOptions) for the kernel
    if !cmdline.is_empty() {
        if let Ok(mut loaded_kernel) =
            boot_services.open_protocol_exclusive::<LoadedImage>(kernel_image)
        {
            // Convert command line to UCS-2 and allocate memory for it
            let cmdline_len = cmdline.len();
            let cmdline_size = (cmdline_len + 1) * 2; // UCS-2 = 2 bytes per char + null

            // Allocate memory for command line
            if let Ok(cmdline_mem) = boot_services.allocate_pages(
                AllocateType::AnyPages,
                MemoryType::LOADER_DATA,
                (cmdline_size + 4095) / 4096,
            ) {
                let cmdline_ptr = cmdline_mem as *mut u16;

                // Convert ASCII to UCS-2
                unsafe {
                    for (i, byte) in cmdline.bytes().enumerate() {
                        *cmdline_ptr.add(i) = byte as u16;
                    }
                    *cmdline_ptr.add(cmdline_len) = 0; // Null terminator

                    // Set LoadOptions on the loaded kernel image
                    // LoadOptions is a pointer to the command line
                    // LoadOptionsSize is the size in bytes

                    // Get raw pointer to LoadedImage protocol
                    let li_ptr = &mut *loaded_kernel as *mut LoadedImage;

                    // The LoadedImage struct has load_options and load_options_size fields
                    // We need to set them directly
                    // Unfortunately uefi-rs doesn't expose setters, so we use raw access
                    let li_raw = li_ptr as *mut LoadedImageRaw;
                    (*li_raw).load_options = cmdline_ptr as *const c_void;
                    (*li_raw).load_options_size = cmdline_size as u32;
                }
            }
        }
    }

    // Start the kernel image - this should not return on success
    boot_services
        .start_image(kernel_image)
        .map_err(|_| KernelError::StartImageFailed)?;

    // If we get here, something went wrong
    Err(KernelError::StartImageFailed)
}

/// Raw LoadedImage structure for direct field access
#[repr(C)]
struct LoadedImageRaw {
    revision: u32,
    parent_handle: *const c_void,
    system_table: *const c_void,
    device_handle: *const c_void,
    file_path: *const c_void,
    reserved: *const c_void,
    load_options_size: u32,
    load_options: *const c_void,
    image_base: *const c_void,
    image_size: u64,
    image_code_type: u32,
    image_data_type: u32,
    unload: *const c_void,
}

/// Read a file from the filesystem into a Vec
fn read_file(
    root: &mut uefi::proto::media::file::Directory,
    path: &str,
) -> Result<Vec<u8>, KernelError> {
    // Convert path to UCS-2
    let path_cstr = CString16::try_from(path).map_err(|_| KernelError::NotFound)?;

    // Open the file
    let file_handle = root
        .open(&path_cstr, FileMode::Read, FileAttribute::empty())
        .map_err(|_| KernelError::NotFound)?;

    let mut file = match file_handle.into_type().map_err(|_| KernelError::InvalidFormat)? {
        uefi::proto::media::file::FileType::Regular(f) => f,
        _ => return Err(KernelError::InvalidFormat),
    };

    // Get file size
    let mut info_buf = [0u8; 256];
    let info: &FileInfo = file
        .get_info(&mut info_buf)
        .map_err(|_| KernelError::FileSystemError)?;

    let file_size = info.file_size() as usize;

    // Allocate buffer and read file
    let mut buffer = Vec::with_capacity(file_size);
    buffer.resize(file_size, 0);

    file.read(&mut buffer)
        .map_err(|_| KernelError::FileSystemError)?;

    Ok(buffer)
}

/// Chainload another EFI application (e.g., Windows Boot Manager)
pub fn chainload_efi(
    boot_services: &BootServices,
    image_handle: Handle,
    efi_path: &str,
) -> Result<(), KernelError> {
    // Get device handle from our loaded image
    let loaded_image = boot_services
        .open_protocol_exclusive::<LoadedImage>(image_handle)
        .map_err(|_| KernelError::FileSystemError)?;

    let device_handle = loaded_image.device().ok_or(KernelError::FileSystemError)?;

    // Open filesystem
    let mut fs = boot_services
        .open_protocol_exclusive::<SimpleFileSystem>(device_handle)
        .map_err(|_| KernelError::FileSystemError)?;

    let mut root = fs.open_volume().map_err(|_| KernelError::FileSystemError)?;

    // Read the EFI application
    let efi_data = read_file(&mut root, efi_path).map_err(|err| match err {
        KernelError::NotFound => KernelError::EfiAppNotFound,
        other => other,
    })?;

    // Drop handles before loading
    drop(fs);
    drop(loaded_image);

    // Load and start the image
    let chain_image = boot_services
        .load_image(
            image_handle,
            LoadImageSource::FromBuffer {
                buffer: &efi_data,
                file_path: None,
            },
        )
        .map_err(|_| KernelError::LoadImageFailed)?;

    boot_services
        .start_image(chain_image)
        .map_err(|_| KernelError::StartImageFailed)?;

    Err(KernelError::StartImageFailed)
}

// Need to add alloc crate for Vec in no_std
extern crate alloc;
