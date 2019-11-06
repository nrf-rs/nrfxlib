//! # GNSS Module for nrfxlib
//!
//! GNSS related socket code.
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
use crate::raw::*;
use nrfxlib_sys as sys;

//******************************************************************************
// Types
//******************************************************************************

/// Represents a connection to the GPS sub-system.
#[derive(Debug)]
pub struct GnssSocket(Socket);

/// Represents a position or NMEA string from the GNSS subsystem
#[derive(Clone)]
pub enum GnssData {
	/// An NMEA formatted string, beginning with '$'.
	Nmea {
		/// A non-null terminated buffer of ASCII bytes
		buffer: [u8; 83],
		/// The number of valid bytes in `buffer`
		length: usize,
	},
	/// A Nordic-supplied structure containing position, time and SV
	/// information.
	Position(sys::nrf_gnss_pvt_data_frame_t),
	/// AGPS data
	Agps(sys::nrf_gnss_agps_data_frame_t),
}

/// Specifies which NMEA fields you want from the GNSS sub-system.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct NmeaMask(u16);

/// The specific fields you can enable or disable in an `NmeaMask`.
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u16)]
pub enum NmeaField {
	/// Enables Global Positioning System Fix Data.
	GpsFixData = sys::NRF_GNSS_NMEA_GGA_MASK as u16,
	/// Enables Geographic Position Latitude/Longitude and time.
	LatLongTime = sys::NRF_GNSS_NMEA_GLL_MASK as u16,
	/// Enables DOP and active satellites.
	DopAndActiveSatellites = sys::NRF_GNSS_NMEA_GSA_MASK as u16,
	/// Enables Satellites in view.
	SatellitesInView = sys::NRF_GNSS_NMEA_GSV_MASK as u16,
	/// Enables Recommended minimum specific GPS/Transit data.
	RecommendedMinimumSpecificFixData = sys::NRF_GNSS_NMEA_RMC_MASK as u16,
}

/// Specifies which non-volatile fields you want to delete before starting the GNSS.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct DeleteMask(u32);

/// The specific fields you can enable or disable in a `DeleteMask`.
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
pub enum DeleteField {
	/// Bit 0 denotes ephemerides data.
	Ephemerides = 1 << 0,
	/// Bit 1 denotes almanac data (excluding leap second and ionospheric correction parameters).
	Almanac = 1 << 1,
	/// Bit 2 denotes ionospheric correction parameters data.
	IonosphericCorrection = 1 << 2,
	/// Bit 3 denotes last good fix (the last position) data.
	LastGoodFix = 1 << 3,
	/// Bit 4 denotes GPS time-of-week (TOW) data.
	TimeOfWeek = 1 << 4,
	/// Bit 5 denotes GPS week number data.
	WeekNumber = 1 << 5,
	/// Bit 6 denotes leap second (UTC parameters) data.
	LeapSecond = 1 << 6,
	/// Bit 7 denotes local clock (TCXO) frequency offset data.
	LocalClockFrequencyOffset = 1 << 7,
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

impl GnssSocket {
	/// Create a new GNSS socket.
	pub fn new() -> Result<GnssSocket, Error> {
		let skt = Socket::new(
			SocketDomain::Local,
			SocketType::Datagram,
			SocketProtocol::Gnss,
		)?;
		Ok(GnssSocket(skt))
	}

	/// Start the GNSS system.
	pub fn start(&self, delete_mask: DeleteMask) -> Result<(), Error> {
		self.0
			.set_option(SocketOption::GnssStart(delete_mask.as_u32()))?;
		Ok(())
	}

	/// Stop the GNSS system.
	pub fn stop(&self) -> Result<(), Error> {
		self.0.set_option(SocketOption::GnssStop)?;
		Ok(())
	}

	/// Set the Fix Interval.
	///
	/// See Nordic for an explanation of this parameter.
	pub fn set_fix_interval(&self, interval: u16) -> Result<(), Error> {
		self.0.set_option(SocketOption::GnssFixInterval(interval))?;
		Ok(())
	}

	/// Set the Fix Retry time.
	///
	/// See Nordic for an explanation of this parameter.
	pub fn set_fix_retry(&self, interval: u16) -> Result<(), Error> {
		self.0.set_option(SocketOption::GnssFixRetry(interval))?;
		Ok(())
	}

	/// Get the current Fix Interval.
	///
	/// See Nordic for an explanation of this parameter.
	pub fn get_fix_interval(&self) -> Result<u16, Error> {
		let mut length: u32 = core::mem::size_of::<u16>() as u32;
		let mut value = 0u16;
		let result = unsafe {
			sys::nrf_getsockopt(
				self.fd,
				sys::NRF_SOL_GNSS as i32,
				sys::NRF_SO_GNSS_FIX_INTERVAL as i32,
				&mut value as *mut u16 as *mut sys::ctypes::c_void,
				&mut length as *mut u32,
			)
		};
		if result < 0 {
			Err(Error::Nordic("fix_interval", result, get_last_error()))
		} else {
			Ok(value)
		}
	}

