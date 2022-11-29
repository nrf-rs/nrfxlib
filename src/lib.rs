//! # nrfxlib - a Rust library for the nRF9160 interface C library
//!
//! This crate contains wrappers for functions and types defined in Nordic's
//! libmodem, which is part of nrfxlib.
//!
//! The `nrfxlib_sys` crate is the auto-generated wrapper for `nrf_modem_os.h`
//! and `nrf_socket.h`. This crate contains Rustic wrappers for those
//! auto-generated types.
//!
//! To bring up the LTE stack you need to call `nrf_modem_init()`. Before that
//! you need to enable the EGU1 and EGU2 interrupts, and arrange for the
//! relevant functions (`application_irq_handler` and `trace_irq_handler`
//! respectively) to be called when they occur. The IPC interrupt handler
//! is registered by the relevant callback.
//!
//! To talk to the LTE modem, use the `at::send_at_command()` function. It will call
//! the callback with the response received from the modem.
//!
//! To automatically send the AT commands which initialise the modem and wait
//! until it has registered on the network, call the `wait_for_lte()` function.
//! Once that is complete, you can create TCP or TLS sockets and send/receive
//! data.
//!
//! Copyright (c) 42 Technology Ltd 2021
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
pub mod dtls;
mod ffi;
pub mod gnss;
pub mod modem;
mod raw;
pub mod tcp;
pub mod tls;
pub mod udp;

//******************************************************************************
// Imports
//******************************************************************************

pub use api::*;
pub use ffi::{get_last_error, NrfxErr};
pub use raw::{poll, PollEntry, PollFlags, PollResult, Pollable};

use core::cell::RefCell;
use cortex_m::interrupt::Mutex;
use linked_list_allocator::Heap;
use log::{debug, trace};
use nrf9160_pac as cpu;
use nrfxlib_sys as sys;

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

/// Errors that can be returned in response to an AT command.
#[derive(Debug, Clone)]
pub enum AtError {
	/// Plain `ERROR` response
	Error,
	/// `+CME ERROR xx` response
	CmeError(i32),
	/// `+CMS ERROR xx` response
	CmsError(i32),
}

/// The set of error codes we can get from this API.
#[derive(Debug, Clone)]
pub enum Error {
	/// An error was returned by the Nordic library. We supply a string
	/// descriptor, the return code, and the value of `errno`.
	Nordic(&'static str, i32, i32),
	/// An AT error (`ERROR`, `+CMS ERROR` or `+CME ERROR`) was returned by the modem.
	AtError(AtError),
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

/// We need to wrap our heap so it's creatable at run-time and accessible from an ISR.
///
/// * The Mutex allows us to safely share the heap between interrupt routines
///   and the main thread - and nrfxlib will definitely use the heap in an
///   interrupt.
/// * The RefCell lets us share and object and mutate it (but not at the same
///   time)
/// * The Option is because the `linked_list_allocator::empty()` function is not
///   `const` yet and cannot be called here
///
type WrappedHeap = Mutex<RefCell<Option<Heap>>>;

//******************************************************************************
// Constants
//******************************************************************************

// None

//******************************************************************************
// Global Variables
//******************************************************************************

/// Our general heap.
///
/// We initialise it later with a static variable as the backing store.
static LIBRARY_ALLOCATOR: WrappedHeap = Mutex::new(RefCell::new(None));

/// Our transmit heap.

/// We initalise this later using a special region of shared memory that can be
/// seen by the Cortex-M33 and the modem CPU.
static TX_ALLOCATOR: WrappedHeap = Mutex::new(RefCell::new(None));

//******************************************************************************
// Macros
//******************************************************************************

// None

//******************************************************************************
// Public Functions and Impl on Public Types
//******************************************************************************

/// Start the NRF Modem library
pub fn init() -> Result<(), Error> {
	unsafe {
		/// Allocate some space in global data to use as a heap.
		static mut HEAP_MEMORY: [u32; 1024] = [0u32; 1024];
		let heap_start = HEAP_MEMORY.as_mut_ptr() as *mut _;
		let heap_size = HEAP_MEMORY.len() * core::mem::size_of::<u32>();
		cortex_m::interrupt::free(|cs| {
			*LIBRARY_ALLOCATOR.borrow(cs).borrow_mut() =
				Some(Heap::new(heap_start, heap_size))
		});
	}

	// Tell nrf_modem what memory it can use.
	let params = sys::nrf_modem_init_params_t {
		shmem: sys::nrf_modem_shmem_cfg {
			ctrl: sys::nrf_modem_shmem_cfg__bindgen_ty_1 {
				// At start of shared memory (see memory.x)
				base: 0x2001_0000,
				// This is the amount specified in the NCS 1.5.1 release.
				size: 0x0000_04e8,
			},
			tx: sys::nrf_modem_shmem_cfg__bindgen_ty_2 {
				// Follows on from control buffer
				base: 0x2001_04e8,
				// This is the amount specified in the NCS 1.5.1 release.
				size: 0x0000_2000,
			},
			rx: sys::nrf_modem_shmem_cfg__bindgen_ty_3 {
				// Follows on from TX buffer
				base: 0x2001_24e8,
				// This is the amount specified in the NCS 1.5.1 release.
				size: 0x0000_2000,
			},
			// No trace info
			trace: sys::nrf_modem_shmem_cfg__bindgen_ty_4 { base: 0, size: 0 },
		},
		ipc_irq_prio: 0,
	};

	unsafe {
		// Use the same TX memory region as above
		cortex_m::interrupt::free(|cs| {
			*TX_ALLOCATOR.borrow(cs).borrow_mut() = Some(Heap::new(
				params.shmem.tx.base as *mut _,
				params.shmem.tx.size as usize,
			))
		});
	}

	// OK, let's start the library
	let result = unsafe { sys::nrf_modem_init(&params, sys::nrf_modem_mode_t_NORMAL_MODE) };

	// Was it happy?
	if result < 0 {
		Err(Error::Nordic("init", result, ffi::get_last_error()))
	} else {
		trace!("nrfxlib init complete");
		Ok(())
	}
}

/// Stop the NRF Modem library
pub fn shutdown() {
	debug!("nrfxlib shutdown");
	unsafe {
		sys::nrf_modem_shutdown();
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
			octets[3],
			octets[2],
			octets[1],
			octets[0],
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
