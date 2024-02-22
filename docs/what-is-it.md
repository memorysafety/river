# River - What is it?

This is a document that is intended to describe WHAT River is, without discussing the details of how
it is or should be implemented.

This document is intended for potential users of the River application, with a secondary goal of
serving as a "big picture" view for implementers.

> The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL
> NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED",
> "MAY", and "OPTIONAL" in this document are to be interpreted as
> described in [BCP 14] [RFC2119] [RFC8174] when, and only when, they
> appear in all capitals, as shown here.

[BCP 14]: https://www.rfc-editor.org/info/bcp14
[RFC2119]: https://datatracker.ietf.org/doc/html/rfc2119
[RFC8174]: https://datatracker.ietf.org/doc/html/rfc8174

## 1 - Abstract

River is a reverse proxy application under design, utilizing the `pingora` reverse proxy engine from
Cloudflare. It will be written in Rust. It will be configurable, allowing for options including
routing, filtering, and modification of proxied requests.

The intent is for River to act as a binary distribution of the `pingora` engine - providing
a typical application interface for configuration and customization for operators.

For more information on fundamentals of the `pingora` library, please refer to [the `pingora`
overview document](./pingora-overview.md). For more detailed information on an implementation level,
please refer to [the 'what to build' document](./what-to-build.md).

The remainder of this document describes the intended behavior of the River application, including
the subset of capabilities provided by the `pingora` library today.

## 2 - Functional Description

The primary behavior of a reverse proxy application is to act as an intermediary between downstream
clients and upstream servers, including termination of TLS for inbound connections if in use. The
reverse proxy application may decide to accept or reject the connection at any point, and may decide
to modify messages at any point.

```text
┌────────────┐          ┌─────────────┐         ┌────────────┐
│ Downstream │       ┌ ─│─   Proxy  ┌ ┼ ─       │  Upstream  │
│   Client   │─────────▶│ │           │──┼─────▶│   Server   │
└────────────┘       │  └───────────┼─┘         └────────────┘
                      ─ ─ ┘          ─ ─ ┘
                        ▲              ▲
                     ┌──┘              └──┐
                     │                    │
                ┌ ─ ─ ─ ─ ┐         ┌ ─ ─ ─ ─ ─
                 Listeners           Connectors│
                └ ─ ─ ─ ─ ┘         └ ─ ─ ─ ─ ─
```

*Figure 1: Proxying Behavior*

### 2.1 - Downstream

River operates by listening to one or more downstream Listener interfaces, accepting connections
from clients.

```text
┌────────────┐
│ Downstream │
│   Client   │───┐
└────────────┘   │
┌────────────┐   │   ┌─────────────┐       ┌─────────────┐
│ Downstream │   │   │  Listener   │       │    Proxy    │
│   Client   │───┼──▶│             │──────▶│             │
└────────────┘   │   └─────────────┘       └─────────────┘
┌────────────┐   │
│ Downstream │   │
│   Client   │───┘
└────────────┘
```

*Figure 2: Listeners*

1. River MUST accept connections via:
    1. Unix Domain Sockets
    2. TCP Sockets
        1. IPv4
        2. IPv6
2. River MUST support the termination of TLS sessions
3. River MUST support the specification of TLS algorithms used for a given downstream listener as a
   subset of all supported algorithms
4. River MUST support the proxying of:
    1. HTTP0.x/HTTP1.x connections
    2. HTTP2.0 connections
5. River MAY support the proxying of:
    1. HTTP3.0 connections.
6. River MUST support receiving information from protocols used for pre-proxying, including:
    1. v1 and v2 of the PROXY protocol
    2. Cloudflare Spectrum
    3. Akamai X-Forwarded-For (XFF) HTTP header field

### 2.2 - Upstream

River operates by making and maintaining connections to one or more upstream services, forwarding
messages from clients.

