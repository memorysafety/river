# Configuration

River has three sources of configuration:

1. Command Line Options
2. Environment Variable Options
3. Configuration File Options

When configuration options are available in multiple sources, priority is given in
the order specified above.

## Configuration File Options

The majority of configuration options are provided via configuration file, allowing
users of River to provide files as part of a regular deployment process. Currently,
all configuration of Services (and their Listener, Connector, and Path Control
options) are provided via configuration file.

At the current moment, two configuration file formats are supported:

* [KDL] - the current preferred format
* TOML - likely to be removed soon

[KDL]: https://kdl.dev/

For more information about configuration parameters available, see
[The KDL Configuration Format] section for more details.

[The KDL Configuration Format]: ./kdl.md

## Environment Variable Options

At the moment, there are no options configurable via environment variables.

In the future, environment variables will be used for configuration of
"secrets", such as passwords used for basic authentication, or bearer tokens
used for accessing management pages.

It is not expected that River will make all configuration options available
through environment variables, as highly structured configuration (e.g. for
Services) via environment variable requires complex and hard to reason about
logic to parse and implement.

## Command Line Options

A limited number of options are available via command line. These options
are intended to provide information such as the path to the configuration
file.

It is not expected that River will make all configuration options available
through CLI.

For more information about options that are available via Command Line
Interface, please refer to [The CLI Interface Format].

[The CLI Interface Format]: ./cli.md
