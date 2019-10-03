//! # Modem helper functions for nrfxlib
//!
//! Helper functions for dealing with the LTE modem.
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

use crate::Error;

//******************************************************************************
// Types
//******************************************************************************

/// Identifies which radios in the nRF9160 should be active
#[derive(Debug, Copy, Clone)]
pub enum SystemMode {
	/// LTE-M only
	LteM,
	/// NB-IoT only
	NbIot,
	/// GNSS Only
	GnssOnly,
	/// LTE-M and GNSS
	LteMAndGnss,
	/// NB-IOT and GNSS
	NbIotAndGnss,
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

/// Waits for the modem to connect to a network.
///
/// The list of acceptable CEREG response indications is taken from the Nordic
/// `lte_link_control` driver.
pub fn wait_for_lte() -> Result<(), Error> {
	let skt = crate::at::AtSocket::new()?;
	// Subscribe
	skt.write(b"AT+CEREG=2")?;

	let connected_indications = ["+CEREG: 1", "+CEREG:1", "+CEREG: 5", "+CEREG:5"];
	'outer: loop {
		let mut buf = [0u8; 128];
		let maybe_length = skt.recv(&mut buf)?;
		if let Some(length) = maybe_length {
			let s = unsafe { core::str::from_utf8_unchecked(&buf[0..length - 1]) };
			for line in s.lines() {
				let line = line.trim();
				for ind in &connected_indications {
					if line.starts_with(ind) {
						break 'outer;
					}
				}
			}
		} else {
			cortex_m::asm::wfe();
		}
	}
	Ok(())
}

/// Powers the modem off.
pub fn off() -> Result<(), Error> {
	crate::at::send_at_command("AT+CFUN=0", |_| {})?;
	Ok(())
}

/// Enable GNSS on the nRF9160-DK (PCA10090NS)
///
/// Sends a AT%XMAGPIO command which activates the off-chip GNSS RF routing
/// switch when receiving signals between 1574 MHz and 1577 MHz.
///
/// Works on the nRF9160-DK (PCA10090NS) and Actinius Icarus. Other PCBs may
/// use different MAGPIO pins to control the GNSS switch.
pub fn configure_gnss_on_pca10090ns() -> Result<(), Error> {
	// Configure the GNSS antenna. See `nrf/samples/nrf9160/gps/src/main.c`.
	crate::at::send_at_command("AT%XMAGPIO=1,0,0,1,1,1574,1577", |_| {})?;
	Ok(())
}

/// Set which radios should be active. Only works when modem is off.
pub fn set_system_mode(mode: SystemMode) -> Result<(), Error> {
	crate::at::send_at_command(
		match mode {
			SystemMode::LteM => "AT%XSYSTEMMODE=1,0,0,0",
			SystemMode::NbIot => "AT%XSYSTEMMODE=0,1,0,0",
			SystemMode::GnssOnly => "AT%XSYSTEMMODE=0,0,1,0",
			SystemMode::LteMAndGnss => "AT%XSYSTEMMODE=1,0,1,0",
			SystemMode::NbIotAndGnss => "AT%XSYSTEMMODE=0,1,1,0",
		},
		|_| {},
	)?;
	Ok(())
}

/// Get which radios should be active
pub fn get_system_mode() -> Result<SystemMode, Error> {
	let mut result = Err(Error::UnrecognisedValue);
	// Don't care about final digit - that's just the LTE/NB-IOT preference
	crate::at::send_at_command("AT%XSYSTEMMODE?", |res| {
		if res.starts_with("%XSYSTEMMODE: 1,0,0,") {
			result = Ok(SystemMode::LteM);
		} else if res.starts_with("%XSYSTEMMODE: 0,1,0,") {
			result = Ok(SystemMode::NbIot);
		} else if res.starts_with("%XSYSTEMMODE: 0,0,1,") {
			result = Ok(SystemMode::GnssOnly);
		} else if res.starts_with("%XSYSTEMMODE: 1,0,1,") {
			result = Ok(SystemMode::LteMAndGnss);
		} else if res.starts_with("%XSYSTEMMODE: 0,1,1,") {
			result = Ok(SystemMode::NbIotAndGnss);
		}
	})?;
	result
}

/// Puts the modem into flight mode.
pub fn flight_mode() -> Result<(), Error> {
	let skt = crate::at::AtSocket::new()?;
	// Flight Mode
	skt.write(b"AT+CFUN=4")?;
	Ok(())
}

/// Powers the modem on and sets it to auto-register, but does not wait for it
/// to connect to a network.
pub fn start() -> Result<(), Error> {
	let skt = crate::at::AtSocket::new()?;
	// Auto Register
	skt.write(b"AT+COPS=0")?;
	// Normal Mode
	skt.write(b"AT+CFUN=1")?;
	Ok(())
}

//******************************************************************************
// Private Functions and Impl on Private Types
//******************************************************************************

// None

//******************************************************************************
// End of File
//******************************************************************************
