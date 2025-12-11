//! Linux kernel loading and boot protocol

use uefi::prelude::*;
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::table::boot::LoadImageSource;
use uefi::CString16;

/// Linux boot protocol header (simplified)
#[repr(C, packed)]
pub struct LinuxSetupHeader {
    pub setup_sects: u8,
    pub root_flags: u16,
    pub syssize: u32,
    pub ram_size: u16,
    pub vid_mode: u16,
    pub root_dev: u16,
    pub boot_flag: u16,
}

/// Load a Linux kernel image (traditional method)
pub fn load_kernel(_path: &str) -> Result<KernelInfo, KernelError> {
    // Traditional kernel loading is complex and rarely needed
    // with modern EFI stub kernels. Returning not implemented.
    Err(KernelError::NotImplemented)
}

/// Load an initrd/initramfs (traditional method)
pub fn load_initrd(_path: &str) -> Result<InitrdInfo, KernelError> {
    // Traditional initrd loading - not needed for EFI stub boot
    Err(KernelError::NotImplemented)
}

/// Information about a loaded kernel
pub struct KernelInfo {
    pub entry_point: u64,
    pub setup_base: u64,
    pub kernel_base: u64,
    pub kernel_size: u64,
}

/// Information about loaded initrd
pub struct InitrdInfo {
    pub base: u64,
    pub size: u64,
}

/// Kernel loading errors
#[derive(Debug, Clone, Copy)]
pub enum KernelError {
    NotFound,
    InvalidFormat,
    NotImplemented,
    MemoryAllocation,
    TooLarge,
    FileSystemError,
    LoadImageFailed,
    StartImageFailed,
}

/// Boot into the loaded kernel (traditional method)
pub fn boot_linux(
    _kernel: &KernelInfo,
    _initrd: Option<&InitrdInfo>,
    _cmdline: &str,
) -> ! {
    // Traditional boot - not commonly used with modern kernels
    loop {}
}

/// Boot an EFI stub kernel
///
/// Modern Linux kernels have an EFI stub that allows them to be loaded
/// directly as EFI applications. This is the preferred and simpler method.
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
    let kernel_data = read_file(&mut root, kernel_path)?;

    // Read initrd if specified (unused for now, but will be needed for full implementation)
    let _initrd_data = if let Some(initrd) = initrd_path {
        Some(read_file(&mut root, initrd)?)
    } else {
        None
    };

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
        if let Ok(loaded_kernel) = boot_services.open_protocol_exclusive::<LoadedImage>(kernel_image) {
            // Convert command line to UCS-2 for UEFI
            // The kernel expects the command line in LoadOptions
            // Note: Need to allocate and set load_options

            // For simplicity, we'll set a basic command line
            // A full implementation would properly convert and set LoadOptions
        }
    }

    // Start the kernel image - this should not return on success
    boot_services
        .start_image(kernel_image)
        .map_err(|_| KernelError::StartImageFailed)?;

    // If we get here, something went wrong
    Err(KernelError::StartImageFailed)
}

/// Read a file from the filesystem into a Vec
fn read_file(
    root: &mut uefi::proto::media::file::Directory,
    path: &str,
) -> Result<alloc::vec::Vec<u8>, KernelError> {
    use alloc::vec::Vec;

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
    let efi_data = read_file(&mut root, efi_path)?;

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