```text
                                           ┌ ─ ─ ─ ─ ─ ─ ─ ─
                                             ┌────────────┐ │
                                           │ │  Upstream  │
                                        ┌───▶│   Server   │ │
                                        │  │ └────────────┘
┌─────────────┐       ┌─────────────┐   │    ┌────────────┐ │
│    Proxy    │       │  Connector  │   │  │ │  Upstream  │
│             │──────▶│             │───┘    │   Server   │ │
└─────────────┘       └─────────────┘      │ └────────────┘
                                             ┌────────────┐ │
                                           │ │  Upstream  │
                                             │   Server   │ │
                                           │ └────────────┘
                                            ─ ─ ─ ─ ─ ─ ─ ─ ┘
```

*Figure 3: A connector communicating with 1/N upstream servers*

1. River MUST support a configurable Time To Live (TTL) for DNS lookups
2. River MUST support a configurable timeouts for:
    1. Connections
    2. Requests
    3. Successful health checks
3. River MUST support pooling of connections, including:
    1. Reuse of TCP sessions for all HTTP versions
    2. Reuse of HTTP2.0 streams for HTTP2.0
4. River MUST support health checks of upstream servers
5. River MUST support the disabling of use of an upstream server based on failed health checks
5. River MUST support load balancing of upstream servers
6. River MUST support sending information for protocols used for pre-proxying, including:
    1. v1 and v2 of the PROXY protocol
    2. Cloudflare Spectrum
    3. Akamai X-Forwarded-For (XFF) HTTP header field
7. River MUST support the configurable selection of a subset of upstream servers based on HTTP URI
   paths

### 2.3 - Upstream Service Discovery

```text
                                           ┌────────────────┐
                                           │Upstream Server │
                             ┌────────────▶│ Listing Source │
                             │             └────────────────┘
                          Service
                         Discovery         ┌ ─ ─ ─ ─ ─ ─ ─ ─
                          Requests           ┌────────────┐ │
                             │             │ │  Upstream  │
                             │          ┌───▶│   Server   │ │
                             ▼          │  │ └────────────┘
┌─────────────┐       ┌─────────────┐   │    ┌────────────┐ │
│    Proxy    │       │  Connector  │   │  │ │  Upstream  │
│             │──────▶│             │───┘    │   Server   │ │
└─────────────┘       └─────────────┘      │ └────────────┘
                             │               ┌────────────┐ │
                        Server List        │ │  Upstream  │
                          Update             │   Server   │ │
                                           │ └────────────┘
                             └ ─ ─ ─ ─ ─ ─▶ ─ ─ ─ ─ ─ ─ ─ ─ ┘
```

*Figure 4: Using Service Discovery to update the list of upstream servers*

River allows for the configurable runtime discovery of upstream servers, in order to dynamically
handle changing sets of upstream servers without requiring a restart or reconfiguration of the
application.

1. River MUST support the use of a fixed list of upstream servers
2. River MUST support the use of DNS-Service Discovery to provide a list of upstream servers for a
   given service
3. River MUST support the use of SRV records to provide a list of upstream servers for a given
   service
4. River MUST have a configurable timeout for re-polling poll-based service discovery mechanisms
5. River MUST support the use of DNS TTL as timeout value for re-polling poll-based service
   discovery mechanisms

### 2.4 - Request Path Control

River allows for configurable behavior modifiers at multiple stages in the request and response
process. These behaviors allow for the modification or rejection of messages exchanged in either
direction between downstream client and upstream server

```text
             ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐  ┌ ─ ─ ─ ─ ─ ─ ┐
                  ┌───────────┐    ┌───────────┐    ┌───────────┐
             │    │  Request  │    │           │    │  Request  │    │  │             │
 Request  ═══════▶│  Arrival  │═══▶│Which Peer?│═══▶│ Forwarded │═══════▶
             │    │           │    │           │    │           │    │  │             │
                  └───────────┘    └───────────┘    └───────────┘
             │          │                │                │          │  │             │
                        │                │                │
             │          ├───On Error─────┼────────────────┤          │  │  Upstream   │
                        │                │                │
             │          │          ┌───────────┐    ┌───────────┐    │  │             │
                        ▼          │ Response  │    │ Response  │
             │                     │Forwarding │    │  Arrival  │    │  │             │
 Response ◀════════════════════════│           │◀═══│           │◀═══════
             │                     └───────────┘    └───────────┘    │  │             │
               ┌────────────────────────┐
             └ ┤ Simplified Phase Chart │─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘  └ ─ ─ ─ ─ ─ ─ ┘
               └────────────────────────┘
```

