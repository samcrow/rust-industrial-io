// libiio-sys/src/context.rs
//
// Copyright (c) 2018, Frank Pagliughi
//
// Licensed under the MIT license:
//   <LICENSE or http://opensource.org/licenses/MIT>
// This file may not be copied, modified, or distributed except according
// to those terms.
//
//! Industrial I/O Contexts.
//!

use std::time::Duration;
use std::ffi::CString;
use std::os::raw::c_uint;
use std::rc::Rc;

use nix::errno::{Errno};
use nix::Error::Sys as SysError;

use ffi;
use super::*;

/// An Industrial I/O Context
///
/// Since the IIO library isn't thread safe, this object cannot be Send or
/// Sync.
///
/// This object maintains a reference counted pointer to the context object
/// of the underlying library's iio_context object. Once all references to
/// the Context object have been dropped, the underlying iio_context will be
/// destroyed. This is done to make creation and use of a single Device more
/// ergonomic by removing the need to manage the lifetime of the Context.
#[derive(Debug,Clone)]
pub struct Context {
    inner: Rc<InnerContext>,
}

/// This holds a pointer to the library context.
/// When it is dropped, the library context is destroyed.
#[derive(Debug)]
struct InnerContext {
    pub(crate) ctx: *mut ffi::iio_context
}

impl Drop for InnerContext {
    fn drop(&mut self) {
        unsafe { ffi::iio_context_destroy(self.ctx) };
    }
}

impl Context {
    /// Creates a default context from a local or remote IIO device.
    ///
    /// @note This will create a network context if the IIOD_REMOTE
    /// environment variable is set to the hostname where the IIOD server
    /// runs. If set to an empty string, the server will be discovered using
    /// ZeroConf. If the environment variable is not set, a local context
    /// will be created instead.
    pub fn new() -> Result<Context> {
        let ctx = unsafe { ffi::iio_create_default_context() };
        if ctx.is_null() { bail!(SysError(Errno::last())); }
        Ok(Context { inner: Rc::new(InnerContext{ ctx }) })
    }

    /// Tries to create a context from the specified URI
    pub fn from_uri(uri: &str) -> Result<Context> {
        let uri = match CString::new(uri) {
            Ok(v) => v,
            Err(_e) => bail!("Can't create context from URI {}", uri),
        };
        let ctx = unsafe {
            ffi::iio_create_context_from_uri(uri.as_ptr())
        };
        if ctx.is_null() { bail!(SysError(Errno::last())); }
        Ok(Context { inner: Rc::new(InnerContext{ ctx }) })
    }


    /// Creates a context from a URI
    ///
    /// This can create a local, network, or XML context as specified by
    /// the URI using the preambles:
    ///   * "local:"  - a local context
    ///   * "xml:"  - an xml (file) context
    ///   * "ip:"  - a network context
    ///   * "usb:"  - a USB backend
    ///   * "serial:"  - a serial backend
    pub fn create_from_uri(uri: &str) -> Result<Context> {
        let uri = CString::new(uri)?;
        let ctx = unsafe { ffi::iio_create_context_from_uri(uri.as_ptr()) };
        if ctx.is_null() { bail!(SysError(Errno::last())); }
        Ok(Context { inner: Rc::new(InnerContext{ ctx }) })
    }

    /// Creates a context from a local device (Linux only)
    #[cfg(target_os = "linux")]
    pub fn create_local() -> Result<Context> {
        let ctx = unsafe { ffi::iio_create_local_context() };
        if ctx.is_null() { bail!(SysError(Errno::last())); }
        Ok(Context { inner: Rc::new(InnerContext{ ctx }) })
    }

    /// Creates a context from a network device
    pub fn create_network(host: &str) -> Result<Context> {
        let host = CString::new(host)?;
        let ctx = unsafe { ffi::iio_create_network_context(host.as_ptr()) };
        if ctx.is_null() { bail!(SysError(Errno::last())); }
        Ok(Context { inner: Rc::new(InnerContext{ ctx }) })
    }

