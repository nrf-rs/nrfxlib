//! # nrfxlib - a Rust library for the nRF9160 interface C library
//!
//! This crate contains wrappers for functions and types defined in Nordic's
//! libbsd, aka nrfxlib.
//!
//! The `nrfxlib_sys` crate is the auto-generated wrapper for `bsd_os.h` and
//! `nrf_socket.h`. This crate contains Rustic wrappers for those
//! auto-generated types.
//!
//! To bring up the LTE stack you need to call `bsd_init()`. Before that you
//! need to enable the EGU1 and EGU2 interrupts, and arrange for the relevant
//! functions (`application_irq_handler` and `trace_irq_handler`
//! respectively) to be called when they occur. You also need to call
//! `IPC_IRQHandler()` when an IPC interrupt occurs.
//!
//! To talk to the LTE modem, use the `send_at_command()` function. It will
//! call the callback with the response received from the modem.
//!
//! To automatically send the AT commands which initialise the modem and wait
//! until it has registered on the network, call the `wait_for_lte()`
//! function. Once that is complete, you can create TCP or TLS sockets and
//! send/receive data.
//!
//! Copyright (c) 42 Technology Ltd 2019
//!
//! Dual-licensed under MIT and Apache 2.0. See the [README](../README.md) for
//! more details.

#![no_std]
#![deny(missing_docs)]

//******************************************************************************
// Sub-Modules
//******************************************************************************

pub mod api;
pub mod at;
mod ffi;
pub mod gnss;
pub mod modem;
mod raw;
pub mod tcp;
pub mod tls;

//******************************************************************************
// Imports
//******************************************************************************

pub use api::*;
pub use ffi::get_last_error;

use nrfxlib_sys as sys;

pub use raw::{poll, PollEntry, PollFlags, PollResult, Pollable};

use log::{debug, trace};

//******************************************************************************
// Types
//******************************************************************************

/// Create a camel-case type name for socket addresses.
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct NrfSockAddrIn(sys::nrf_sockaddr_in);

/// Create a camel-case type name for socket information.
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct NrfAddrInfo(sys::nrf_addrinfo);

impl core::ops::Deref for NrfSockAddrIn {
	type Target = sys::nrf_sockaddr_in;

	fn deref(&self) -> &sys::nrf_sockaddr_in {
		&self.0
	}
}

/// The set of error codes we can get from this API.
#[derive(Debug, Clone)]
pub enum Error {
	/// An error was returned by the Nordic library. We supply a string
	/// descriptor, the return code, and the value of `errno`.
	Nordic(&'static str, i32, i32),
	/// An AT error (`ERROR`, `+CMS ERROR` or `+CME ERROR`) was returned by the modem.
	AtError,
	/// Data returned by the modem was not in a format we could understand.
	BadDataFormat,
	/// Given hostname was too long for internal buffers to hold
	HostnameTooLong,
	/// Unrecognised value from AT interface
	UnrecognisedValue,
	/// A socket write error occurred
	WriteError,
	/// Too many sockets given
	TooManySockets,
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

/// Start the BSD library
pub fn init() {
	debug!("nrfxlib init");
	unsafe {
		sys::bsd_init();
	}
	trace!("nrfxlib init complete");
}

/// Stop the BSD library
pub fn shutdown() {
	debug!("nrfxlib shutdown");
	unsafe {
		sys::bsd_shutdown();
	}
	trace!("nrfxlib shutdown complete");
}

impl From<core::fmt::Error> for Error {
	fn from(_err: core::fmt::Error) -> Error {
		Error::WriteError
	}
}

impl core::fmt::Display for NrfSockAddrIn {
	fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
		let octets = self.sin_addr.s_addr.to_be_bytes();
		write!(
			f,
			"{}.{}.{}.{}:{}",
			octets[0],
			octets[1],
			octets[2],
			octets[3],
			u16::from_be(self.sin_port)
		)
	}
}

//******************************************************************************
// Private Functions and Impl on Private Types
//******************************************************************************

// None

//******************************************************************************
// End of File
//******************************************************************************
