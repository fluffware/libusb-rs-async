use std::sync::Arc;
use libusb::*;
use std::mem::MaybeUninit;

use context::ContextAsync;
use device_handle::{self, DeviceHandle};
use device_descriptor::{self, DeviceDescriptor};
use config_descriptor::{self, ConfigDescriptor};
use fields::{self, Speed};


/// A reference to a USB device.
pub struct Device {
    context: Arc<ContextAsync>,
    device: *mut libusb_device,
}

impl Drop for Device {
    /// Releases the device reference.
    fn drop(&mut self) {
        unsafe {
            libusb_unref_device(self.device);
        }
    }
}

unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Device {
    /// Reads the device descriptor.
    pub fn device_descriptor(&self) -> ::Result<DeviceDescriptor> {
        let mut descriptor = MaybeUninit::< libusb_device_descriptor>::uninit();

        // since libusb 1.0.16, this function always succeeds
        try_unsafe!(libusb_get_device_descriptor(self.device, 
                                                 descriptor.as_mut_ptr()));
        let descriptor = unsafe{descriptor.assume_init()};
        Ok(device_descriptor::from_libusb(descriptor))
    }

    /// Reads a configuration description for a given index.
    pub fn config_descriptor(&self, config_index: u8) -> ::Result<ConfigDescriptor> {
        let mut config = 
            MaybeUninit::<*const libusb_config_descriptor>::uninit();

        try_unsafe!(libusb_get_config_descriptor(self.device, config_index, 
                                                 config.as_mut_ptr()));
        let config = unsafe{config.assume_init()};
        Ok(unsafe { config_descriptor::from_libusb(config) })
    }
    
    /// Reads a configuration descriptor for a given configuration value.
    pub fn config_descriptor_by_value(&self, config_value: u8)
                                      -> ::Result<ConfigDescriptor> {
        let mut config = 
            MaybeUninit::<*const libusb_config_descriptor>::uninit();
        
        try_unsafe!(libusb_get_config_descriptor_by_value(self.device,
                                                          config_value, 
                                                          config.as_mut_ptr()));
        let config = unsafe{config.assume_init()};
        Ok(unsafe { config_descriptor::from_libusb(config) })
    }

    /// Reads the configuration descriptor for the current configuration.
    pub fn active_config_descriptor(&self) -> ::Result<ConfigDescriptor> {
        let mut config = 
            MaybeUninit::<*const libusb_config_descriptor>::uninit();

        try_unsafe!(libusb_get_active_config_descriptor(self.device,
                                                        config.as_mut_ptr()));
        let config = unsafe{config.assume_init()};
        Ok(unsafe { config_descriptor::from_libusb(config) })
    }

    /// Returns the number of the bus that the device is connected to.
    pub fn bus_number(&self) -> u8 {
        unsafe {
            libusb_get_bus_number(self.device)
        }
    }

    /// Returns the device's address on the bus that it's connected to.
    pub fn address(&self) -> u8 {
        unsafe {
            libusb_get_device_address(self.device)
        }
    }

    /// Returns the device's connection speed.
    pub fn speed(&self) -> Speed {
        fields::speed_from_libusb(unsafe {
            libusb_get_device_speed(self.device)
        })
    }

    /// Opens the device.
    pub fn open(&self) -> ::Result<DeviceHandle> {
        let mut handle = MaybeUninit::<*mut libusb_device_handle>::uninit();

        try_unsafe!(libusb_open(self.device, handle.as_mut_ptr()));
        ContextAsync::device_opened(&self.context);
        let handle = unsafe {handle.assume_init()};
        Ok(unsafe { device_handle::from_libusb(&self.context, handle) })
    }
}

#[doc(hidden)]
pub unsafe fn from_libusb(context: &Arc<ContextAsync>,
                              device: *mut libusb_device) -> Device {
    libusb_ref_device(device);

    Device {
        context: context.clone(),
        device: device,
    }
}
