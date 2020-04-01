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
	/// 0 implies no peer verification. 1 implies peer verification is
	/// optional. 2 implies peer verification is strict (mandatory).
	TlsPeerVerify(sys::nrf_sec_peer_verify_t),
	/// 0 implies no TLS credential caching. 1 implies caching.
	TlsSessionCache(sys::nrf_sec_session_cache_t),
	/// A list of the TLS security/key tags you want to use
	TlsTagList(&'a [sys::nrf_sec_tag_t]),
	/// Defines the interval between each fix in seconds. The default is 1. A
	/// value of 0 means single-fix mode.
	GnssFixInterval(sys::nrf_gnss_fix_interval_t),
	/// Defines how long (in seconds) the receiver should try to get a fix.
	/// The default is 60 seconds. 0 means wait forever.
	GnssFixRetry(sys::nrf_gnss_fix_retry_t),
	/// Controls which, if any, NMEA frames are provided by the GNSS system
	GnssNmeaMask(sys::nrf_gnss_nmea_mask_t),
	/// Starts the GNSS system, after deleting the specified non-volatile values.
	GnssStart(sys::nrf_gnss_delete_mask_t),
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
	/// Plain TCP stream socket
	Tcp,
	/// Plain UDP datagram socket
	Udp,
	/// A TLS v1.2 over TCP stream socket
	Tls1v2,
	/// A TLS v1.3 over TCP stream socket
	Tls1v3,
	/// A DTLS v1.2 over UDP datagram socket
	Dtls1v2,
	/// A connection to the GPS/GNSS sub-system
	Gnss,
}

/// Describes something we can poll on.
pub trait Pollable {
	#[doc(hidden)]
	/// Get the underlying socket ID for this socket.
	fn get_fd(&self) -> i32;
}

/// Describes a socket you wish to poll, and the result of polling it.
pub struct PollEntry<'a> {
	socket: &'a dyn Pollable,
	flags: PollFlags,
	result: PollResult,
}

/// The ways in which you can poll on a particular socket
#[derive(Debug, Copy, Clone)]
#[repr(i16)]
pub enum PollFlags {
	/// Wake up if this socket is readable
	Read = sys::NRF_POLLIN as i16,
	/// Wake up if this socket is writeable
	Write = sys::NRF_POLLOUT as i16,
	/// Wake up if this socket is readable or writeable
	ReadOrWrite = sys::NRF_POLLIN as i16 + sys::NRF_POLLOUT as i16,
}

/// The ways a socket can respond to a poll.
#[derive(Debug, Copy, Clone)]
pub struct PollResult(u32);

//******************************************************************************
// Constants
//******************************************************************************

const MAX_SOCKETS_POLL: usize = 8;

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
		let result = unsafe { sys::nrf_write(self.fd, ptr as *const _, length as u32) };
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
		let result = unsafe {
			sys::nrf_recv(
				self.fd,
				ptr as *mut _,
				length as u32,
				sys::NRF_MSG_DONTWAIT as i32,
			)
		};
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
		let result = unsafe { sys::nrf_recv(self.fd, ptr as *mut _, length as u32, 0) };
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
			SocketOption::TlsSessionCache(_) => sys::NRF_SOL_SECURE as i32,
			SocketOption::TlsTagList(_) => sys::NRF_SOL_SECURE as i32,
			SocketOption::GnssFixInterval(_) => sys::NRF_SOL_GNSS as i32,
			SocketOption::GnssFixRetry(_) => sys::NRF_SOL_GNSS as i32,
			SocketOption::GnssNmeaMask(_) => sys::NRF_SOL_GNSS as i32,
			SocketOption::GnssStart(_) => sys::NRF_SOL_GNSS as i32,
			SocketOption::GnssStop => sys::NRF_SOL_GNSS as i32,
		}
	}

	pub(crate) fn get_name(&self) -> i32 {
		match self {
			SocketOption::TlsHostName(_) => sys::NRF_SO_HOSTNAME as i32,
			SocketOption::TlsPeerVerify(_) => sys::NRF_SO_SEC_PEER_VERIFY as i32,
			SocketOption::TlsSessionCache(_) => sys::NRF_SO_SEC_SESSION_CACHE as i32,
			SocketOption::TlsTagList(_) => sys::NRF_SO_SEC_TAG_LIST as i32,
			SocketOption::GnssFixInterval(_) => sys::NRF_SO_GNSS_FIX_INTERVAL as i32,
			SocketOption::GnssFixRetry(_) => sys::NRF_SO_GNSS_FIX_RETRY as i32,
			SocketOption::GnssNmeaMask(_) => sys::NRF_SO_GNSS_NMEA_MASK as i32,
			SocketOption::GnssStart(_) => sys::NRF_SO_GNSS_START as i32,
			SocketOption::GnssStop => sys::NRF_SO_GNSS_STOP as i32,
		}
	}

	pub(crate) fn get_value(&self) -> *const sys::ctypes::c_void {
		match self {
			SocketOption::TlsHostName(s) => s.as_ptr() as *const sys::ctypes::c_void,
			SocketOption::TlsPeerVerify(x) => x as *const _ as *const sys::ctypes::c_void,
			SocketOption::TlsSessionCache(x) => x as *const _ as *const sys::ctypes::c_void,
			SocketOption::TlsTagList(x) => x.as_ptr() as *const sys::ctypes::c_void,
			SocketOption::GnssFixInterval(x) => x as *const _ as *const sys::ctypes::c_void,
			SocketOption::GnssFixRetry(x) => x as *const _ as *const sys::ctypes::c_void,
			SocketOption::GnssNmeaMask(x) => x as *const _ as *const sys::ctypes::c_void,
			SocketOption::GnssStart(x) => x as *const _ as *const sys::ctypes::c_void,
			SocketOption::GnssStop => core::ptr::null(),
		}
	}

	pub(crate) fn get_length(&self) -> u32 {
		match self {
			SocketOption::TlsHostName(s) => s.len() as u32,
			SocketOption::TlsPeerVerify(x) => core::mem::size_of_val(x) as u32,
			SocketOption::TlsSessionCache(x) => core::mem::size_of_val(x) as u32,
			SocketOption::TlsTagList(x) => core::mem::size_of_val(x) as u32,
			SocketOption::GnssFixInterval(x) => core::mem::size_of_val(x) as u32,
			SocketOption::GnssFixRetry(x) => core::mem::size_of_val(x) as u32,
			SocketOption::GnssNmeaMask(x) => core::mem::size_of_val(x) as u32,
			SocketOption::GnssStart(x) => core::mem::size_of_val(x) as u32,
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
			Udp => sys::NRF_IPPROTO_UDP as i32,
			Tls1v2 => sys::NRF_SPROTO_TLS1v2 as i32,
			Tls1v3 => sys::NRF_SPROTO_TLS1v3 as i32,
			Dtls1v2 => sys::NRF_SPROTO_DTLS1v2 as i32,
			Gnss => sys::NRF_PROTO_GNSS as i32,
		}
	}
}