	/// Get the Fix Retry time.
	///
	/// See Nordic for an explanation of this parameter.
	pub fn get_fix_retry(&self) -> Result<u16, Error> {
		let mut length: u32 = core::mem::size_of::<u16>() as u32;
		let mut value = 0u16;
		let result = unsafe {
			sys::nrf_getsockopt(
				self.fd,
				sys::NRF_SOL_GNSS as i32,
				sys::NRF_SO_GNSS_FIX_RETRY as i32,
				&mut value as *mut u16 as *mut sys::ctypes::c_void,
				&mut length as *mut u32,
			)
		};
		if result < 0 {
			Err(Error::Nordic("fix_retry", result, get_last_error()))
		} else {
			Ok(value)
		}
	}

	/// Set the NMEA mask.
	///
	/// You can select which particular NMEA strings you want from the GNSS socket here.
	///
	/// If you pass a default `NmeaMask`, you get no NMEA strings (only
	/// `GnssData::Position`).
	pub fn set_nmea_mask(&self, mask: NmeaMask) -> Result<(), Error> {
		self.0
			.set_option(SocketOption::GnssNmeaMask(mask.as_u16()))?;
		Ok(())
	}

	/// Get the current NMEA mask.
	///
	/// See `set_nmea_mask`.
	pub fn get_nmea_mask(&self) -> Result<NmeaMask, Error> {
		let mut length: u32 = core::mem::size_of::<u16>() as u32;
		let mut value = 0u16;
		let result = unsafe {
			sys::nrf_getsockopt(
				self.fd,
				sys::NRF_SOL_GNSS as i32,
				sys::NRF_SO_GNSS_NMEA_MASK as i32,
				&mut value as *mut u16 as *mut sys::ctypes::c_void,
				&mut length as *mut u32,
			)
		};
		if result < 0 {
			Err(Error::Nordic("nmea_mask", result, get_last_error()))
		} else {
			Ok(NmeaMask(value))
		}
	}

	/// Get a fix from the GNSS system.
	///
	/// Performs a read on the GNSS socket. The Nordic library determines which
	/// frame type you get on each read. You will get `None` if there is no fix
	/// to be read.
	pub fn get_fix(&self) -> Result<Option<GnssData>, Error> {
		let mut frame = core::mem::MaybeUninit::<sys::nrf_gnss_data_frame_t>::uninit();
		let buffer_size = core::mem::size_of::<sys::nrf_gnss_data_frame_t>();
		let result = unsafe {
			sys::nrf_recv(
				self.0.fd,
				frame.as_mut_ptr() as *mut sys::ctypes::c_void,
				buffer_size,
				sys::NRF_MSG_DONTWAIT as i32,
			)
		};
		self.process_fix(result, frame)
	}

	/// Wait for a fix from the GNSS system.
	///
	/// Performs a read on the GNSS socket and returns either a
	/// `GnssData::Nmea`, if an NMEA string has been returned, or a
	/// `GnssData::Position`. The Nordic library determines which you get on
	/// each read. You will get `None` if there is no fix to be read.
	pub fn get_fix_blocking(&self) -> Result<Option<GnssData>, Error> {
		let mut frame = core::mem::MaybeUninit::<sys::nrf_gnss_data_frame_t>::uninit();
		let buffer_size = core::mem::size_of::<sys::nrf_gnss_data_frame_t>();
		let result = unsafe {
			sys::nrf_recv(
				self.0.fd,
				frame.as_mut_ptr() as *mut sys::ctypes::c_void,
				buffer_size,
				0,
			)
		};
		self.process_fix(result, frame)
	}

