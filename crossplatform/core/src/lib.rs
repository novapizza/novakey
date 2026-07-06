//! novakey-core
//!
//! Platform-agnostic Telex Vietnamese IME engine — a faithful Rust
//! reimplementation of the NovaKey macOS Swift engine (`Sources/NovaKey/Engine/`).
//! No OS dependencies, so the full parity suite runs anywhere.
//!
//! Public API: [`TelexEngine`], [`EngineResult`], [`KeyClass`].
//!
//! # Output invariant
//! Every character the engine emits is a single precomposed NFC BMP scalar
//! (Latin Extended Additional / Extended-B). This is what lets the Windows
//! sender treat "1 replaced char = 1 UTF-16 unit = 1 backspace". See
//! [`assert_single_scalar_output`].

pub mod buffer;
pub mod data;
pub mod engine;
pub mod spelling;
pub mod tone;

pub use buffer::SyllableBuffer;
pub use data::{ToneMark, ViChar, VowelModifier};
pub use engine::{EngineResult, KeyClass, TelexEngine};

/// Debug-time invariant check: assert a string the engine intends to send is
/// composed only of BMP single scalars with no combining marks, so backspace
/// counting stays exact. Callable from the platform sender layer.
#[inline]
pub fn assert_single_scalar_output(text: &str) {
    debug_assert!(
        text.chars().all(|c| {
            let u = c as u32;
            // BMP only, and not a combining diacritical mark (U+0300..=U+036F).
            u <= 0xFFFF && !(0x0300..=0x036F).contains(&u)
        }),
        "engine emitted non-BMP or combining output: {:?}",
        text
    );
}
