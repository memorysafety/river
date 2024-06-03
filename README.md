# River

`river` is a Reverse Proxy Application based on the `pingora` library from Cloudflare.

## Current State

We reached the [initial v0.2.0 release] at the end of April (and a small [v0.2.1 release]
for crates.io availability in May), completing the work in [Kickstart Spike 1].

As of the end of May, work towards the next features in [Kickstart Spike 2] have begun.

The next work is focused on:

1. Development of "multiple upstream" features, including:
    * Supporting Load Balancing of upstream servers
    * Supporting Health Checks of upstream servers
    * Supporting Service Discovery of upstream servers
2. Developer and Operator Quality of Life features, including:
    * Supporting basic static HTML file serving
    * Supporting semi-dynamic observability endpoints, e.g. for Prometheus polling
    * Support for hot-reloading of configuration
    * CI for build and test checks on pull requests
3. Development of initial Robustness features, including:
    * Rate limiting of connections and/or requests
    * CIDR/API range-based filtering for rejecting connections

Stay tuned for updates on these features!

[initial v0.2.0 release]: https://github.com/memorysafety/river/releases/tag/v0.2.0
[v0.2.1 release]: https://github.com/memorysafety/river/releases/tag/v0.2.1
[Kickstart Spike 1]: https://github.com/memorysafety/river/milestone/1
[Kickstart Spike 2]: https://github.com/memorysafety/river/milestone/3

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
