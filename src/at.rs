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

use crate::{raw::*, AtError, Error};

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
		let skt = Socket::new(SocketDomain::Lte, SocketType::Datagram, SocketProtocol::At)?;
		Ok(AtSocket(skt))
	}

	/// Send an AT command to the modem
	pub fn send_command(&self, command: &str) -> Result<(), Error> {
		self.0.write(command.as_bytes()).map(|_count| ())
	}

	/// Read from the AT socket until we get something that indicates the command has completed.
	///
	/// Commands are completed by `OK`, `ERROR`, `+CME ERROR:xxx` or `+CMS
	/// ERROR:xxx`. These are mapped to a Rust `Result` type.
	///
	/// Any other data received is deemed to be a command result and passed to the given fn `callback_function`.
	pub fn poll_response<F>(&mut self, mut callback_function: F) -> Result<(), Error>
	where
		F: FnMut(&str),
	{
		let result;
		'outer: loop {
			let mut buf = [0u8; 256];
			let length = 'inner: loop {
				match self.recv(&mut buf)? {
					None => {
						// EAGAIN
					}
					Some(n) => break 'inner n,
				};
			};
			let s = unsafe { core::str::from_utf8_unchecked(&buf[0..length - 1]) };
			for line in s.lines() {
				let line = line.trim();
				match line {
					"OK" => {
						result = Ok(());
						break 'outer;
					}
					"ERROR" => {
						result = Err(Error::AtError(AtError::Error));
						break 'outer;
					}
					err if err.starts_with("+CME ERROR:") => {
						let num_str = &err[11..];
						let value = num_str.trim().parse().unwrap_or(-1);
						result = Err(Error::AtError(AtError::CmeError(value)));
						break 'outer;
					}
					err if err.starts_with("+CMS ERROR:") => {
						let num_str = &err[11..];
						let value = num_str.trim().parse().unwrap_or(-1);
						result = Err(Error::AtError(AtError::CmsError(value)));
						break 'outer;
					}
					data => {
						callback_function(data);
					}
				}
			}
		}
		result
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
pub fn send_at_command<F>(command: &str, function: F) -> Result<(), Error>
where
	F: FnMut(&str),
{
	let mut skt = AtSocket::new()?;
	skt.send_command(command)?;
	skt.poll_response(function)
}

//******************************************************************************
// Private Functions and Impl on Private Types
//******************************************************************************

// None

//******************************************************************************
// End of File
//******************************************************************************
