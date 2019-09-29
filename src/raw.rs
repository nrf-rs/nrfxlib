//! # Raw Sockets for nrfxlib
//!
//! Generic socket related code.
//!
//! Copyright (c) 42 Technology Ltd 2019
//!
//! Dual-licensed under MIT and Apache 2.0. See the [README](../README.md) for
//! more details.

//******************************************************************************
// Sub-Modules
//******************************************************************************

// None

//******************************************************************************
// Imports
//******************************************************************************

use super::{get_last_error, Error};
use nrfxlib_sys as sys;

//******************************************************************************
// Types
//******************************************************************************

/// Represents a connection to something - either the LTE stack itself, or
/// some remote device.
#[derive(Debug)]
pub struct Socket {
	pub(crate) fd: i32,
}

/// The options that can be passed to a socket.
#[derive(Debug)]
pub(crate) enum SocketOption<'a> {
	/// Set the host name for the TLS certificate to match
	TlsHostName(&'a str),
	/// Pass 1 is you want to verify the peer you are connecting to.
	TlsPeerVerify(i32),
	/// A list of the TLS security/key tags you want to use
	TlsTagList(&'a [u32]),
	/// Defines the interval between each fix in seconds. The default is 1. A
	/// value of 0 means single-fix mode.
	GnssFixInterval(u16),
	/// Defines how long (in seconds) the receiver should try to get a fix.
	/// The default is 60 seconds.
	GnssFixRetry(u16),
	/// Controls which details are provided by the GNSS system
	GnssNmeaMask(u16),
	/// Starts the GNSS system
	GnssStart,
	/// Stops the GNSS system
	GnssStop,
}

/// The domain for a socket
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum SocketDomain {
	/// Corresponds to NRF_AF_LTE. Used for talking to the Nordic LTE modem.
	Lte,
	/// Corresponds to NRF_AF_INET. Used for IPv4 sockets.
	Inet,
	/// Corresponds to NRF_AF_LOCAL. Used for talking to the Nordic library (e.g. GNSS functions).
	Local,
}

/// The type of socket (Stream, Datagram, or neither)
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum SocketType {
	/// Used with `SocketDomain::Lte`
	None,
	/// Used with `SocketDomain::Inet` for TCP and TLS streams
	Stream,
	/// Used with UDP sockets, and for GPS
	Datagram,
}

/// The protocol used on this socket.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum SocketProtocol {
	/// Used with `SocketDomain::Lte`
	At,
	/// Plain TCP socket
	Tcp,
	/// A TLS v1.2 stream
	Tls1v2,
	/// A connection to the GPS/GNSS sub-system
	Gnss,
}

//******************************************************************************
// Constants
//******************************************************************************

// None

//******************************************************************************
// Global Variables
//******************************************************************************

// None

//******************************************************************************
// Macros
//******************************************************************************

// None

//******************************************************************************
// Public Functions and Impl on Public Types
//******************************************************************************

impl Socket {
	pub(crate) fn new(
		domain: SocketDomain,
		skt_type: SocketType,
		protocol: SocketProtocol,
	) -> Result<Socket, Error> {
		let result = unsafe { sys::nrf_socket(domain.into(), skt_type.into(), protocol.into()) };
		if result < 0 {
			Err(Error::Nordic("new_socket", result, get_last_error()))
		} else {
			Ok(Socket { fd: result })
		}
	}

	pub(crate) fn set_option<'a>(&'a self, option: SocketOption<'a>) -> Result<(), Error> {
		let length = option.get_length();
		let result = unsafe {
			sys::nrf_setsockopt(
				self.fd,
				option.get_level(),
				option.get_name(),
				option.get_value(),
				length,
			)
		};
		if result < 0 {
			Err(Error::Nordic("set_option", result, get_last_error()))
		} else {
			Ok(())
		}
	}

	/// Perform a blocking write on the socket.
	pub fn write(&self, buf: &[u8]) -> Result<usize, Error> {
		let length = buf.len();
		let ptr = buf.as_ptr();
		let result = unsafe { sys::nrf_write(self.fd, ptr as *const _, length) };
		if result < 0 {
			Err(Error::Nordic("write", result as i32, get_last_error()))
		} else {
			Ok(result as usize)
		}
	}

	/// Perform a non-blocking read on the socket. Will fill up none, some or
	/// all of the given buffer. You must slice the buffer using the returned
	/// `usize` value.
	pub fn recv(&self, buf: &mut [u8]) -> Result<Option<usize>, Error> {
		let length = buf.len();
		let ptr = buf.as_mut_ptr();
		let result =
			unsafe { sys::nrf_recv(self.fd, ptr as *mut _, length, sys::NRF_MSG_DONTWAIT as i32) };
		if result == -1 && get_last_error() == sys::NRF_EAGAIN as i32 {
			// This is EAGAIN
			Ok(None)
		} else if result < 0 {
			Err(Error::Nordic("recv", result as i32, get_last_error()))
		} else {
			Ok(Some(result as usize))
		}
	}

	/// Perform a blocking read on the socket. Will fill up some or all of the
	/// given buffer. You must slice the buffer using the returned `usize`
	/// value.
	pub fn recv_wait(&self, buf: &mut [u8]) -> Result<usize, Error> {
		let length = buf.len();
		let ptr = buf.as_mut_ptr();
		let result = unsafe { sys::nrf_recv(self.fd, ptr as *mut _, length, 0) };
		if result < 0 {
			Err(Error::Nordic("recv_wait", result as i32, get_last_error()))
		} else {
			Ok(result as usize)
		}
	}
}

