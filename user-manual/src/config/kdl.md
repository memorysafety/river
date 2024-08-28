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
            "91.107.223.4:443" tls-sni="onevariable.com" proto="h2-or-h1"
        }
        path-control {
            request-filters {
                filter kind="block-cidr-range" addrs="192.168.0.0/16, 10.0.0.0/8, 2001:0db8::0/32, 127.0.0.1"
            }
            upstream-request {
                filter kind="remove-header-key-regex" pattern=".*(secret|SECRET).*"
                filter kind="upsert-header" key="x-proxy-friend" value="river"
            }
            upstream-response {
                filter kind="remove-header-key-regex" pattern=".*ETag.*"
                filter kind="upsert-header" key="x-with-love-from" value="river"
            }
        }
        rate-limiting {
            timeout millis=100

            rule kind="source-ip" \
                max-buckets=4000 tokens-per-bucket=10 refill-qty=1 refill-rate-ms=10

            rule kind="uri" pattern="static/.*" \
                max-buckets=2000 tokens-per-bucket=20 refill-qty=5 refill-rate-ms=1
        }
    }
    Example3 {
        listeners {
            "0.0.0.0:9000"
            "0.0.0.0:9443" cert-path="./assets/test.crt" key-path="./assets/test.key" offer-h2=true
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

`"SOCKETADDR" [tls-sni="DOMAIN"] [proto="PROTO"]`

`SOCKETADDR` is a UTF-8 string that is parsed into an IPv4 or IPv6 address and port.

If the connector should use TLS for connections to the upstream server, the TLS-SNI
is specified in the form `tls-sni="DOMAIN"`, where DOMAIN is a domain name. If this
is not provided, connections to upstream servers will be made without TLS.

The protocol used to connect with the upstream server us specified in the form
`proto="PROTO"`, where `PROTO` is a string with one of the following values:

* `h1-only`: Only HTTP1.0 will be used to connect
* `h2-only`: Only HTTP2.0 will be used to connect
* `h2-or-h1`: HTTP2.0 will be preferred, with fallback to HTTP1.0

The `proto` field is optional. If it is not specified and TLS is configured, the default
will be `h2-or-h1`. If TLS is not configured, the default will be `h1-only`, and any
other option will result in an error.

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

Each path control filter allows for modification or rejection at different stages of request
and response handling.

This section is optional.

Example:

```kdl
path-control {
    request-filters {
        filter kind="block-cidr-range" addrs="192.168.0.0/16, 10.0.0.0/8, 2001:0db8::0/32, 127.0.0.1"
    }
    upstream-request {
        filter kind="remove-header-key-regex" pattern=".*(secret|SECRET).*"
        filter kind="upsert-header" key="x-proxy-friend" value="river"
    }
    upstream-response {
        filter kind="remove-header-key-regex" pattern=".*ETag.*"
        filter kind="upsert-header" key="x-with-love-from" value="river"
    }
}
```

#### `services.$NAME.path-control.request-filters`

Filters at this stage are the earliest. Currently supported filters:

* `kind = "block-cidr-range"`
    * Arguments: `addrs = "ADDRS"`, where `ADDRS` is a comma separated list of IPv4 or IPv6 addresses or CIDR address ranges.
    * Any matching source IP addresses will be rejected with a 400 error code.

#### `services.$NAME.path-control.upstream-request`

* `kind = "remove-header-key-regex"`
    * Arguments: `pattern = "PATTERN"`, where `PATTERN` is a regular expression matching the key of an HTTP header
    * Any matching header entry will be removed from the request before forwarding
* `kind = "upsert-header"`
    * Arguments: `key="KEY" value="VALUE"`, where `KEY` is a valid HTTP header key, and `VALUE` is a valid HTTP header value
    * The given header will be added or replaced to `VALUE`

#### `services.$NAME.path-control.upstream-response`

* `kind = "remove-header-key-regex"`
    * Arguments: `pattern = "PATTERN"`, where `PATTERN` is a regular expression matching the key of an HTTP header
    * Any matching header entry will be removed from the response before forwarding
* `kind = "upsert-header"`
    * Arguments: `key="KEY" value="VALUE"`, where `KEY` is a valid HTTP header key, and `VALUE` is a valid HTTP header value
    * The given header will be added or replaced to `VALUE`

### `services.$NAME.rate-limiting`

This section contains the configuration for rate limiting rules.

Rate limiting rules are used to limit the total number of requests made by downstream clients,
based on various criteria.

Note that Rate limiting is on a **per service** basis, services do not share rate limiting
information.

This section is optional.

Example:

```
rate-limiting {
    timeout millis=100

    rule kind="source-ip" \
        max-buckets=4000 tokens-per-bucket=10 refill-qty=1 refill-rate-ms=10

    rule kind="uri" pattern="static/.*" \
        max-buckets=2000 tokens-per-bucket=20 refill-qty=5 refill-rate-ms=1
}
```

#### `services.$NAME.rate-limiting.timeout`

The `timeout` parameter is used to set the total timeout for acquiring all rate limiting tokens.

If acquiring applicable rate limiting tokens takes longer than this time, the request will not be
forwarded and will respond with a 429 error.

This parameter is mandatory if the `rate-limiting` section is present.

This is specified in the form:

`timeout millis=TIME`, where `TIME` is an unsigned integer

**Implementation Note**: The rate limiting timeout is a temporary implementation detail to limit
requests from waiting "too long" to obtain their tokens. In the future, it is planned to modify
the leaky bucket implementation to instead set an upper limit on the maximum "token debt", or
how many requests are waiting for a token. Instead of waiting and timing out, requests will instead
be given immediate feedback that the rate limiting is overcongested, and return a 429 error immediately,
instead of after a given timeout.

When this change occurs, the `timeout` parameter will be deprecated, and replaced with a `max-token-debt`
parameter instead.

#### `services.$NAME.rate-limiting.rule`

Rules are used to specify rate limiting parameters, and applicability of rules to a given request.

##### Leaky Buckets

Rate limiting in River uses a [Leaky Bucket] model for determining whether a request can be served
immediately, or if it should be delayed (or rejected). For a given rule, a "bucket" of "tokens"
is created, where one "token" is required for each request.

The bucket for a rule starts with a configurable `tokens-per-bucket` number. When a request arrives,
it attempts to take one token from the bucket. If one is available, it is served immediately. Otherwise,
the request waits in a first-in, first-out order for a token to become available.

The bucket is refilled at a configurable rate, specified by `refill-rate-ms`, and adds a configurable
number of tokens specified by `refill-qty`. The number of tokens in the bucket will never exceed the
initial `tokens-per-bucket` number.

Once a refill occurs, requests may become ready if a token becomes available.

[Leaky Bucket]: https://en.wikipedia.org/wiki/Leaky_bucket

##### How many buckets?

Some rules require many buckets. For example, rules based on the source IP address will create a bucket
for each unique IP address of downstream users.

However, each of these buckets require space to contain the metadata, and to avoid unbounded growth,
we allow for a configurable `max-buckets` number, which serves to influence the total memory required
for storing buckets. This uses an [Adaptive Replacement Cache]
to allow for concurrent access to these buckets, as well as the ability to automatically buckets that
are not actively being used (somewhat similar to an LRU or "Least Recently Used" cache).

[Adaptive Replacement Cache]: https://docs.rs/concread/latest/concread/arcache/index.html

There is a trade off here: The larger `max-buckets` is, the longer that River can "remember" a bucket
for a given factor, such as specific IP addresses. However, it also requires more resident memory to
retain this information.

If `max-buckets` is set too low, then buckets will be "evicted" from the cache, meaning that subsequent
requests matching that bucket will require the creation of a new bucket (with a full set of tokens),
potentially defeating the objective of accurate rate limiting.

##### Gotta claim 'em all

When multiple rules apply to a single request, for example rules based on both source IP address,
and the URI path, then a request must claim ALL applicable tokens before proceeding. If a given IP
address is making it's first request, but to a URI that that has an empty bucket, it will immediately
obtain the IP address token, but be forced to wait until the URI token has been claimed

##### Kinds of Rules

Currently two kinds of rules are supported:

* `kind="source-ip"` - this tracks the IP address of the requestor. A unique bucket will be created for
  the IPv4 or IPv6 address of the requestor.
* `kind="uri" pattern="REGEX"` - This tracks the URI path of the request, such as `static/images/example.jpg`
    * If the request's URI path matches the provided `REGEX`, the full URI path will be assigned to a given
      bucket
    * For example, if the regex `static/.*` was provided:
        * `index.html` would not match this rule, and would not require obtaining a token
        * `static/images/example.jpg` would match this rule, and would require obtaining a token
        * `static/styles/example.css` would also match this rule, and would require obtaining a token
        * Note that `static/images/example.jpg` and `static/styles/example.css` would each have a UNIQUE
          bucket.

### `services.$NAME.file-server`

This section is only allowed when `connectors` and `path-control` are not present.

This is used when serving static files, rather than proxying connections.

### `services.$NAME.file-server.base-path`

This is the base path used for serving files. ALL files within this directory
(and any children) will be available for serving.

This is specified in the form `base-path "PATH"`, where `PATH` is a valid UTF-8 path.

This section is required.
