//! # libbsd.a API implementation
//!
//! Implements the C functions that libbsd.a needs in order to operate.
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

use nrfxlib_sys as sys;

//******************************************************************************
// Types
//******************************************************************************

// None

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

/// Trampoline into the BSD library function `bsd_os_application_irq_handler`.
/// You must call this when an EGU1 interrupt occurs.
pub fn application_irq_handler() {
	unsafe {
		sys::bsd_os_application_irq_handler();
	}
}

/// Trampoline into the BSD library function `bsd_os_trace_irq_handler`. You
/// must call this when an EGU2 interrupt occurs.
pub fn trace_irq_handler() {
	unsafe {
		sys::bsd_os_trace_irq_handler();
	}
}

/// Trampoline into the BSD library function `IPC_IRQHandler`. You must call
/// this when an IPC interrupt occurs.
pub fn ipc_irq_handler() {
	unsafe {
		crate::ffi::IPC_IRQHandler();
	}
}

//******************************************************************************
// Private Functions and Impl on Private Types
//******************************************************************************

// None

//******************************************************************************
// End of File
//******************************************************************************
