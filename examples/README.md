## Examples for Hax

The goal of this directory is to serve as a snapshot of the current
F* libraries and verified examples using Hax.

The dependency chain is:

rust_primitives <- core <- hax_lib <- examples

# Rust Primitives

The `/rust_primitives` directory contains hand-written models for Rust
built-in features like machine integers and arrays. In particular, the
code in this directory reconciles any type or semantic differences
between Rust and F*. A number of files in this directory use the HACL
Library.

# Core & Alloc

The `/core` directory contains hand-written models for some parts of
the Core and Alloc libraries of Rust.

As a first goal, we would like to typecheck the code in this directory
against interfaces generated from Rust Core and Alloc.

As a second goal, we would like to generate the code in this directory
from an annotated version of Rust Core and Alloc.

# Hax Library

The `/hax_lib` directory contains hand-written and generated code
for the Hax library which adds new features and functionality to Rust
to help programmers. For example, this library includes bounded indexes
for arrays, unbounded integers etc.

# Examples

The `/lob` directory includes an example inspired by the Internet Computer.

The `/kyber-compress` directory includes a minimal example taken from Kyber.

The `/kyber` directory contains a relatively complete extraction of Kyber.