*Figure 5: Request Path Lifecycle*

1. River MUST support modifying or rejecting a connection at any of the following stages:
    1. Downstream Request Arrival
    2. Peer Selection
    3. Upstream Request Forwarding
    4. Upstream Response Arrival
    5. Downstream Request Forwarding
    6. Request Body (partial request fragments)
    7. Response Body (partial response fragments)
2. River MUST support rejecting a connection by returning an error response
3. River MUST support CIDR/API range-based filtering allow and deny lists
4. River MUST support rate limiting of requests or responses on the basis of one or
   more of the following:
    1. A fixed rate per second
    2. A "burst" rate - allowing for short increases above the fixed rate
5. River MUST support application of rate limiting of requests or responses on the per-endpoint
   basis.
6. River MUST support removal of HTTP headers on a glob or regex matching basis
7. River MUST support addition of fixed HTTP headers to a request
8. River MUST support the normalization of request and response headers and bodies, including:
    1. URI normalization
    2. Text encoding


### 2.5 - Observability

River allows for configurable observability settings, in order to support the operation and
maintenance of the system.

1. River MUST support emitting structured logs at a configurable level
2. River MUST provide quantitative metrics and performance counters
3. River MUST support push- and pull-based methods of obtaining structured logs, metrics, and
   performance counters
4. River MUST support emitting logs and metrics locally to file, stdout, or stderr in a consistently
   structured format.

### 2.6 - Configuration

River MUST provide methods for configuration in order to control the behavior of the reverse proxy
application

1. River MUST support the configuration of all configurable options via a human editable text file
   (e.g. TOML, YAML).
2. River MUST support emitting a configuration file containing all configuration items and default
   configuration settings as a command line option
3. River MUST support a subset of configuration options at the command line
4. River MUST document all command line configurable options via a help command
5. River MUST support a subset of configuration options via environment variables
6. River MUST support emitting a list of all configuration options configurable via environment
   variables as a command line option
7. River MUST give the following priority to configuration:
    1. Command Line Options (highest priority)
    2. Environment Variable Options
    3. Configuration File Options (lowest priority)

### 2.7 - Environmental Requirements

These requirements relate to the supported execution environment(s) of the application.

1. The application MUST support execution in a Linux environment
2. The application MAY support execution in operating systems such as MacOS, Windows, or Redox OS.
3. The application MUST support execution within a container
4. The application MUST support two stages of execution:
    1. The first stage MUST execute with the user and group used to launch the application, and
       perform initial setup steps
    2. The second stage MUST be forked from the first stage, executing with the user and group
       specified in the application configuration
5. The application MUST support execution without "root" or "administrator" privileges, given that:
    1. The user and group used to launch the application has the capability to fork the second stage
    2. The user and group used to fork the second stage has capabilities necessary for steady state
       operation.

### 2.8 - Graceful Reloading

These requirements relate to the feature of "graceful reloading" - allowing for stopping one
instance (referred to as the "Old" instance) of the application and the starting of a second
instance (referred to as the "New" instance), handing off existing connections where possible.

1. The application MUST support the passing of open Listeners from one instance of the application
   to another.
2. The application MUST support the configuration of an upgrade socket used for both giving and
   receiving the current Listeners.
3. The application MUST allow for a configurable period of time before the termination of in-flight
   requests handled by the "Old" instance.
4. The application MUST allow for a configurable period of time before the termination of active
   connections handled by the "Old" instance if unable to transfer to the "New" instance.
5. The "Old" instance of the application MUST terminate after all in-flight requests and active
   connections have been transferred to the "New" instance or have been closed after timing out.

### 2.9 - Certificate Provisioning and Management

