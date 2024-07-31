# Configuration File (KDL)

The primary configuration file format used by River uses the
[KDL Configuration Language](https://kdl.dev/).

KDL is a language for describing structured data.

There are currently two major sections used by River:

## The `system` section

Here is an example `system` configuration block:

```kdl
system {
    threads-per-service 8
    daemonize false
    pid-file "/tmp/river.pidfile"

    // Path to upgrade socket
    //
    // NOTE: `upgrade` is NOT exposed in the config file, it MUST be set on the CLI
    // NOTE: This has issues if you use relative paths. See issue https://github.com/memorysafety/river/issues/50
    // NOTE: The upgrade command is only supported on Linux
    upgrade-socket "/tmp/river-upgrade.sock"
}
```

### `system.threads-per-service INT`

This field configures the number of threads spawned by each service. This configuration
applies to all services.

A positive, non-zero integer is provided as `INT`.

This field is optional, and defaults to `8`.

### `system.daemonize BOOL`

This field configures whether River should daemonize.

The values `true` or `false` is provided as `BOOL`.

This field is optional, and defaults to `false`.

If this field is set as `true`, then `system.pid-file` must also be set.

### `system.pid-file PATH`

This field configured the path to the created pidfile when River is configured
to daemonize.

A UTF-8 absolute path is provided as `PATH`.

This field is optional if `system.daemonize` is `false`, and required if
`system.daemonize` is `true`.

### `system.upgrade-socket`

This field configured the path to the upgrade socket when River is configured
to take over an existing instance.

A UTF-8 absolute path is provided as `PATH`.

This field is optional if the `--upgrade` flag is provided via CLI, and required if
`--upgrade` is not set.

## The `services` section

Here is an example `services` block:

```kdl
services {
    Example1 {
        listeners {
            "0.0.0.0:8080"
            "0.0.0.0:4443" cert-path="./assets/test.crt" key-path="./assets/test.key" offer-h2=true
        }
        connectors {
            load-balance {
                selection "Ketama" key="UriPath"
                discovery "Static"
                health-check "None"
            }
            "91.107.223.4:443" tls-sni="onevariable.com"
        }
        path-control {
            upstream-request {
                filter kind="remove-header-key-regex" pattern=".*(secret|SECRET).*"
                filter kind="upsert-header" key="x-proxy-friend" value="river"
            }
            upstream-response {
                filter kind="remove-header-key-regex" pattern=".*ETag.*"
                filter kind="upsert-header" key="x-with-love-from" value="river"
            }
        }
    }
    Example3 {
        listeners {
            "0.0.0.0:9000"
            "0.0.0.0:9443" cert-path="./assets/test.crt" key-path="./assets/test.key"
        }
        file-server {
            // The base path is what will be used as the "root" of the file server
            //
            // All files within the root will be available
            base-path "."
        }
    }
}
```

Each block represents a single service, with the name of the service serving as
the name of the block.

### `services.$NAME`

The `$NAME` field is a UTF-8 string, used as the name of the service. If the name
does not contain spaces, it is not necessary to surround the name in quotes.

Examples:

* `Example1` - Valid, "Example1"
* `"Example2"` - Valid, "Example2"
* `"Server One"` - Valid, "Server One"
* `Server Two` - Invalid (missing quotation marks)

### `services.$NAME.listeners`

This section contains one or more Listeners.
This section is required.
Listeners are specified in the form:

`"SOCKETADDR" [cert-path="PATH" key-path="PATH" [offer-h2=BOOL]]`

`SOCKETADDR` is a UTF-8 string that is parsed into an IPv4 or IPv6 address and port.

If the listener should accept TLS connections, the certificate and key paths are
specified in the form `cert-path="PATH" key-path="PATH"`, where `PATH` is a UTF-8
path to the relevant files. If these are not provided, connections will be accepted
without TLS.

If the listener should offer HTTP2.0 connections, this is specified in the form
`offer-h2=BOOL`, where `BOOL` is either `true` or `false`. `offer-h2` may only
be specified if `cert-path` and `key-path` are present. This configuration is
optional, and defaults to `true` if TLS is configured. If this field is `true`,
HTTP2.0 will be offered (but not required). If this field is `false` then only
HTTP1.x will be offered.

### `services.$NAME.connectors`

This section contains one or more Connectors.
This section is required.
Connectors are specified in the form:

`"SOCKETADDR" [tls-sni="DOMAIN"]`

`SOCKETADDR` is a UTF-8 string that is parsed into an IPv4 or IPv6 address and port.

If the connector should use TLS for connections to the upstream server, the TLS-SNI
is specified in the form `tls-sni="DOMAIN"`, where DOMAIN is a domain name. If this
is not provided, connections to upstream servers will be made without TLS.

### `services.$NAME.connectors.load-balance`

This section defines how load balancing properties are configured for the
connectors in this set.

This section is optional.

### `services.$NAME.connectors.load-balance.selection`

This defines how the upstream server is selected.

Options are:

* `selection "RoundRobin"`
    * Servers are selected in a Round Robin fashion, giving equal distribution
* `selection "Random"`
    * Servers are selected on a random basis, giving a statistically equal distribution
* `selection "FNV" key="KEYKIND"`
    * FNV hashing is used based on the provided KEYKIND
* `selection "Ketama" key="KEYKIND"`
    * Stable Ketama hashing is used based on the provided KEYKIND

Where `KEYKIND` is one of the following:

* `UriPath` - The URI path is hashed
* `SourceAddrAndUriPath` - The Source address and URI path is hashed

### `services.$NAME.path-control`

This section contains the configuration for path control filters

### `services.$NAME.file-server`

This section is only allowed when `connectors` and `path-control` are not present.

This is used when serving static files, rather than proxying connections.

### `services.$NAME.file-server.base-path`

This is the base path used for serving files. ALL files within this directory
(and any children) will be available for serving.

This is specified in the form `base-path "PATH"`, where `PATH` is a valid UTF-8 path.

This section is required.