	/// Parse the data returned from a GNSS socket read.
	fn process_fix(
		&self,
		result: isize,
		frame: core::mem::MaybeUninit<sys::nrf_gnss_data_frame_t>,
	) -> Result<Option<GnssData>, Error> {
		match result {
			0 => {
				// No fix available
				Ok(None)
			}
			n if n < 0 => {
				let err = get_last_error();
				if err == sys::NRF_EAGAIN as i32 {
					// Special case for EAGAIN
					Ok(None)
				} else {
					// Report the error
					Err(Error::Nordic("get_fix", n as i32, err))
				}
			}
			_ => {
				// Got some valid data - but what?
				let frame = unsafe { frame.assume_init() };
				// Unpack the C union and return a nice Rust structure...
				if frame.data_id as u32 == sys::NRF_GNSS_PVT_DATA_ID {
					// We have frame.pvt
					// NOTE(unsafe) - we have to trust that the Nordic library has given us enough bytes for the frame.
					let pvt = unsafe { frame.__bindgen_anon_1.pvt };
					Ok(Some(GnssData::Position(pvt)))
				} else if frame.data_id as u32 == sys::NRF_GNSS_NMEA_DATA_ID {
					// We have frame.nmea
					let nmea = unsafe { &frame.__bindgen_anon_1.nmea[..] };
					// Find null-terminator
					let string_length = nmea
						.iter()
						.cloned()
						.enumerate()
						.find(|x| x.1 == b'\0' || x.1 == b'\r' || x.1 == b'\n')
						.map(|x| x.0)
						.unwrap_or(0);
					if core::str::from_utf8(&nmea[0..string_length]).is_ok() {
						// Valid UTF-8
						Ok(Some(GnssData::Nmea {
							buffer: unsafe { frame.__bindgen_anon_1.nmea },
							length: string_length,
						}))
					} else {
						// Not a UTF-8 string
						Err(Error::BadDataFormat)
					}
				} else if frame.data_id as u32 == sys::NRF_GNSS_AGPS_DATA_ID {
					// We have frame.agps
					// NOTE(unsafe) - we have to trust that the Nordic library has given us enough bytes for the frame.
					let agps = unsafe { frame.__bindgen_anon_1.agps };
					Ok(Some(GnssData::Agps(agps)))
				} else {
					// Not a known data type
					Err(Error::BadDataFormat)
				}
			}
		}
	}
}

impl Pollable for GnssSocket {
	/// Get the underlying socket ID for this socket.
	fn get_fd(&self) -> i32 {
		self.0.fd
	}
}

impl Drop for GnssSocket {
	fn drop(&mut self) {
		let _ = self.stop();
	}
}

impl core::ops::Deref for GnssSocket {
	type Target = Socket;
	fn deref(&self) -> &Socket {
		&self.0
	}
}

impl core::ops::DerefMut for GnssSocket {
	fn deref_mut(&mut self) -> &mut Socket {
		&mut self.0
	}
}

impl GnssData {
	/// Returns true if this fix is valid (i.e. is a position frame, AND has the valid flag set).
	pub fn is_valid(&self) -> bool {
		match self {
			GnssData::Nmea { .. } => false,
			GnssData::Position(p) => (p.flags & sys::NRF_GNSS_PVT_FLAG_FIX_VALID_BIT as u8) != 0,
			GnssData::Agps { .. } => false,
		}
	}
}

impl core::fmt::Debug for GnssData {
	fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
		match self {
			GnssData::Nmea { buffer, length } => {
				// NOTE(unsafe) - we checked this when we created the GnssData on line 890
				let nmea_str = unsafe { core::str::from_utf8_unchecked(&buffer[0..*length]) };
				fmt.debug_struct("GnssData")
					.field("nmea", &nmea_str)
					.finish()
			}
			GnssData::Position(p) => fmt.debug_struct("GnssData").field("position", &p).finish(),
			GnssData::Agps(p) => fmt.debug_struct("GnssData").field("agps", &p).finish(),
		}
	}
}

impl NmeaMask {
	/// Create a new NmeaMask, which selects no NMEA fields.
	pub fn new() -> Self {
		NmeaMask(0)
	}

	/// Enable a particular NMEA field type in this mask.
	pub fn set(self, field: NmeaField) -> Self {
		NmeaMask(self.0 | field.value())
	}

	/// Disable a particular NMEA field type in this mask.
	pub fn clear(self, field: NmeaField) -> Self {
		NmeaMask(self.0 & !field.value())
	}

	/// Convert to an integer, for the socket to consume.
	pub fn as_u16(self) -> u16 {
		self.0
	}
}

impl NmeaField {
	/// Convert an NmeaField into an integer
	fn value(self) -> u16 {
		self as u16
	}
}

impl DeleteMask {
	/// Create a new DeleteMask, which selects nothing to be deleted.
	pub fn new() -> Self {
		DeleteMask(0)
	}

	/// Mark a particular field as requiring deletion.
	pub fn set(self, field: DeleteField) -> Self {
		DeleteMask(self.0 | field.value())
	}

	/// Unmark a particular field as requiring deletion.
	pub fn clear(self, field: DeleteField) -> Self {
		DeleteMask(self.0 & !field.value())
	}

	/// Convert to an integer, for the socket to consume.
	pub fn as_u32(self) -> u32 {
		self.0
	}
}

impl DeleteField {
	/// Convert an DeleteField into an integer
	fn value(self) -> u32 {
		self as u32
	}
}

//******************************************************************************
// Private Functions and Impl on Private Types
//******************************************************************************

// None

//******************************************************************************
// End of File
//******************************************************************************
