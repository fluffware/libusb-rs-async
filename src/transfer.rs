use std::sync::{Arc,Mutex};
use context::ContextAsync;
use std::future::{Future};
use std::task;
use std::pin::Pin;
use std::ops::DerefMut;
use libusb::{
    self,
    libusb_transfer,
    libusb_free_transfer,
    libusb_submit_transfer,
    libusb_cancel_transfer
};
use libc::{c_uchar, c_int};
use std::convert::TryFrom;
use std::fmt;

/// The result of a finished transfer request sent by
/// [`Transfer::submit`](struct.Transfer.html#method.submit)
#[derive(Debug,PartialEq,Eq,Clone,Copy,Hash)]
pub enum TransferStatus
{
    Completed,
    Error,
    TimedOut,
    Cancelled,
    Stall,
    NoDevice,
    Overflow,
    Unknown
}

impl fmt::Display for TransferStatus
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(
            match self {
                TransferStatus::Completed => "Completed",
                TransferStatus::Error => "Error",
                TransferStatus::TimedOut => "Timed out",
                TransferStatus::Cancelled => "Cancelled",
                TransferStatus::Stall => "Stall",
                TransferStatus::NoDevice => "No device",
                TransferStatus::Overflow => "Overflow",
                TransferStatus::Unknown => "Unknown status"
            })
    }
}

impl From<c_int> for TransferStatus
{
    fn from(status_value: c_int) -> Self
    {
        match status_value {
            libusb::LIBUSB_TRANSFER_COMPLETED => TransferStatus::Completed,
            libusb::LIBUSB_TRANSFER_ERROR => TransferStatus::Error,
            libusb::LIBUSB_TRANSFER_TIMED_OUT => TransferStatus::TimedOut,
            libusb::LIBUSB_TRANSFER_CANCELLED => TransferStatus::Cancelled,
            libusb::LIBUSB_TRANSFER_STALL => TransferStatus::Stall,
            libusb::LIBUSB_TRANSFER_NO_DEVICE => TransferStatus::NoDevice,
            libusb::LIBUSB_TRANSFER_OVERFLOW => TransferStatus::Overflow,
            _ => TransferStatus::Unknown
        }
    }
}

/// A request to transfer data to or from a device.
///
/// An instance of this struct is obtained by calling
/// [DeviceHandle::alloc_transfer](struct.DeviceHandle.html#method.alloc_transfer)
pub struct Transfer {
    // Avoids having the context dropped while this transfer is active
    _context: Arc<ContextAsync>,
    buffer: Vec<u8>,
    transfer: *mut libusb_transfer,
    waker: Mutex<Option<task::Waker>>
}

impl Drop for Transfer
{
    fn drop(&mut self)
    {
        unsafe {
            libusb_free_transfer(self.transfer);
        }
        println!("Dropped");
    }
}

extern "C" fn asyn_callback(libusb_transfer: *mut libusb_transfer)
{
    {
        let waker = {
            let transfer = unsafe {
                Arc::<Transfer>::from_raw((*libusb_transfer).user_data  
                                          as *const Transfer)};
            let w = transfer.waker.lock().unwrap().take();
            w
        };
        // The reference count is decreased at this point.
        // This signals that the transfer is done. 
        if let Some(w) = waker {
            w.wake();
        }
    }
    
    println!("Callback done");
}

impl Transfer {
    /// Prepare a control transfer that writes data to the device
    pub fn fill_control_write(&mut self, request_type: u8, request: u8, 
                              value: u16, index: u16, buf: &[u8])
    {
        
        let buffer = & mut self.buffer;
        buffer.clear();
        buffer.push(request_type);
        buffer.push(request);
        buffer.extend_from_slice(&value.to_le_bytes());
        buffer.extend_from_slice(&index.to_le_bytes());
        buffer.extend_from_slice(
            &u16::try_from(buf.len()).unwrap().to_le_bytes());
        buffer.extend_from_slice(buf);
        
        let transfer = unsafe{&mut *self.transfer};
        transfer.flags = 0;
        transfer.endpoint = 0;
        transfer.transfer_type = libusb::LIBUSB_TRANSFER_TYPE_CONTROL;
        transfer.timeout = 0;
        transfer.length = self.buffer.len() as c_int;
        transfer.buffer = self.buffer.as_mut_ptr() as *mut c_uchar;
        transfer.num_iso_packets = 0;
    }

