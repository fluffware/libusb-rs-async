extern crate libusb_async as libusb;
extern crate futures;
use libusb::*;
use std::thread;
use std::time::Duration;
 use std::convert::TryInto;
use futures::executor::block_on;
fn main()
{
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        println!("usage: read_async <vendor-id-in-hex> <product-id-in-hex>");
        return;
    }
    
    let vid = u16::from_str_radix(args[1].as_ref(), 16).unwrap();
    let pid = u16::from_str_radix(args[2].as_ref(), 16).unwrap();

    match libusb::Context::new() {
        Ok(mut context) => {
            match open_device(&mut context, vid, pid) {
                Some((device, _, mut handle)) => {
                    // Find an interrupt IN endpoint
                    let config_value = handle.active_configuration().unwrap();
                    let config =
                        device.config_descriptor_by_value(config_value)
                        .expect("No config descriptor found");
                    let ep_intf = config.interfaces().find_map(|intf| {
                        let intf_descr = intf.descriptors().next();
                        if let Some(intf_descr) = intf_descr {
                            match intf_descr.endpoint_descriptors().find(|ep| {
                                ep.transfer_type() == TransferType::Interrupt
                                    && ep.direction() == Direction::In
                            }) {
                                Some(ep) => Some((intf.number(),ep.address())),
                                None => None
                            }
                        } else {
                            None
                        }
                    });

                    /*
                    let mut trans = handle.alloc_transfer(0).unwrap();
                    trans.fill_control_write(
                        request_type(Direction::Out,
                                     RequestType::Standard,
                                     Recipient::Device),
                        0x09,
                        0,
                        0,
                        &[]);

                    
                    let submit = trans.submit().unwrap();
                    let res = block_on(submit);
                    println!("Result status: {}", res.get_status());
                     */
                    
                    let mut trans = handle.alloc_transfer(0).unwrap();
                    // Get string descriptor 1
                    trans.fill_control_read(
                        request_type(Direction::In,
                                     RequestType::Standard,
                                     Recipient::Device),
                        0x06,
                        0x0301,
                        0x0409,
                        100);
                    
                    
                    let submit = trans.submit().unwrap();
                    let res = block_on(submit);
                    match res.get_status() {
                        TransferStatus::Completed => {
                            let b = res.get_buffer();
                            if b.len() >= 12 {
                                let name_utf16 = b[10..].chunks(2).map({
                                    |chunk| u16::from_le_bytes(chunk.try_into().unwrap())
                                }).collect::<Vec<u16>>();
                                let name = String::from_utf16(&name_utf16).unwrap();
                                println!("String 1: {}", name);
                            } else {
                                println!("No string descriptor");
                            }
                        },
                        s => println!("Result status: {}", s)
                    }

                    if let Some((intf,ep)) = ep_intf {
                        println!("Using interface {}, endpoint: {}", intf, ep);
                        if handle.kernel_driver_active(intf).unwrap_or(false) {
                            handle.detach_kernel_driver(intf).unwrap();
                        }
                        handle.claim_interface(intf).unwrap();
                        loop {
                            let mut trans = handle.alloc_transfer(0).unwrap();
                            
                            trans.fill_interrupt_read(ep, 8);
                            
                            
                            let submit = trans.submit().unwrap();
                            let res = block_on(submit);
                            match res.get_status() {
                                TransferStatus::Completed => {
                                    println!("Interrupt in: {:?}", res.get_buffer());
                                },
                                s => println!("Result status: {}", s)
                            }
                        }
                    }
                    
                },
                None => println!("could not find device {:04x}:{:04x}", vid, pid)
            }
        },
        Err(e) => panic!("could not initialize libusb: {}", e)
    }
}

fn open_device(context: &mut libusb::Context, vid: u16, pid: u16) -> Option<(libusb::Device, libusb::DeviceDescriptor, libusb::DeviceHandle)> {
    let devices = match context.devices() {
        Ok(d) => d,
        Err(_) => return None
    };

    for device in devices.iter() {
        let device_desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue
        };

        if device_desc.vendor_id() == vid && device_desc.product_id() == pid {
            match device.open() {
                Ok(handle) => return Some((device, device_desc, handle)),
                Err(_) => continue
            }
        }
    }

    None
}