These requirements relate to the features of obtaining or renewing TLS certificates automatically
without user interaction.

1. The application MUST support the use of the Automatic Certificate Management Environment (ACME)
   protocol to obtain new TLS certificates.
2. The application MUST support the use of ACME protocol to renew TLS certificates.
3. The application MUST support the configuration of domain names to be managed (including obtaining
   and renewal steps) automatically
4. The application MUST support both fully qualified and wildcard domains.
5. The application MUST support configuration of certificate renewal interval, from either:
    1. The number of days since the certificate was acquired
    2. The number of days until the certificate will expired
6. The application MUST support API Version 2 of the ACME protocol
7. The application MAY support API Version 1 of the ACME protocol

## 3 - Development Practices

The following are development practice requirements for initial implementers of River.

### 3.1 - Documentation Practices

These requirements relate to the technical documentation of River.

1. The implementers MUST maintain complete developer-facing documentation, or "doc comments"
    1. This MAY be achieved using the `#![deny(missing_docs)]` directive or similar flags in CI
       testing
2. The implementers MUST maintain a separate user-facing documentation, describing usage,
   configuration, installation, and other details and examples.
    1. This MAY be achieved using a tool such as `mdBook`, creating a user facing "Book" for River
3. The implementers MUST automatically publish the developer- and user- facing documentation for
   all released versions
4. The implementers MUST automatically publish the developer- and user- facing documentation for
   the main development branch
    1. This MAY be on a per-pull request basis, or on a scheduled basis e.g. once per day.
5. The implementers MUST document how to build developer- and user- facing documentation

### 3.2 - Benchmarking Practices

These requirements relate to the performance benchmarking of River. No specific performance
metrics are required or specified here, instead weight is placed on measurements over time, allowing
improvements or regressions to be visible and measurable throughout the development process.

1. The implementers MUST maintain a test suite of performance tests, expected to exercise:
    1. Typical Use Cases
    2. Unusual or "Worst Case" use cases
    3. Use cases previously reported as performance regressions
2. The implementers MUST run and record the results of performance tests on a regular basis, such
   as on every pull request, or on a scheduled daily/weekly basis.
3. The performance tests MUST track the following metrics:
    1. Peak and Average CPU usage during test execution
    2. Peak and Average Memory usage during test execution
    3. CPU and Wall Clock time of test execution
4. The performance tests MAY track the following "perf counter" metrics:
    1. Branch prediction failures
    2. Page faults
    3. Cache Misses
    4. Context Switches
5. The implementers MUST document how to build and execute performance tests
6. The implementers MAY provide a suite of comparison tests, executing a subset of performance tests
   against contemporary reverse proxy applications, such as NGINX or Apache.

### 3.3 - Continuous Integration Practices

These requirements document tooling practices expected for the development of River.

1. The implementers MUST provide a set of automated checks that are required to pass prior to merges
   to the main development branch. These automated checks MAY include:
    1. Code Formatting checks, e.g. `cargo fmt`
    2. Code linting checks, e.g. `cargo clippy`
    3. Unit test execution, e.g. `cargo test`
    4. Documentation build steps (for user- and developer- facing documentation)
    5. Integration test execution
    5. Performance test execution
2. The implementers MUST provide a set of automated checks that are required to run on a periodic
   basis. These automated checks MAY include:
    1. Building against the latest stable, beta, or nightly versions of the Rust compiler and
       toolchain
    2. Performance test execution
    3. Documentation build steps
    4. Documentation publishing steps
2. The implementers MUST provide and document the process for running all automated checks locally,
   in order to allow contributors to perform these checks prior to submitting a Pull Request.

### 3.4 - Contribution Practices

1. The implementers MUST provide and enforce a Code of Conduct for contribution
    1. The implementors MAY use the [Contributor Covenant] to achieve this goal
2. The implementers MUST provide and maintain a Contribution guide for third party contributions
3. The implementers MUST provide and maintain a security policy, to allow for private disclosure
   of vulnerabilities

[Contributor Covenant]: https://www.contributor-covenant.org/version/1/3/0/code-of-conduct/
