mod descriptor;
mod slack;
mod socket_libc;
mod socket_std;

use std::cmp::min;
use std::ffi::CStr;
use std::ffi::CString;
use std::io::Read;
use std::io::Write;
use std::os::raw::c_char;
use std::os::raw::c_uchar;

use log::error;

#[no_mangle]
pub extern "C" fn new_descriptor_manager(port: u16) -> *mut Box<dyn descriptor::DescriptorManager> {
    if init_log().is_err() {
        eprintln!("Failed to initialize logging");
        return std::ptr::null_mut();
    };

    // std::net socket server
    //let result = socket_std::SocketDescriptorManager::new(port);
    // libc socket server
    //let result = socket_libc::SocketDescriptorManager::new(port);
    // slack server
    let result: std::io::Result<_> = Ok(slack::SlackDescriptorManager::new(
        std::env::var("SLACK_SIGNING_SECRET")
            .expect("SLACK_SIGNING_SECRET to be in the environment")
            .as_str(),
        slack_morphism::SlackApiTokenValue(
            std::env::var("SLACK_BOT_USER_OAUTH_TOKEN")
                .expect("SLACK_BOT_USER_OAUTH_TOKEN to be in the environment"),
        ),
    ));
    match result {
        Ok(manager) => Box::into_raw(Box::new(Box::new(manager))),
        // TODO: return an error?
        Err(e) => {
            error!("Cannot create DescriptorManager: {}", e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn close_descriptor_manager(
    manager: *mut Box<dyn descriptor::DescriptorManager>,
) -> i32 {
    if manager.is_null() {
        error!("Cannot close DescriptorManager: already null");
        return -1;
    }
    // SAFETY: `from_raw` can result in a double-free - the pointer is set to null and after
    // dropping and pointer is checked if it is null before dropping
    unsafe {
        Box::from_raw(manager);
        return 0;
    }
}

#[no_mangle]
pub extern "C" fn block_until_descriptor(
    mut manager: *mut Box<dyn descriptor::DescriptorManager>,
) -> i32 {
    if manager.is_null() {
        error!("Cannot block for descriptor: DescriptorManager is null");
        return -1;
    }
    // SAFETY: `from_raw` can result in a double-free - the pointer is set to null and after
    // dropping and pointer is checked if it is null before dropping
    unsafe {
        match (*manager).block_until_descriptor() {
            Ok(_) => 0,
            Err(e) => {
                error!("Cannot block for descriptor: {}", e);
                -1
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn new_descriptor(
    manager: *mut Box<dyn descriptor::DescriptorManager>,
) -> *mut Box<dyn descriptor::Descriptor> {
    if manager.is_null() {
        error!("Cannot get new descriptor: DescriptorManager is null");
        return std::ptr::null_mut();
    }

    unsafe {
        // TODO: Change DescriptorManager::new_descriptor to return
        // Result<Option<Box<Descriptor>>, Error> and have callers return None descriptors for
        // non-error cases like TryRecvError::Empty
        match (*manager).new_descriptor() {
            Ok(descriptor) => Box::into_raw(Box::new(descriptor)),
            Err(ref e) => {
                // Silently return null if they're not actual errors
                if let Some(ref error) = e.downcast_ref::<std::io::Error>() {
                    if error.kind() != std::io::ErrorKind::WouldBlock {
                        return std::ptr::null_mut();
                    }
                }

                if let Some(ref error) = e.downcast_ref::<crossbeam_channel::TryRecvError>() {
                    if error.is_empty() {
                        return std::ptr::null_mut();
                    }
                }
                error!("Cannot create new descriptor: {}", e);
                std::ptr::null_mut()
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn close_descriptor(
    mut _manager: *mut Box<dyn descriptor::DescriptorManager>,
    descriptor: *mut Box<dyn descriptor::Descriptor>,
) -> i32 {
    if descriptor.is_null() {
        error!("Cannot close descriptor: Descriptor is null");
        return -1;
    }
    unsafe {
        Box::from_raw(descriptor);
        return 0;
    }
}

#[no_mangle]
pub extern "C" fn get_descriptor_hostname(
    mut manager: *mut Box<dyn descriptor::DescriptorManager>,
    mut descriptor: *mut Box<dyn descriptor::Descriptor>,
    read_point: *mut c_uchar,
    space_left: usize,
) -> isize {
    if manager.is_null() || descriptor.is_null() || read_point.is_null() {
        error!("Cannot get descriptor hostname: argument is null");
        return -1;
    }

    unsafe {
        let hostname_bytes = (*descriptor).get_hostname().as_bytes();
        match CString::new(&hostname_bytes[..min(hostname_bytes.len(), space_left - 1)]) {
            Ok(c_hostname) => {
                let buffer = std::slice::from_raw_parts_mut(
                    read_point,
                    min(space_left, c_hostname.as_bytes_with_nul().len()),
                );
                buffer.copy_from_slice(c_hostname.as_bytes_with_nul());
                0
            }
            Err(e) => {
                error!("Cannot get descriptor hostname: {}", e);
                -1
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn read_from_descriptor(
    mut manager: *mut Box<dyn descriptor::DescriptorManager>,
    mut descriptor: *mut Box<dyn descriptor::Descriptor>,
    read_point: *mut c_uchar,
    space_left: usize,
) -> isize {
    if manager.is_null() || descriptor.is_null() || read_point.is_null() {
        error!("Cannot read from descriptor: argument is null");
        return -1;
    }

    unsafe {
        let buffer = std::slice::from_raw_parts_mut(read_point, space_left);
        match (*descriptor).read(buffer) {
            Ok(bytes) => isize::try_from(bytes).unwrap_or(-1),
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::Interrupted =>
            {
                0
            }
            Err(e) => {
                error!("Cannot read from descriptor: {}", e);
                -1
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn write_to_descriptor(
    manager: *mut Box<dyn descriptor::DescriptorManager>,
    descriptor: *mut Box<dyn descriptor::Descriptor>,
    content: *const c_char,
) -> isize {
    if manager.is_null() || descriptor.is_null() || content.is_null() {
        error!("Cannot write to descriptor: argument is null");
        return -1;
    }
    unsafe {
        match (*descriptor).write(CStr::from_ptr(content).to_bytes()) {
            Ok(written) => isize::try_from(written).unwrap_or(-1),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => 0,
            Err(e) => {
                error!("Cannot write to descriptor: {}", e);
                -1
            }
        }
    }
}

fn init_log() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use fern::colors::{Color, ColoredLevelConfig};

    let colors_level = ColoredLevelConfig::new()
        .info(Color::Green)
        .warn(Color::Magenta);

    fern::Dispatch::new()
        // Perform allocation-free log formatting
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}{}\x1B[0m",
                chrono::Local::now().format("%b %e %H:%M:%S :: "),
                record.target(),
                colors_level.color(record.level()),
                format_args!(
                    "\x1B[{}m",
                    colors_level.get_color(&record.level()).to_fg_str()
                ),
                message
            ))
        })
        // Add blanket level filter -
        .level(log::LevelFilter::Info)
        // - and per-module overrides
        .level_for("hyper", log::LevelFilter::Info)
        // Output to stdout, files, and other Dispatch configurations
        .chain(std::io::stdout())
        // Apply globally
        .apply()?;

    Ok(())
}
