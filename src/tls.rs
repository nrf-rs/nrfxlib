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

use super::{get_last_error, Error};
use crate::raw::*;
use log::debug;
use nrfxlib_sys as sys;

//******************************************************************************
// Imports
//******************************************************************************

// None

//******************************************************************************
// Types
//******************************************************************************

/// Represents a connection to a remote TCP/IP device using TLS.
#[derive(Debug)]
pub struct TlsSocket {
	socket: Socket,
}

/// Specify which version of the TLS standard to use
#[derive(Debug, Copy, Clone)]
pub enum Version {
	/// TLS v1.2
	Tls1v2,
	/// TLS v1.3
	Tls1v3,
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

impl TlsSocket {
	/// Create a new TLS socket. Only supports TLS v1.2/1.3 and IPv4 at the moment.
	pub fn new(
		peer_verify: PeerVerification,
		security_tags: &[u32],
		version: Version,
	) -> Result<TlsSocket, Error> {
		let nrf_tls_version = match version {
			Version::Tls1v2 => SocketProtocol::Tls1v2,
			Version::Tls1v3 => SocketProtocol::Tls1v3,
		};

		let socket = Socket::new(SocketDomain::Inet, SocketType::Stream, nrf_tls_version)?;

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

		Ok(TlsSocket { socket })
	}

	/// Look up the hostname and for each result returned, try to connect to
	/// it.
	pub fn connect(&self, hostname: &str, port: u16) -> Result<(), Error> {
		use core::fmt::Write;

		debug!("Connecting via TLS to {}:{}", hostname, port);

		// First we set the hostname
		self.socket
			.set_option(SocketOption::TlsHostName(hostname))?;

		let mut result;
		// Now, make a null-terminated hostname
		let mut hostname_smallstring: heapless::String<heapless::consts::U64> =
			heapless::String::new();
		write!(hostname_smallstring, "{}\0", hostname).map_err(|_| Error::HostnameTooLong)?;
		// Now call getaddrinfo with some hints
		let hints = sys::nrf_addrinfo {
			ai_flags: 0,
			ai_family: sys::NRF_AF_INET as i32,
			ai_socktype: sys::NRF_SOCK_STREAM as i32,
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
			return Err(Error::Nordic("tls_dns", result, get_last_error()));
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
			Err(Error::Nordic("tls_connect", result, get_last_error()))
		} else {
			Ok(())
		}
	}
}

impl Pollable for TlsSocket {
	/// Get the underlying socket ID for this socket.
	fn get_fd(&self) -> i32 {
		self.socket.fd
	}
}

impl core::ops::DerefMut for TlsSocket {
	fn deref_mut(&mut self) -> &mut Socket {
		&mut self.socket
	}
}

impl core::ops::Deref for TlsSocket {
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

/// Store SSL certificates in the modem NVRAM for use with a subsequent TLS
/// connection.
///
/// Any existing certificates with the given tag are deleted.
///
/// * `tag` - the numeric value used to identify this set of certificates.
/// * `ca_chain` - Supply a string representing an X509 server side
///   certificate chain in PEM format, or None.
/// * `public_cert` - If you want client-side auth, supply an X509 client
///   certificate in PEM format here, otherwise supply None.
/// * `key` - If you want client-side auth, supply the private key for the
///   `public_cert` in PEM format here, otherwise supply None.
pub fn provision_certificates(
	tag: u32,
	ca_chain: Option<&'static str>,
	public_cert: Option<&'static str>,
	key: Option<&'static str>,
) -> Result<(), Error> {
	// Delete the existing keys
	for tag_type in &[
		sys::nrf_key_mgnt_cred_type_t_NRF_KEY_MGMT_CRED_TYPE_CA_CHAIN,
		sys::nrf_key_mgnt_cred_type_t_NRF_KEY_MGMT_CRED_TYPE_PUBLIC_CERT,
		sys::nrf_key_mgnt_cred_type_t_NRF_KEY_MGMT_CRED_TYPE_PRIVATE_CERT,
	] {
		unsafe {
			let _res = sys::nrf_inbuilt_key_delete(tag, *tag_type);
			// Carry on, even if we can't delete.
		}
	}

	unsafe {
		if let Some(ca_chain) = ca_chain {
			// Store the CA certificate in persistent memory so we can use it later
			let res = sys::nrf_inbuilt_key_write(
				// nrf_sec_tag_t            sec_tag,
				tag,
				// nrf_key_mgnt_cred_type_t cred_type,
				sys::nrf_key_mgnt_cred_type_t_NRF_KEY_MGMT_CRED_TYPE_CA_CHAIN,
				// uint8_t                * p_buffer,
				// I don't know why the API needs this as mut - const should be fine
				ca_chain.as_ptr() as *mut u8,
				// uint16_t                 buffer_len);
				ca_chain.len() as u16,
			);
			if res != 0 {
				return Err(Error::Nordic("ca_chain write", res, get_last_error()));
			}
		}
		if let Some(public_cert) = public_cert {
			// Store the client public key certificate in persistent memory so we
			// can use it later
			let res = sys::nrf_inbuilt_key_write(
				// nrf_sec_tag_t            sec_tag,
				tag,
				// nrf_key_mgnt_cred_type_t cred_type,
				sys::nrf_key_mgnt_cred_type_t_NRF_KEY_MGMT_CRED_TYPE_PUBLIC_CERT,
				// uint8_t                * p_buffer,
				// I don't know why the API needs this as mut - const should be fine
				public_cert.as_ptr() as *mut u8,
				// uint16_t                 buffer_len);
				public_cert.len() as u16,
			);
			if res != 0 {
				return Err(Error::Nordic("public_cert write", res, get_last_error()));
			}
		}
		if let Some(key) = key {
			// Store the client private key certificate in persistent memory so we
			// can use it later
			let res = sys::nrf_inbuilt_key_write(
				// nrf_sec_tag_t            sec_tag,
				tag,
				// nrf_key_mgnt_cred_type_t cred_type,
				sys::nrf_key_mgnt_cred_type_t_NRF_KEY_MGMT_CRED_TYPE_PRIVATE_CERT,
				// uint8_t                * p_buffer,
				// I don't know why the API needs this as mut - const should be fine
				key.as_ptr() as *mut u8,
				// uint16_t                 buffer_len);
				key.len() as u16,
			);
			if res != 0 {
				return Err(Error::Nordic("private_cert write", res, get_last_error()));
			}
		}
	}

	Ok(())
}

//******************************************************************************
// Private Functions and Impl on Private Types
//******************************************************************************

// None

//******************************************************************************
// End of File
//******************************************************************************