impl PollResult {
	/// Is polled socket now readable?
	pub fn is_readable(&self) -> bool {
		(self.0 & sys::NRF_POLLIN) != 0
	}

	/// Is polled socket now writeable?
	pub fn is_writable(&self) -> bool {
		(self.0 & sys::NRF_POLLOUT) != 0
	}

	/// Is polled socket now in an error state?
	pub fn is_errored(&self) -> bool {
		(self.0 & sys::NRF_POLLERR) != 0
	}

	/// Is polled socket now closed?
	pub fn is_closed(&self) -> bool {
		(self.0 & sys::NRF_POLLHUP) != 0
	}

	/// Was polled socket closed before we polled it?
	pub fn was_not_open(&self) -> bool {
		(self.0 & sys::NRF_POLLNVAL) != 0
	}
}

impl Default for PollResult {
	fn default() -> PollResult {
		PollResult(0)
	}
}

impl Pollable for Socket {
	/// Get the underlying socket ID for this socket.
	fn get_fd(&self) -> i32 {
		self.fd
	}
}

impl<'a> PollEntry<'a> {
	/// Create a new `PollEntry` - you need a socket to poll, and what you want
	/// to poll it for.
	pub fn new(socket: &'a dyn Pollable, flags: PollFlags) -> PollEntry {
		PollEntry {
			socket,
			flags,
			result: PollResult::default(),
		}
	}

	/// Get the result of polling this socket.
	pub fn result(&self) -> PollResult {
		self.result
	}
}

/// Poll on multiple sockets at once.
///
/// For example:
///
/// ```ignore
/// use nrfxlib::{at::AtSocket, gnss::GnssSocket, Pollable, PollFlags, PollResult};
/// let mut socket1 = AtSocket::new();
/// let mut socket2 = GnssSocket::new();
/// let mut poll_list = [
/// 	PollEntry::new(&mut socket1, PollFlags::Read),
/// 	PollEntry::new(&mut socket2, PollFlags::Read),
/// ];
/// match nrfxlib::poll(&mut poll_list, 100) {
/// 	Ok(0) => {
///		// Timeout
/// 	}
/// 	Ok(n) => {
///		// One of the sockets is ready. See `poll_list[n].result()`.
/// 	}
/// 	Err(e) => {
///		// An error occurred
/// 	}
/// }
/// ```
pub fn poll(poll_list: &mut [PollEntry], timeout_ms: u16) -> Result<i32, Error> {
	let mut count = 0;

	if poll_list.len() > MAX_SOCKETS_POLL {
		return Err(Error::TooManySockets);
	}

	let mut poll_fds: [sys::nrf_pollfd; MAX_SOCKETS_POLL] = [sys::nrf_pollfd {
		handle: 0,
		requested: 0,
		returned: 0,
	}; MAX_SOCKETS_POLL];

	for (poll_entry, pollfd) in poll_list.iter_mut().zip(poll_fds.iter_mut()) {
		pollfd.handle = poll_entry.socket.get_fd();
		pollfd.requested = poll_entry.flags as i16;
		count += 1;
	}

	let result = unsafe { sys::nrf_poll(poll_fds.as_mut_ptr(), count, timeout_ms as i32) };

	match result {
		-1 => Err(Error::Nordic("poll", -1, get_last_error())),
		0 => Ok(0),
		n => {
			for (poll_entry, pollfd) in poll_list.iter_mut().zip(poll_fds.iter()) {
				poll_entry.result = PollResult(pollfd.returned as u32);
			}
			Ok(n)
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
