use std::mem::MaybeUninit;
use std::thread::{self, JoinHandle};
use std::sync::{Arc, Mutex,RwLock};

use libc::c_int;
use libusb::*;

use device_list::{self, DeviceList};
use device_handle::{self, DeviceHandle};
use error;


// The part of the context that can be shared
pub struct ContextAsync
{
    pub context: *mut libusb_context,
    // Lock while starting and stopping thread
    async_thread: Mutex<Option<JoinHandle<()>>>,
    open_count: RwLock<u32>,
        
}

/// A `libusb` context.
pub struct Context {
    context: Arc<ContextAsync>
}

unsafe impl Send for ContextAsync {}
unsafe impl Sync for ContextAsync {}

impl Drop for ContextAsync {
    /// Closes the `libusb` context.
    fn drop(&mut self) {
        unsafe {
            libusb_exit(self.context);
        }
    }
}

unsafe impl Sync for Context {}
unsafe impl Send for Context {}

impl Context {
    /// Opens a new `libusb` context.
    pub fn new() -> ::Result<Self> {
        let mut context = MaybeUninit::<*mut libusb_context>::uninit();
            
        try_unsafe!(libusb_init(context.as_mut_ptr()));
        let context = unsafe{ context.assume_init() };
        
        let context = Arc::new(
            ContextAsync{ context: context ,
                          async_thread: Mutex::new(None),
                          open_count: RwLock::new(0),
            });
        Ok(Context {context})
    }

    /// Sets the log level of a `libusb` context.
    pub fn set_log_level(&mut self, level: LogLevel) {
        unsafe {
            libusb_set_debug(self.context.context, level.as_c_int());
        }
    }

    pub fn has_capability(&self) -> bool {
        unsafe {
            libusb_has_capability(LIBUSB_CAP_HAS_CAPABILITY) != 0
        }
    }

    /// Tests whether the running `libusb` library supports hotplug.
    pub fn has_hotplug(&self) -> bool {
        unsafe {
            libusb_has_capability(LIBUSB_CAP_HAS_HOTPLUG) != 0
        }
    }

    /// Tests whether the running `libusb` library has HID access.
    pub fn has_hid_access(&self) -> bool {
        unsafe {
            libusb_has_capability(LIBUSB_CAP_HAS_HID_ACCESS) != 0
        }
    }

    /// Tests whether the running `libusb` library supports detaching the kernel driver.
    pub fn supports_detach_kernel_driver(&self) -> bool {
        unsafe {
            libusb_has_capability(LIBUSB_CAP_SUPPORTS_DETACH_KERNEL_DRIVER) != 0
        }
    }

    /// Returns a list of the current USB devices. The context must outlive the device list.
    pub fn devices(&self) -> ::Result<DeviceList> {
        let mut list = MaybeUninit::<*const *mut libusb_device>::uninit();

        let n = unsafe { libusb_get_device_list(
            self.context.context,
            list.as_mut_ptr()) };
        let list = unsafe{list.assume_init()};

        if n < 0 {
            Err(error::from_libusb(n as c_int))
        }
        else {
            Ok(unsafe { device_list::from_libusb(&self.context, list, n as usize) })
        }
    }

    /// Convenience function to open a device by its vendor ID and product ID.
    ///
    /// This function is provided as a convenience for building prototypes without having to
    /// iterate a [`DeviceList`](struct.DeviceList.html). It is not meant for production
    /// applications.
    ///
    /// Returns a device handle for the first device found matching `vendor_id` and `product_id`.
    /// On error, or if the device could not be found, it returns `None`.
    pub fn open_device_with_vid_pid<'a>(&'a self, vendor_id: u16, product_id: u16) -> Option<DeviceHandle> {
        let handle = unsafe { libusb_open_device_with_vid_pid(
            self.context.context, vendor_id, product_id) };

        if handle.is_null() {
            None
        }
        else {
            Some(unsafe { device_handle::from_libusb(&self.context, handle) })
        }
    }

}

impl ContextAsync
{
    /// A device has been opened and if necessary start the event loop
    pub fn device_opened(ca: &Arc<Self>)
    {
        let mut thread = ca.async_thread.lock().unwrap();
        let mut count = ca.open_count.write().unwrap();
        *count += 1;

        if thread.is_none() {
            let context = ca.clone();
            *thread = Some(thread::spawn(move || {
                println!("USB event loop started");
                let libusb_ctxt = context.context;
                loop {
                    {
                        let count = context.open_count.read().unwrap();
                        if *count == 0 {
                            break;
                        }
                    }
                    unsafe {
                        libusb_handle_events(libusb_ctxt);
                    }
                }
                println!("USB event loop stopped");
            }));
        }
    }


    /// Close a device
    /// The actual closing should be done in the supplied closure.
    /// This is so the correct lock cn be held while doing it.
    pub fn device_close<F>(ca: &Arc<Self>, close: F)
        where F: FnOnce()
    {
        let mut thread = ca.async_thread.lock().unwrap();
        {
            let mut count = ca.open_count.write().unwrap();
            *count -= 1;
        }
        close();
        let count = ca.open_count.read().unwrap();
        if *count == 0 {
            if let Some(join) = thread.take() {
                join.join().unwrap();
            }
        }
    }

}

/// Library logging levels.
pub enum LogLevel {
    /// No messages are printed by `libusb` (default).
    None,

    /// Error messages printed to `stderr`.
    Error,

    /// Warning and error messages are printed to `stderr`.
    Warning,

    /// Informational messages are printed to `stdout`. Warnings and error messages are printed to
    /// `stderr`.
    Info,

    /// Debug and informational messages are printed to `stdout`. Warnings and error messages are
    /// printed to `stderr`.
    Debug,
}

impl LogLevel {
    fn as_c_int(&self) -> c_int {
        match *self {
            LogLevel::None    => LIBUSB_LOG_LEVEL_NONE,
            LogLevel::Error   => LIBUSB_LOG_LEVEL_ERROR,
            LogLevel::Warning => LIBUSB_LOG_LEVEL_WARNING,
            LogLevel::Info    => LIBUSB_LOG_LEVEL_INFO,
            LogLevel::Debug   => LIBUSB_LOG_LEVEL_DEBUG,
        }
    }
}

