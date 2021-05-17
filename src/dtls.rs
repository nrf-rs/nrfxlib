//! # DTLS Sockets for nrfxlib
//!
//! DTLS (encrypted UDP) socket related code.
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

pub use crate::tls::provision_certificates;

use super::{get_last_error, Error};
use crate::raw::*;
use log::debug;
use nrfxlib_sys as sys;

//******************************************************************************
// Types
//******************************************************************************

/// Represents a connection to a remote TCP/IP device using DTLS over UDP.
#[derive(Debug)]
pub struct DtlsSocket {
	socket: Socket,
}

/// Specify which version of the DTLS standard to use
#[derive(Debug, Copy, Clone)]
pub enum Version {
	/// DTLS v1.2
	Dtls1v2,
}

/// Specify whether to verify the peer
#[derive(Debug, Copy, Clone)]
pub enum PeerVerification {
	/// Yes - check the peer's certificate is valid and abort if it isn't
	Enabled,
	/// Maybe - check the peer's certificate is valid but don't abort if it isn't
	Optional,
	/// No - do not validate the peer's certificate. Using this option leaves
	/// you vulnerable to man-in-the-middle attacks.
	Disabled,
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

impl DtlsSocket {
	/// Create a new TLS socket. Only supports TLS v1.2/1.3 and IPv4 at the moment.
	pub fn new(
		peer_verify: PeerVerification,
		security_tags: &[u32],
		version: Version,
	) -> Result<DtlsSocket, Error> {
		let nrf_dtls_version = match version {
			Version::Dtls1v2 => SocketProtocol::Dtls1v2,
		};

		let socket = Socket::new(SocketDomain::Inet, SocketType::Datagram, nrf_dtls_version)?;

		// Now configure this socket

		// Set whether we verify the peer
		socket.set_option(SocketOption::TlsPeerVerify(peer_verify.as_integer()))?;

		// Always enable session caching to speed up connecting. 0 = enabled, 1
		// = disabled (the default).
		socket.set_option(SocketOption::TlsSessionCache(0))?;

		// We don't set the cipher list, and assume the defaults are sensible.

		if !security_tags.is_empty() {
			// Configure the socket to use the pre-stored certificates. See
			// `provision_certificates`.
			socket.set_option(SocketOption::TlsTagList(security_tags))?;
		}

		Ok(DtlsSocket { socket })
	}

	/// Look up the hostname and for each result returned, try to connect to
	/// it.
	pub fn connect(&self, hostname: &str, port: u16) -> Result<(), Error> {
		use core::fmt::Write;

		debug!("Connecting via DTLS to {}:{}", hostname, port);

		// First we set the hostname
		self.socket
			.set_option(SocketOption::TlsHostName(hostname))?;

		let mut result;
		// Now, make a null-terminated hostname
		let mut hostname_smallstring: heapless::String<64> = heapless::String::new();
		write!(hostname_smallstring, "{}\0", hostname).map_err(|_| Error::HostnameTooLong)?;
		// Now call getaddrinfo with some hints
		let hints = sys::nrf_addrinfo {
			ai_flags: 0,
			ai_family: sys::NRF_AF_INET as i32,
			ai_socktype: sys::NRF_SOCK_DGRAM as i32,
			ai_protocol: 0,
			ai_addrlen: 0,
			ai_addr: core::ptr::null_mut(),
			ai_canonname: core::ptr::null_mut(),
			ai_next: core::ptr::null_mut(),
		};
		let mut output_ptr: *mut sys::nrf_addrinfo = core::ptr::null_mut();
		result = unsafe {
			sys::nrf_getaddrinfo(
				// hostname
				hostname_smallstring.as_ptr(),
				// service
				core::ptr::null(),
				// hints
				&hints,
				// output pointer
				&mut output_ptr,
			)
		};

		if (result != 0) && output_ptr.is_null() {
			return Err(Error::Nordic("dtls_dns", result, get_last_error()));
		} else {
			let mut record: &sys::nrf_addrinfo = unsafe { &*output_ptr };
			loop {
				let dns_addr: &sys::nrf_sockaddr_in =
					unsafe { &*(record.ai_addr as *const sys::nrf_sockaddr_in) };
				// Create a new sockaddr_in with the right port
				let connect_addr = sys::nrf_sockaddr_in {
					sin_len: core::mem::size_of::<sys::nrf_sockaddr_in>() as u8,
					sin_family: sys::NRF_AF_INET as i32,
					sin_port: htons(port),
					sin_addr: dns_addr.sin_addr.clone(),
				};

				debug!("Trying IP address {}", &crate::NrfSockAddrIn(connect_addr));

				// try and connect to this result
				result = unsafe {
					sys::nrf_connect(
						self.socket.fd,
						&connect_addr as *const sys::nrf_sockaddr_in as *const _,
						connect_addr.sin_len as u32,
					)
				};
				if result == 0 {
					break;
				}
				if !record.ai_next.is_null() {
					record = unsafe { &*record.ai_next };
				} else {
					break;
				}
			}
			unsafe {
				sys::nrf_freeaddrinfo(output_ptr);
			}
		}
		if result != 0 {
			Err(Error::Nordic("dtls_connect", result, get_last_error()))
		} else {
			Ok(())
		}
	}
}

impl Pollable for DtlsSocket {
	/// Get the underlying socket ID for this socket.
	fn get_fd(&self) -> i32 {
		self.socket.fd
	}
}

impl core::ops::DerefMut for DtlsSocket {
	fn deref_mut(&mut self) -> &mut Socket {
		&mut self.socket
	}
}

impl core::ops::Deref for DtlsSocket {
	type Target = Socket;
	fn deref(&self) -> &Socket {
		&self.socket
	}
}

impl PeerVerification {
	/// The NRF library wants peer verification as a integer, so this function
	/// converts as per `sys::nrf_sec_peer_verify_t`.
	fn as_integer(self) -> u32 {
		match self {
			PeerVerification::Enabled => 2,
			PeerVerification::Optional => 1,
			PeerVerification::Disabled => 0,
		}
	}
}

//******************************************************************************
// Private Functions and Impl on Private Types
//******************************************************************************

// None

//******************************************************************************
// End of File
//******************************************************************************
