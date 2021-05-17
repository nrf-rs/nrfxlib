//! # FFI (Foreign Function Interface) Module
//!
//! This module contains implementations of functions that libbsd.a expects to
//! be able to call.
//!
//! Copyright (c) 42 Technology, 2019
//!
//! Dual-licensed under MIT and Apache 2.0. See the [README](../README.md) for
//! more details.

use log::debug;

/// Number of IPC configurations in `NrfxIpcConfig`
const IPC_CONF_NUM: usize = 8;

/// Used by `libmodem` to configure the IPC peripheral. See `nrfx_ipc_config_t`
/// in `nrfx/drivers/include/nrfx_ipc.h`.
#[derive(Debug, Clone)]
pub struct NrfxIpcConfig {
	/// Configuration of the connection between signals and IPC channels.
	send_task_config: [u32; IPC_CONF_NUM],
	/// Configuration of the connection between events and IPC channels.
	receive_event_config: [u32; IPC_CONF_NUM],
	/// Bitmask with events to be enabled to generate interrupt.
	receive_events_enabled: u32,
}

/// IPC callback function type
type NrfxIpcHandler = extern "C" fn(event_mask: u32, ptr: *mut u8);

/// IPC error type
#[repr(u32)]
#[derive(Debug, Copy, Clone)]
pub enum NrfxErr {
	///< Operation performed successfully.
	Success = (0x0BAD0000 + 0),
	///< Internal error.
	ErrorInternal = (0x0BAD0000 + 1),
	///< No memory for operation.
	ErrorNoMem = (0x0BAD0000 + 2),
	///< Not supported.
	ErrorNotSupported = (0x0BAD0000 + 3),
	///< Invalid parameter.
	ErrorInvalidParam = (0x0BAD0000 + 4),
	///< Invalid state, operation disallowed in this state.
	ErrorInvalidState = (0x0BAD0000 + 5),
	///< Invalid length.
	ErrorInvalidLength = (0x0BAD0000 + 6),
	///< Operation timed out.
	ErrorTimeout = (0x0BAD0000 + 7),
	///< Operation is forbidden.
	ErrorForbidden = (0x0BAD0000 + 8),
	///< Null pointer.
	ErrorNull = (0x0BAD0000 + 9),
	///< Bad memory address.
	ErrorInvalidAddr = (0x0BAD0000 + 10),
	///< Busy.
	ErrorBusy = (0x0BAD0000 + 11),
	///< Module already initialized.
	ErrorAlreadyInitialized = (0x0BAD0000 + 12),
}

/// st error from the library. See `bsd_os_errno_set` and
/// Stores the l
/// `get_last_error`.
static LAST_ERROR: core::sync::atomic::AtomicI32 = core::sync::atomic::AtomicI32::new(0);

/// Remembers the IPC interrupt context we were given
static IPC_CONTEXT: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// Remembers the IPC handler function we were given
static IPC_HANDLER: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// Function required by BSD library. We need to set the EGU1 interrupt.
#[no_mangle]
pub extern "C" fn nrf_modem_os_application_irq_set() {
	cortex_m::peripheral::NVIC::pend(crate::cpu::Interrupt::EGU1);
}

/// Function required by BSD library. We need to clear the EGU1 interrupt.
#[no_mangle]
pub extern "C" fn nrf_modem_os_application_irq_clear() {
	cortex_m::peripheral::NVIC::unpend(crate::cpu::Interrupt::EGU1);
}

/// Function required by BSD library. We need to set the EGU2 interrupt.
#[no_mangle]
pub extern "C" fn nrf_modem_os_trace_irq_set() {
	cortex_m::peripheral::NVIC::pend(crate::cpu::Interrupt::EGU2);
}

/// Function required by BSD library. We need to clear the EGU2 interrupt.
#[no_mangle]
pub extern "C" fn nrf_modem_os_trace_irq_clear() {
	cortex_m::peripheral::NVIC::unpend(crate::cpu::Interrupt::EGU2);
}

/// Function required by BSD library. We have no init to do.
#[no_mangle]
pub extern "C" fn nrf_modem_os_init() {
	// Nothing
}

/// Function required by BSD library. Stores an error code we can read later.
#[no_mangle]
pub extern "C" fn nrf_modem_os_errno_set(errno: i32) {
	LAST_ERROR.store(errno, core::sync::atomic::Ordering::SeqCst);
}

/// Return the last error stored by the nrfxlib C library.
pub fn get_last_error() -> i32 {
	LAST_ERROR.load(core::sync::atomic::Ordering::SeqCst)
}

