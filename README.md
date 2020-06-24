# Libusb-rs-async
This is a fork of the [libusb-rs library](http://dcuddeback.github.io/libusb-rs).
This version of the library has been extended to support asynchronous operations
using futures. So far only control tranfers and interrupt in is supported.
Most of the documentation for libusb-rs is still relevant for this library.

The main difference is that you can get a Transfer object from DeviceHandle::alloc_transfer that you can use to build and submit requests.
You will need some kind of runtime to actually use the asynchronous features, e.g. [Tokio](https://github.com/tokio-rs/tokio)
