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
	peer_verify: i32,
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
	/// Create a new TLS socket. Only supports TLS v1.2 and IPv4 at the moment.
	pub fn new(peer_verify: bool, security_tags: &[u32]) -> Result<TlsSocket, Error> {
		let socket = Socket::new(
			SocketDomain::Inet,
			SocketType::Stream,
			SocketProtocol::Tls1v2,
		)?;

		// Now configure this socket

		// Set whether we verify the peer
		socket.set_option(SocketOption::TlsPeerVerify(if peer_verify { 1 } else { 0 }))?;

		// We skip the cipher list

		if !security_tags.is_empty() {
			// Configure the socket to use the pre-stored certificates. See
			// `provision_certificates`.
			socket.set_option(SocketOption::TlsTagList(security_tags))?;
		}

		Ok(TlsSocket {
			socket,
			peer_verify: if peer_verify { 1 } else { 0 },
		})
	}

	/// Look up the hostname and for each result returned, try to connect to
	/// it.
	pub fn connect(&self, hostname: &str, port: u16) -> Result<(), Error> {
		use core::fmt::Write;

		// First we set the hostname
		self.socket
			.set_option(SocketOption::TlsHostName(hostname))?;

		let mut result;
		// Now, make a null-terminated hostname
		let mut hostname_smallstring: heapless::String<heapless::consts::U64> =
			heapless::String::new();
		write!(hostname_smallstring, "{}\0", hostname).map_err(|_| Error::HostnameTooLong)?;
		// Now call getaddrinfo with some hints
		let hints = crate::NrfAddrInfo {
			ai_flags: 0,
			ai_family: sys::NRF_AF_INET as i32,
			ai_socktype: sys::NRF_SOCK_STREAM as i32,
			ai_protocol: 0,
			ai_addrlen: 0,
			ai_addr: core::ptr::null_mut(),
			ai_canonname: core::ptr::null_mut(),
			ai_next: core::ptr::null_mut(),
		};
		let mut output_ptr: *mut crate::NrfAddrInfo = core::ptr::null_mut();
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
			let mut record: &crate::NrfAddrInfo = unsafe { &*output_ptr };
			loop {
				let dns_addr: &crate::NrfSockAddrIn =
					unsafe { &*(record.ai_addr as *const crate::NrfSockAddrIn) };
				// Create a new sockaddr_in with the right port
				let connect_addr = crate::NrfSockAddrIn {
					sin_len: core::mem::size_of::<crate::NrfSockAddrIn>() as u8,
					sin_family: sys::NRF_AF_INET as i32,
					sin_port: htons(port),
					sin_addr: dns_addr.sin_addr.clone(),
				};

				// try and connect to this result
				result = unsafe {
					sys::nrf_connect(
						self.socket.fd,
						&connect_addr as *const crate::NrfSockAddrIn as *const _,
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