/// Function required by BSD library
#[no_mangle]
pub extern "C" fn nrf_modem_os_timedwait(_context: u32, p_timeout_ms: *const i32) -> i32 {
	let timeout_ms = unsafe { *p_timeout_ms };
	if timeout_ms < 0 {
		// With Zephyr, negative timeouts pend on a semaphore with K_FOREVER.
		// We can't do that here.
		0i32
	} else {
		// NRF9160 runs at 64 MHz, so this is close enough
		cortex_m::asm::delay((timeout_ms as u32) * 64_000);
		nrfxlib_sys::NRF_ETIMEDOUT as i32
	}
}

/// Function required by BSD library
#[no_mangle]
pub extern "C" fn nrf_modem_os_trace_put(_data: *const u8, _len: u32) -> i32 {
	// Do nothing
	0
}

/// Function required by BSD library
#[no_mangle]
pub extern "C" fn nrf_modem_irrecoverable_error_handler(err: u32) -> ! {
	panic!("bsd_irrecoverable_error_handler({})", err);
}

/// The Modem library needs to dynamically allocate memory (a heap) for proper
/// functioning. This memory is used to store the internal data structures that
/// are used to manage the communication between the application core and the
/// modem core. This memory is never shared with the modem core and hence, it
/// can be located anywhere in the application core's RAM instead of the shared
/// memory regions. This function allocates dynamic memory for the library.
#[no_mangle]
pub extern "C" fn nrf_modem_os_alloc(num_bytes: usize) -> *mut u8 {
	debug!("nrf_modem_os_alloc({})", num_bytes);
	let usize_size = core::mem::size_of::<usize>();
	let mut result = core::ptr::null_mut();
	unsafe {
		cortex_m::interrupt::free(|cs| {
			let layout =
				core::alloc::Layout::from_size_align_unchecked(num_bytes + usize_size, usize_size);
			if let Some(ref mut allocator) = *crate::GENERIC_ALLOCATOR.borrow(cs).borrow_mut() {
				match allocator.allocate_first_fit(layout) {
					Ok(block) => {
						// We need the block size to run the de-allocation. Store it in the first four bytes.
						core::ptr::write_volatile::<usize>(block.as_ptr() as *mut usize, num_bytes);
						// Give them the rest of the block
						result = block.as_ptr().offset(usize_size as isize);
					}
					Err(_e) => {
						// Ignore
					}
				}
			}
		});
	}
	result
}

/// The Modem library needs to dynamically allocate memory (a heap) for proper
/// functioning. This memory is used to store the internal data structures that
/// are used to manage the communication between the application core and the
/// modem core. This memory is never shared with the modem core and hence, it
/// can be located anywhere in the application core's RAM instead of the shared
/// memory regions. This function allocates dynamic memory for the library.
#[no_mangle]
pub extern "C" fn nrf_modem_os_free(ptr: *mut u8) {
	debug!("nrf_modem_os_free({:?})", ptr);
	let usize_size = core::mem::size_of::<usize>() as isize;
	unsafe {
		cortex_m::interrupt::free(|cs| {
			// Fetch the size from the previous four bytes
			let real_ptr = ptr.offset(-usize_size);
			let num_bytes = core::ptr::read_volatile::<usize>(real_ptr as *const usize);
			let layout =
				core::alloc::Layout::from_size_align_unchecked(num_bytes, usize_size as usize);
			if let Some(ref mut allocator) = *crate::GENERIC_ALLOCATOR.borrow(cs).borrow_mut() {
				allocator.deallocate(core::ptr::NonNull::new_unchecked(ptr), layout);
			}
		});
	}
}

/// Allocate a buffer on the TX area of shared memory.
///
/// @param bytes Buffer size.
/// @return pointer to allocated memory
#[no_mangle]
pub extern "C" fn nrf_modem_os_shm_tx_alloc(num_bytes: usize) -> *mut u8 {
	debug!("nrf_modem_os_shm_tx_alloc({})", num_bytes);
	let usize_size = core::mem::size_of::<usize>();
	let mut result = core::ptr::null_mut();
	unsafe {
		cortex_m::interrupt::free(|cs| {
			let layout =
				core::alloc::Layout::from_size_align_unchecked(num_bytes + usize_size, usize_size);
			if let Some(ref mut allocator) = *crate::TX_ALLOCATOR.borrow(cs).borrow_mut() {
				match allocator.allocate_first_fit(layout) {
					Ok(block) => {
						// We need the block size to run the de-allocation. Store it in the first four bytes.
						core::ptr::write_volatile::<usize>(block.as_ptr() as *mut usize, num_bytes);
						// Give them the rest of the block
						result = block.as_ptr().offset(usize_size as isize);
					}
					Err(_e) => {
						// Ignore
					}
				}
			}
		});
	}
	result
}

