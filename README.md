# Asynchronous (Discrete Event) Simulator

A discrete event simulator that let you specify application logic in asynchronous functions.
The main goal of this crate is to make simulation code look very similar to that of a real-world system..
This is achieved by providing an API similar to that of the standard library or tokio but with an implementation based on discrete events.

This crate provides the simulator itself, a timer that allows an asynchronous task to sleep, and synchronization primitives.
Additionally, it includes basic primitives for processes and links that can be used to jump start your process.

## Project Status
This project is still in early development and many parts of the API are subject to change.

Pull requests for additional synchronization primitives or other functionality are very welcome.
