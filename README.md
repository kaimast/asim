# Asynchronous (Discrete Event) Simulator

[![ci-badge](https://github.com/kaimast/asim/actions/workflows/ci.yml/badge.svg)](https://github.com/kaimast/asim/actions)
[![license-badge](https://img.shields.io/crates/l/asim)](https://github.com/kaimast/asim/blob/main/LICENSE)
[![crates-badge](https://img.shields.io/crates/v/asim)](https://crates.io/crates/asim)
[![docs.rs](https://img.shields.io/docsrs/asim)](https://docs.rs/asim)

A discrete event simulator that let you specify application logic in asynchronous functions.
The main goal of this crate is to make simulation code look very similar to that of a real-world system..
This is achieved by providing an API similar to that of the standard library or tokio but with an implementation based on discrete events.

This crate provides the simulator itself, a timer that allows an asynchronous task to sleep, and synchronization primitives.
Additionally, it includes basic primitives for processes and links that can be used to jump start your process.

## Project Status
This project is still in early development and many parts of the API are subject to change.

Pull requests for additional synchronization primitives or other functionality are very welcome.

## Similar Crates
The following crates that also implement event simulation exist. Both of them are more mature but do not use async Rust like this crate.

* [sim](https://docs.rs/sim/latest/sim/)
* [desim](https://docs.rs/desim/latest/desim/)