/// Free a shared memory buffer in the TX area.
///
/// @param ptr Th buffer to free.
#[no_mangle]
pub extern "C" fn nrf_modem_os_shm_tx_free(ptr: *mut u8) {
	debug!("nrf_modem_os_shm_tx_free({:?})", ptr);
	let usize_size = core::mem::size_of::<usize>() as isize;
	unsafe {
		cortex_m::interrupt::free(|cs| {
			// Fetch the size from the previous four bytes
			let real_ptr = ptr.offset(-usize_size);
			let num_bytes = core::ptr::read_volatile::<usize>(real_ptr as *const usize);
			let layout =
				core::alloc::Layout::from_size_align_unchecked(num_bytes, usize_size as usize);
			if let Some(ref mut allocator) = *crate::TX_ALLOCATOR.borrow(cs).borrow_mut() {
				allocator.deallocate(core::ptr::NonNull::new_unchecked(ptr), layout);
			}
		});
	}
}

/// @brief Function for loading configuration directly into IPC peripheral.
///
/// @param p_config Pointer to the structure with the initial configuration.
#[no_mangle]
pub extern "C" fn nrfx_ipc_config_load(p_config: *const NrfxIpcConfig) {
	unsafe {
		let config: &NrfxIpcConfig = &*p_config;
		debug!("nrfx_ipc_config_load({:?})", config);

		let ipc = &(*nrf9160_pac::IPC_NS::ptr());

		for (i, value) in config.send_task_config.iter().enumerate() {
			ipc.send_cnf[i as usize].write(|w| w.bits(*value));
		}

		for (i, value) in config.receive_event_config.iter().enumerate() {
			ipc.receive_cnf[i as usize].write(|w| w.bits(*value));
		}

		ipc.intenset
			.write(|w| w.bits(config.receive_events_enabled));
	}
}

///
/// @brief Function for initializing the IPC driver.
///
/// @param irq_priority Interrupt priority.
/// @param handler      Event handler provided by the user. Cannot be NULL.
/// @param p_context    Context passed to event handler.
///
/// @retval NRFX_SUCCESS             Initialization was successful.
/// @retval NRFX_ERROR_INVALID_STATE Driver is already initialized.
#[no_mangle]
pub extern "C" fn nrfx_ipc_init(
	irq_priority: u8,
	handler: NrfxIpcHandler,
	p_context: usize,
) -> NrfxErr {
	use cortex_m::interrupt::InterruptNumber;
	let irq = nrf9160_pac::Interrupt::IPC;
	let irq_num = usize::from(irq.number());
	unsafe {
		cortex_m::peripheral::NVIC::unmask(irq);
		(*cortex_m::peripheral::NVIC::ptr()).ipr[irq_num].write(irq_priority.into());
	}
	IPC_CONTEXT.store(p_context, core::sync::atomic::Ordering::SeqCst);
	IPC_HANDLER.store(handler as usize, core::sync::atomic::Ordering::SeqCst);
	// Report success
	NrfxErr::Success
}

/// Function for uninitializing the IPC module.
#[no_mangle]
pub extern "C" fn nrfx_ipc_uninit() {
	unimplemented!();
}

/// Call this when we have an IPC IRQ. Not `extern C` as its not called by the
/// library, only our interrupt handler code.
pub unsafe fn ipc_irq_handler() {
	// Get the information about events that fired this interrupt
	let events_map = (*nrf9160_pac::IPC_NS::ptr()).intpend.read().bits() as u32;

	// Clear these events
	let mut bitmask = events_map;
	while bitmask != 0 {
		let event_idx = bitmask.trailing_zeros();
		bitmask ^= 1 << event_idx;
		(*nrf9160_pac::IPC_NS::ptr()).events_receive[event_idx as usize].write(|w| w.bits(0));
	}

	// Execute interrupt handler to provide information about events to app
	let handler_addr = IPC_HANDLER.load(core::sync::atomic::Ordering::SeqCst);
	let handler = core::mem::transmute::<usize, NrfxIpcHandler>(handler_addr);
	let context = IPC_CONTEXT.load(core::sync::atomic::Ordering::SeqCst);
	(handler)(events_map, context as *mut u8);
}
