# River

`river` is a Reverse Proxy Application based on the `pingora` library from Cloudflare.

## Current State

River is currently v0.5.0. See the [v0.5.0 release notes] for more details on recently
added features.

[v0.5.0 release notes]: https://github.com/memorysafety/river/blob/main/docs/release-notes/2024-08-30-v0.5.0.md

**Until further notice, there is no expectation of stability.**

### Demonstration steps

At the moment, `river` can be invoked from the command line. See `--help` for
all options.

Configuration is currently done exclusively via configuration file. See
[`test-config.kdl`] for an example configuration file. Additionally, see
[kdl configuration] for more configuration details.

[`test-config.kdl`]: ./source/river/assets/test-config.kdl
[kdl configuration]: https://onevariable.com/river-user-manual/config/kdl.html

## License

Licensed under the Apache License, Version 2.0: ([LICENSE-APACHE](./LICENSE-APACHE)
or <http://www.apache.org/licenses/LICENSE-2.0>).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
