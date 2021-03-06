# janus-app

This is an experimental high-level Rust binding to [Janus Gateway](https://github.com/meetecho/janus-gateway)'s application plugin API.

**WARNING!** This project is an experimental and not tested WIP. Don't try to use it for creating actual plugins.

## The concept

There's already [janus-plugin](https://github.com/mozilla/janus-plugin-rs) crate which enables creating plugins for Janus Gateway but its API is [too low-level and unsafe](https://github.com/mozilla/janus-plugin-rs/issues/10).

This crate enables writing plugins in a more idiomatic Rust way:

* Plugin code has nothing to do with raw pointers and unsafe C functions. These things are abstracted out by the crate.
* A plugin is a trait implementation, not a bunch of `extern "C"` functions.
* Object-oriented API instead of procedural.
* A plugin and each of its handles may have their state.
* Plugin handles' core C state is not mixed together with plugin's Rust state.
* Dispatching callbacks from other plugin's threads is thread safe. This enables the plugin to handle messages and media events in an asynchronous non-blocking way.
* [Serde](https://github.com/serde-rs/serde) library is being used for (de)serialization within the plugin as de-facto Rust's standard. No need to tackle C's [Jansson](https://github.com/akheron/jansson) that is being used on the low-level API.
* Unit testing is possible for plugins because they aren't coupled to C code.

## Example plugin

For an example plugin see `example` sub-crate.

You can build a docker image with Janus Gateway bundled with the plugin:

```bash
docker build -t janus-app-example:latest -f docker/Dockerfile .
docker run --rm -it -p 8188:8188 janus-app-example:latest
```

## Documentation

Build documentation with:

```bash
cargo doc --no-deps
```

then open `target/doc/janus_app/index.html`.