    /// Prepare a control transfer that reads data from the device
    pub fn fill_control_read(&mut self, request_type: u8, request: u8, 
                             value: u16, index: u16, length: u16)
    {
        
        let buffer = & mut self.buffer;
        buffer.clear();
        buffer.push(request_type);
        buffer.push(request);
        buffer.extend_from_slice(&value.to_le_bytes());
        buffer.extend_from_slice(&index.to_le_bytes());
        buffer.extend_from_slice(&length.to_le_bytes());
        buffer.resize(usize::from(length) + 8, 0);
        
        let transfer = unsafe{&mut *self.transfer};
        transfer.flags = 0;
        transfer.endpoint = 0;
        transfer.transfer_type = libusb::LIBUSB_TRANSFER_TYPE_CONTROL;
        transfer.timeout = 0;
        transfer.length = self.buffer.len() as c_int;
        transfer.buffer = self.buffer.as_mut_ptr() as *mut c_uchar;
        transfer.num_iso_packets = 0;
    }

    /// Prepare a read (IN) transfer from an interrupt endpoint
    pub fn fill_interrupt_read(&mut self, endpoint: u8, length: u16)
    {
        let buffer = & mut self.buffer;
        buffer.clear();
        buffer.resize(usize::from(length), 0);
        
        let transfer = unsafe{&mut *self.transfer};
        transfer.flags = 0;
        transfer.endpoint = endpoint;
        transfer.transfer_type = libusb::LIBUSB_TRANSFER_TYPE_INTERRUPT;
        transfer.timeout = 0;
        transfer.length = self.buffer.len() as c_int;
        transfer.buffer = self.buffer.as_mut_ptr() as *mut c_uchar;
        transfer.num_iso_packets = 0;
    }


    /// Start a transfer request
    ///
    /// The transfer must have been prepared by one of the `fill_*` methods.
    pub fn submit(self) 
                  -> ::Result<TransferFuture>
    {
        unsafe{(*self.transfer).callback = asyn_callback};
        let tarc = Arc::new(self);
        unsafe{(*tarc.transfer).user_data = Arc::into_raw(tarc.clone()) as *mut libc::c_void};
        try_unsafe! {
            libusb_submit_transfer(tarc.transfer)
        };
        Ok(TransferFuture{transfer: Some(tarc)})
    }

    /// Get the status of a completed submit 
    pub fn get_status(&self) -> TransferStatus
    {
        TransferStatus::from(unsafe{(*self.transfer).status})
    }

    /// Get the buffer of a transfer
    ///
    /// Normally only used on a completed transfer to get response data.
    pub fn get_buffer<'a>(&'a self) -> &'a [u8]
    {
        self.buffer.as_ref()
    }
}

impl PartialEq for Transfer
{
    fn eq(&self, other: &Self) -> bool
    {
        self.transfer == other.transfer
    }
}

impl Eq for Transfer
{
}

#[doc(hidden)]
pub unsafe fn from_libusb(context: &Arc<ContextAsync>,
                          transfer: *mut libusb_transfer)
                          -> Transfer
{
    Transfer {
        _context: context.clone(),
        buffer: Vec::new(),
        waker: Mutex::new(None),
        transfer
    }
}

/// Future that is ready when a transfer is finished.
///
/// The result of a successful transfer is a
/// [`Transfer`](struct.Transfer.html) object.

pub struct TransferFuture
{
    transfer: Option<Arc<Transfer>>,
}

impl Drop for TransferFuture
{
    fn drop(&mut self) {
        if self.transfer.is_some() {
            // Cancel transfer if not completed and polled
            unsafe {
                libusb_cancel_transfer(self.transfer.as_ref().unwrap().transfer)
            };
        }
    }
}

impl Future for TransferFuture
{
    type Output = Transfer;
    fn poll(self: Pin<&mut Self>, cx: &mut task::Context)
            -> task::Poll<Self::Output>
    {
        if self.transfer.is_some() {
            if Arc::strong_count(self.as_ref().transfer.as_ref().unwrap())==1 {
                let transfer = self.get_mut().transfer.take().unwrap();
                if let Ok(mut transfer) = Arc::try_unwrap(transfer) {
                    let mut buf_len = 
                        unsafe{(*transfer.transfer).actual_length};
                    if unsafe{(*transfer.transfer).transfer_type} 
                    == libusb::LIBUSB_TRANSFER_TYPE_CONTROL {
                        buf_len += 8;
                    }
                    transfer.buffer.resize(
                        usize::try_from(buf_len).unwrap(),
                        0);
                    
                    return task::Poll::Ready(transfer);
                } else {
                    panic!("Failed to unwrap Arc into Transfer");
                }
            }
            let transfer = self.transfer.as_ref().unwrap();
            let mut waker = transfer.waker.lock().unwrap();
            *waker.deref_mut() = Some(cx.waker().clone());
            task::Poll::Pending
        } else {
            panic!("Future contains no transfer");
        }
    }
}