    /// Creates a context from an XML file
    pub fn create_xml(xml_file: &str) -> Result<Context> {
        let xml_file = CString::new(xml_file)?;
        let ctx = unsafe { ffi::iio_create_xml_context(xml_file.as_ptr()) };
        if ctx.is_null() { bail!(SysError(Errno::last())); }
        Ok(Context { inner: Rc::new(InnerContext{ ctx }) })
    }

    /// Creates a context from a XML data in memory
    pub fn create_xml_mem(xml: &str) -> Result<Context> {
        let n = xml.len();
        let xml = CString::new(xml)?;
        let ctx = unsafe { ffi::iio_create_xml_context_mem(xml.as_ptr(), n) };
        if ctx.is_null() { bail!(SysError(Errno::last())); }
        Ok(Context { inner: Rc::new(InnerContext{ ctx }) })
    }


    /// Get a description of the context
    pub fn description(&self) -> String {
        let pstr = unsafe { ffi::iio_context_get_description(self.inner.ctx) };
        cstring_opt(pstr).unwrap_or_default()
    }

    /// Gets the number of context-specific attributes
    pub fn num_attrs(&self) -> usize {
        let n = unsafe { ffi::iio_context_get_attrs_count(self.inner.ctx) };
        n as usize
    }

    /// Sets the timeout for I/O operations
    ///
    /// `timeout` The timeout. A value of zero specifies that no timeout
    /// should be used.
    pub fn set_timeout(&mut self, timeout: Duration) -> Result<()> {
        let timeout_ms: u64 = 1000 * timeout.as_secs() + u64::from(timeout.subsec_millis());
        let ret = unsafe { ffi::iio_context_set_timeout(self.inner.ctx, timeout_ms as c_uint) };
        if ret < 0 { bail!(SysError(Errno::last())); }
        Ok(())
    }

    /// Get the number of devices in the context
    pub fn num_devices(&self) -> usize {
        let n = unsafe { ffi::iio_context_get_devices_count(self.inner.ctx) };
        n as usize
    }

    /// Gets a device by index
    pub fn get_device(&self, idx: usize) -> Result<Device> {
        let dev = unsafe { ffi::iio_context_get_device(self.inner.ctx, idx as c_uint) };
        if dev.is_null() { bail!("Index out of range"); }
        Ok(Device { dev, ctx: self.clone() })
    }

    /// Try to find a device by name or ID
    /// `name` The name or ID of the device to find
    pub fn find_device(&self, name: &str) -> Option<Device> {
        let name = CString::new(name).unwrap();
        let dev = unsafe { ffi::iio_context_find_device(self.inner.ctx, name.as_ptr()) };
        if dev.is_null() {
            None
        }
        else {
            Some(Device { dev, ctx: self.clone() })
        }
    }

    /// Gets an iterator for all the devices in the context.
    pub fn devices(&self) -> DeviceIterator {
        DeviceIterator {
            ctx: self,
            idx: 0,
        }
    }

    /// Destroy the context
    ///
    /// This consumes the context to destroy the instance.
    pub fn destroy(self) {}
}

impl PartialEq for Context {
    /// Two contexts are the same if they refer to the same underlying
    /// object in the library.
    fn eq(&self, other: &Context) -> bool {
        self.inner.ctx == other.inner.ctx
    }
}

pub struct DeviceIterator<'a> {
    ctx: &'a Context,
    idx: usize,
}

impl<'a> Iterator for DeviceIterator<'a> {
    type Item = Device;

    fn next(&mut self) -> Option<Self::Item> {
        match self.ctx.get_device(self.idx) {
            Ok(dev) => {
                self.idx += 1;
                Some(dev)
            },
            Err(_) => None
        }
    }
}

/*
    TODO: We need to implement a context::get_attr()
    before we can add this.

pub struct AttrIterator<'a> {
    ctx: &'a Context,
    idx: usize,
}

impl<'a> Iterator for AttrIterator<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        match self.ctx.get_attr(self.idx) {
            Ok(name) => {
                self.idx += 1;
                Some(name)
            },
            Err(_) => None
        }
    }
}
*/

