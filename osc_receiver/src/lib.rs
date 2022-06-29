#![cfg_attr(not(feature = "pi"), no_std)]

#[cfg(feature = "pi")]
extern crate std as core;

extern crate alloc;

pub mod led_strip;
