//! # AT Sockets for nrfxlib
//!
//! AT socket related code. AT commands are sent to the modem down a socket
//! using the Nordic-specific `SOCK_PROTO_AT`.
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

use super::Error;
use crate::raw::*;

//******************************************************************************
// Types
//******************************************************************************

/// Represents a connection to the modem using AT Commands.
#[derive(Debug)]
pub struct AtSocket(Socket);

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

impl AtSocket {
	/// Create a new AT socket.
	pub fn new() -> Result<AtSocket, Error> {
		let skt = Socket::new(SocketDomain::Lte, SocketType::None, SocketProtocol::At)?;
		Ok(AtSocket(skt))
	}

	/// Send an AT command to the modem
	pub fn send_command(&self, command: &str) -> Result<(), Error> {
		self.0.write(command.as_bytes()).map(|_count| ())
	}
}

impl Pollable for AtSocket {
	/// Get the underlying socket ID for this socket.
	fn get_fd(&self) -> i32 {
		self.0.fd
	}
}

impl core::ops::DerefMut for AtSocket {
	fn deref_mut(&mut self) -> &mut Socket {
		&mut self.0
	}
}

impl core::ops::Deref for AtSocket {
	type Target = Socket;
	fn deref(&self) -> &Socket {
		&self.0
	}
}

/// Sends an AT command to the modem and calls the given closure with any
/// indications received. Indications have any whitespace or newlines trimmed.
///
/// Creates and destroys a new NRF_AF_LTE/NRF_PROTO_AT socket. Will block
/// until we get 'OK' or some sort of error response from the modem.
pub fn send_at_command<F>(command: &str, mut function: F) -> Result<(), Error>
where
	F: FnMut(&str),
{
	let skt = AtSocket::new()?;
	skt.send_command(command)?;
	let result;
	'outer: loop {
		let mut buf = [0u8; 256];
		let length = 'inner: loop {
			match skt.recv(&mut buf) {
				Ok(None) => {
					// EAGAIN
				}
				Err(e) => {
					return Err(e);
				}
				Ok(Some(n)) => break 'inner n,
			};
		};
		let s = unsafe { core::str::from_utf8_unchecked(&buf[0..length - 1]) };
		for line in s.lines() {
			let line = line.trim();
			if line == "OK" {
				// This is our final response
				result = Ok(());
				break 'outer;
			} else if line == "ERROR"
				|| line.starts_with("+CME ERROR")
				|| line.starts_with("+CMS ERROR")
			{
				// We think this is our final response
				result = Err(Error::AtError);
				break 'outer;
			} else {
				// Assume it's an indication
				function(line);
			}
		}
	}
	result
}

//******************************************************************************
// Private Functions and Impl on Private Types
//******************************************************************************

// None

//******************************************************************************
// End of File
//******************************************************************************
