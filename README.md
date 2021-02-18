[![crates.io](https://img.shields.io/crates/d/nrfxlib.svg)](https://crates.io/crates/nrfxlib)
[![crates.io](https://img.shields.io/crates/v/nrfxlib.svg)](https://crates.io/crates/nrfxlib)

# nrfxlib

> Rust nrfxlib wrapper for the nRF9160

This crate is published by [42 Technology Ltd](https://www.42technology.com).

## Introduction

This crate provides a Socket API for embedded applications running on the
Nordic nRF9160.

Access to the LTE baseband on the nRF9160 is currently only available using
Nordic's closed-source binary blob - a static library called `libbsd.a`, which
lives in a public Nordic git repo called `nrfxlib` along with a few other bits
and pieces (see https://github.com/NordicPlayground/nrfxlib).

That library provides a Berkeley-ish socket API, with some extensions to the
usual socket types so that you can open an AT socket to talk AT commands to the
baseband, and a GNSS socket so you can read GPS data.

## Getting the static library

We use a crate called `nrfxlib-sys` to link to the library. This crate
includes Nordic's header files and static library as a git sub-module (from
their [Github page](https://github.com/NordicPlayground/nrfxlib)) and runs
[`bindgen`] to generate Rust 'headers' which correspond to the functions and
constants in the relevant header files. You will need bindgen and LLVM to
be installed for [`bindgen`] to work, so please do see their
[documentation](https://github.com/rust-lang/rust-bindgen).

[`bindgen`]: https://crates.io/crates/bindgen

## Using this wrapper

The basic premise is that this crate calls out to Nordic's library to do all
the work, and it just presents some simple types to the user (which hopefully
reduce the likelihood of the user getting something seriously wrong).

For example, Nordic's library uses standard C integers for their socket file
descriptors. We have wrapped these up in a `Socket` struct, ensuring that
`nrf_socket_close` is called when the `Socket` object is dropped. You can also
no longer pass arbitrary integers to the `read` and `write` functions, and
instead you call methods on the `Socket` type.

We have further specialised the `Socket` into `TlsSocket`, `AtSocket`,
`GnssSocket` and `TcpSocket`, each with their own factory functions and
special methods. Support for UDP datagrams and other sorts of sockets is TBD -
pull requests are welcome!

If you want to make a TLS connection, you need to first push the certificates
and keys into a special area of flash controlled by the Nordic library. You
can do this with the `provision_certificates` function. Each certificate or
key is given a unique integer tag (by you), and you pass these tags when you
create the `TlsSocket` so the stack knows which certificates you want to use.
You at least need to supply a root certificate to be used for verifying the
server-side certificate. You can optionally also supply a client-side
certificate and private key, for performing client authentication.

## What Currently Works

* Opening plain TCP connections, including DNS lookups of host-names
* Opening TLS connections, with and without client-side certificates
* Opening an AT socket, sending AT commands and receiving responses
* Opening a GNSS socket and getting a GNSS fix
* Polling on sockets
* Configuring the chip for LTE-M, NB-IoT and/or GNSS mode.

## Example

See [nrf9160-demo](https://github.com/42-technology-ltd/nrf9160-demo) for a demo application that uses this library.

## Changelog

### Unreleased Changes ([Source](https://github.com/42-technology-ltd/nrfxlib/tree/master) | [Changes](https://github.com/42-technology-ltd/nrfxlib/compare/v0.5.0...master))

* Updated to nrfxlib version 1.4.2. This requires Rust v1.51 as we use the new resolver to allow bindgen as a build-dep.

### v0.5.0 ([Source](https://github.com/42-technology-ltd/nrfxlib/tree/v0.5.0) | [Changes](https://github.com/42-technology-ltd/nrfxlib/compare/v0.4.0...v0.5.0))

* Updated to nrfxlib version 1.2.0
* Certificates now handled through AT commands.

### v0.4.0 ([Source](https://github.com/42-technology-ltd/nrfxlib/tree/v0.4.0) | [Changes](https://github.com/42-technology-ltd/nrfxlib/compare/v0.3.0...v0.4.0))

* Add TLS v1.3 and DTLS v1.2 support

### v0.3.0 ([Source](https://github.com/42-technology-ltd/nrfxlib/tree/v0.3.0) | [Changes](https://github.com/42-technology-ltd/nrfxlib/compare/v0.2.2...v0.3.0))

* Derive `clone` for `Error`.
* Update to latest `nrxflib-sys` crate.
* Update wrappers for updated GPS API.

### v0.2.2 ([Source](https://github.com/42-technology-ltd/nrfxlib/tree/v0.2.2) | [Changes](https://github.com/42-technology-ltd/nrfxlib/compare/v0.2.1...v0.2.2))

* Fixed changelog in README.

### v0.2.1 ([Source](https://github.com/42-technology-ltd/nrfxlib/tree/v0.2.1) | [Changes](https://github.com/42-technology-ltd/nrfxlib/compare/v0.2.0...v0.2.1))

* Change PollEntry so it holds a const-reference rather than a mutable-reference to the socket.
* Use latest nrfxlib-sys crate.

### v0.2.0 ([Source](https://github.com/42-technology-ltd/nrfxlib/tree/v0.2.0) | [Changes](https://github.com/42-technology-ltd/nrfxlib/compare/v0.1.0...v0.2.0))

* Changed `modem::start()` to `modem::on()` and removed called to AT+COPS=0.
* Add wrapper for `nrf_poll` to pend on multiple sockets at once.
* Added `GnssSocket::get_blocking_fix()`
* Added API to get/set the System Mode.
* Added 'use_case' socket option for GPS.
* Use git version of `nrfxlib-sys` which has a cargo-5730 workaround.

### v0.1.0 ([Source](https://github.com/42-technology-ltd/nrfxlib/tree/v0.1.0))

First release.

## Licence

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
