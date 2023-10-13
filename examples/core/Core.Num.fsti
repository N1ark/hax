module Core.Num
open Rust_primitives

let impl__u32__wrapping_add: u32 -> u32 -> u32 = add_mod
val impl__u32__rotate_left: u32 -> u32 -> u32
val impl__u32__from_le_bytes: array u8 (sz 4) -> u32
val impl__u32__from_be_bytes: array u8 (sz 4) -> u32
val impl__u32__to_le_bytes: u32 -> array u8 (sz 4)
val impl__u32__to_be_bytes: u32 -> array u8 (sz 4)
val impl__u32__rotate_right: u32 -> u32 -> u32


open FStar.UInt64
let impl__u64__wrapping_add: u64 -> u64 -> u64 = add_underspec
val impl__u64__rotate_left: u32 -> u32 -> u32
val impl__u64__from_le_bytes: array u8 (sz 8) -> u64
val impl__u64__from_be_bytes: array u8 (sz 8) -> u64
val impl__u64__to_le_bytes: u64 -> array u8 (sz 8)
val impl__u64__to_be_bytes: u64 -> array u8 (sz 8)
val impl__u64__rotate_right: u64 -> u64 -> u64


open FStar.UInt128
let impl__u128__wrapping_add (x: u128) (y: u128): u128 = add_underspec x y
val impl__u128__rotate_left: u128 -> u128 -> u128
val impl__u128__from_le_bytes: array u8 (sz 16) -> u128
val impl__u128__from_be_bytes: array u8 (sz 16) -> u128
val impl__u128__to_le_bytes: u128 -> array u8 (sz 16)
val impl__u128__to_be_bytes: u128 -> array u8 (sz 16)
val impl__u128__rotate_right: u128 -> u128 -> u128

val impl__u8__pow: u8 -> u32 -> u8
val impl__u16__pow: u16 -> u32 -> u16
val impl__u32__pow: u32 -> u32 -> u32
val impl__u64__pow: u64 -> u32 -> u64
val impl__u128__pow: u128 -> u32 -> u128

