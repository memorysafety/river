# River

`river` is a Reverse Proxy Application based on the `pingora` library from Cloudflare.

## Current State

As part of the initial [Kickstart Spike], we are working towards an early preview of
the `river` tool.

[Kickstart Spike]: https://github.com/memorysafety/river/milestone/1

**Until further notice, there is no expectation of stability.**

### Demonstration steps

At the moment, `river` can be invoked from the command line. See `--help` for
all options.

Configuration is currently done exclusively via configuration file. See
[`test-config.toml`] for an example configuration file. Additionally, see
[`toml-configuration.md`] for more configuration details.

[`test-config.toml`]: ./source/river/assets/test-config.toml
[`toml-configuration.md`]: ./docs/toml-configuration.md

## License

Licensed under the Apache License, Version 2.0: ([LICENSE-APACHE](./LICENSE-APACHE)
or <http://www.apache.org/licenses/LICENSE-2.0>).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
