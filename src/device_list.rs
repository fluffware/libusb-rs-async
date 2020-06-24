use std::slice;
use std::sync::{Arc};

use libusb::*;

use context::ContextAsync;
use device::{self, Device};

/// A list of detected USB devices.
pub struct DeviceList {
    context: Arc<ContextAsync>,
    list: *const *mut libusb_device,
    len: usize,
}

impl Drop for DeviceList {
    /// Frees the device list.
    fn drop(&mut self) {
        unsafe {
            libusb_free_device_list(self.list, 1);
        }
    }
}

impl DeviceList {
    /// Returns the number of devices in the list.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns an iterator over the devices in the list.
    ///
    /// The iterator yields a sequence of `Device` objects.
    pub fn iter(&self) -> Devices {
        Devices {
            context: self.context.clone(),
            devices: unsafe { slice::from_raw_parts(self.list, self.len) },
            index: 0,
        }
    }
}

/// Iterator over detected USB devices.
pub struct Devices<'b> {
    context: Arc<ContextAsync>,
    devices: &'b [*mut libusb_device],
    index: usize,
}

impl<'b> Iterator for Devices<'b> {
    type Item = Device;

    fn next(&mut self) -> Option<Device> {
        if self.index < self.devices.len() {
            let device = self.devices[self.index];

            self.index += 1;
            Some(unsafe { device::from_libusb(&self.context, device) })
        }
        else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.devices.len() - self.index;
        (remaining, Some(remaining))
    }
}


#[doc(hidden)]
pub unsafe fn from_libusb(context: &Arc<ContextAsync>, list: *const *mut libusb_device, len: usize,) -> DeviceList {
    DeviceList {
        context: context.clone(),
        list: list,
        len: len,
    }
}