impl core::fmt::Write for Socket {
	fn write_str(&mut self, s: &str) -> core::fmt::Result {
		match self.write(s.as_bytes()) {
			Ok(_n) => Ok(()),
			Err(_e) => Err(core::fmt::Error),
		}
	}
}

impl Drop for Socket {
	fn drop(&mut self) {
		unsafe {
			let _ = sys::nrf_close(self.fd);
		}
	}
}

impl<'a> SocketOption<'a> {
	pub(crate) fn get_level(&self) -> i32 {
		match self {
			SocketOption::TlsHostName(_) => sys::NRF_SOL_SECURE as i32,
			SocketOption::TlsPeerVerify(_) => sys::NRF_SOL_SECURE as i32,
			SocketOption::TlsTagList(_) => sys::NRF_SOL_SECURE as i32,
			SocketOption::GnssFixInterval(_) => sys::NRF_SOL_GNSS as i32,
			SocketOption::GnssFixRetry(_) => sys::NRF_SOL_GNSS as i32,
			SocketOption::GnssNmeaMask(_) => sys::NRF_SOL_GNSS as i32,
			SocketOption::GnssStart => sys::NRF_SOL_GNSS as i32,
			SocketOption::GnssStop => sys::NRF_SOL_GNSS as i32,
		}
	}

	pub(crate) fn get_name(&self) -> i32 {
		match self {
			SocketOption::TlsHostName(_) => sys::NRF_SO_HOSTNAME as i32,
			SocketOption::TlsPeerVerify(_) => sys::NRF_SO_SEC_PEER_VERIFY as i32,
			SocketOption::TlsTagList(_) => sys::NRF_SO_SEC_TAG_LIST as i32,
			SocketOption::GnssFixInterval(_) => sys::NRF_SO_GNSS_FIX_INTERVAL as i32,
			SocketOption::GnssFixRetry(_) => sys::NRF_SO_GNSS_FIX_RETRY as i32,
			SocketOption::GnssNmeaMask(_) => sys::NRF_SO_GNSS_NMEA_MASK as i32,
			SocketOption::GnssStart => sys::NRF_SO_GNSS_START as i32,
			SocketOption::GnssStop => sys::NRF_SO_GNSS_STOP as i32,
		}
	}

	pub(crate) fn get_value(&self) -> *const sys::ctypes::c_void {
		match self {
			SocketOption::TlsHostName(s) => s.as_ptr() as *const _,
			SocketOption::TlsPeerVerify(x) => x as *const i32 as *const _,
			SocketOption::TlsTagList(x) => x.as_ptr() as *const _,
			SocketOption::GnssFixInterval(x) => x as *const u16 as *const _,
			SocketOption::GnssFixRetry(x) => x as *const u16 as *const _,
			SocketOption::GnssNmeaMask(x) => x as *const u16 as *const _,
			SocketOption::GnssStart => core::ptr::null(),
			SocketOption::GnssStop => core::ptr::null(),
		}
	}

	pub(crate) fn get_length(&self) -> u32 {
		match self {
			SocketOption::TlsHostName(s) => s.len() as u32,
			SocketOption::TlsPeerVerify(x) => core::mem::size_of_val(x) as u32,
			SocketOption::TlsTagList(x) => core::mem::size_of_val(x) as u32,
			SocketOption::GnssFixInterval(x) => core::mem::size_of_val(x) as u32,
			SocketOption::GnssFixRetry(x) => core::mem::size_of_val(x) as u32,
			SocketOption::GnssNmeaMask(x) => core::mem::size_of_val(x) as u32,
			SocketOption::GnssStart => 0u32,
			SocketOption::GnssStop => 0u32,
		}
	}
}

impl Into<i32> for SocketDomain {
	fn into(self) -> i32 {
		use SocketDomain::*;
		match self {
			Local => sys::NRF_AF_LOCAL as i32,
			Lte => sys::NRF_AF_LTE as i32,
			Inet => sys::NRF_AF_INET as i32,
		}
	}
}

impl Into<i32> for SocketType {
	fn into(self) -> i32 {
		use SocketType::*;
		match self {
			None => 0,
			Stream => sys::NRF_SOCK_STREAM as i32,
			Datagram => sys::NRF_SOCK_DGRAM as i32,
		}
	}
}

impl Into<i32> for SocketProtocol {
	fn into(self) -> i32 {
		use SocketProtocol::*;
		match self {
			At => sys::NRF_PROTO_AT as i32,
			Tcp => sys::NRF_IPPROTO_TCP as i32,
			Tls1v2 => sys::NRF_SPROTO_TLS1v2 as i32,
			Gnss => sys::NRF_PROTO_GNSS as i32,
		}
	}
}

//******************************************************************************
// Private Functions and Impl on Private Types
//******************************************************************************

pub(crate) fn htons(input: u16) -> u16 {
	let top: u16 = (input >> 8) & 0xFF;
	let bottom: u16 = (input >> 0) & 0xFF;
	(bottom << 8) | top
}

//******************************************************************************
// End of File
//******************************************************************************
