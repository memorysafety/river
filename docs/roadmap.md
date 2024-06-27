# `river` roadmap - End of June, 2024

## Completed Milestones

### "Kickstart Spike 1" / v0.2.0

This work took place over the course of April 2024. The goals of this milestone were:


1. Getting the river application up and running as a Linux binary
2. Getting enough configuration options working to allow for basic operation
3. Integrating the pingora library and getting basic reverse proxy operation working
4. Start setting up build and release infrastructure
5. Start working on observability, including structured logging

For more information: https://github.com/memorysafety/river/blob/main/docs/release-notes/2024-04-29-v0.2.0.md

## In Progress Milestones

### "Spike 2.1" / v0.3.x

This is the first part of work intended to take place during June and July 2024.

This work is focused on "load balancing" use cases, including:

1. Supporting Load Balancing of upstream servers
2. Supporting Health Checks of upstream servers
3. Supporting Service Discovery of upstream servers

This work is in progress, and is wrapping up shortly.

### "Spike 2.2" / v0.4.x

This is the second part of work intended to take place during June and July 2024.

This work is focused on "Developer and Operator Quality of Life" features, including:

1. Supporting basic static HTML file serving
2. Supporting semi-dynamic observability endpoints, e.g. for Prometheus polling
3. Support for hot-reloading of configuration
4. CI for build and test checks on pull requests

### "Spike 2.3" / v0.5.x

This is the third and final part of work intended to take place during June and July 2024.

This work is focused on "initial Robustness" features, including:

1. Rate limiting of connections and/or requests
2. CIDR/API range-based filtering for rejecting connections

## Future Milestones - towards 1.0

The following milestones are working towards the requirements specified in the design document
for `river`: https://github.com/memorysafety/river/blob/main/docs/what-is-it.md

These milestones are the currently planned way of structuring major features in the approach
towards a stable 1.0 release.

### "ACME features" / v0.6.x

This future work is focused on implementing ACME protocol support, to enable automatically obtaining
and/or renewing TLS certificates from providers such as Lets Encrypt. This feature is expected to
work without active human interaction.

Requirements for this stage include:

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

### "Full Service-Discovery Features" / v0.7.x

The work in "Spike 2.1" introduced basic scaffolding for service discovery, but did not
support any "active" service discovery features outside of a static list provided on
start-up.

This work is focused on supporting a number of useful service discovery features, including:

1. River MUST support the use of DNS-Service Discovery to provide a list of upstream servers for a
   given service
2. River MUST support the use of SRV records to provide a list of upstream servers for a given
   service
3. River MUST have a configurable timeout for re-polling poll-based service discovery mechanisms
4. River MUST support the use of DNS TTL as timeout value for re-polling poll-based service
   discovery mechanisms
5. Ensure that we support the following for discovered upstreams:
    * Timeouts on connections
    * Timeouts on Requests
    * Timeouts on health checks

### "Full Path Control Features" / v0.8.x

Spike 1 introduced initial Path Control features, allowing for filtering or modification of
requests and responses.

As these filters are "fixed" working towards the 1.0 release, it is likely we will want to
build these out to cover common security and reliability use cases, including resistance to
Denial of Service attacks, or general overload.

Additionally, there is intent to implement default-enabled normalization modifications and
checks, intended to prevent against common attack vectors or programmer errors.

### Polish, packaging, and pre-release / v0.9.x+

At this stage, `river` is considered nearly feature complete for a 1.0 release. This milestone
is intended to prepare release candidates, which can be used for widespread test releases.

Particularly, this stage is also when we will want to ensure that development and operational
documentation for River is complete, and suitable for end-users who are not already familiar
with River during the early preview stages.

It is expected to potentially make any remaining breaking changes, work to ensure that River
can be packaged in a variety of expected ways, and to get user feedback with respect to
performance and usability.

### Non-Milestone Items that need to be scheduled

The following items are not necessarily "milestone" targets, but should be scheduled across
the other existing milestones:

* Building out more extensive unit, functional, user interface, and end-to-end testing
    * This also may include augmenting existing pingora tests
    * This also will include developing an integration test suite specific to river
* Building out benchmarking and regression test suites
    * These will be used to ensure addition of new features does not regress overall performance
    * The intent of these benchmarks are largely to be used relative to river itself, not
      necessarily against other existing proxying tools
* Extending and enhancing structured logging and metrics
    * We will want to instrument aspects of the proxying lifecycle, to be able to make
      meaningful measurements of river's performance over time
    * We will want to take feedback from real-world and benchmarking use cases in
      order to make it possible to debug and reason about the internal workings of
      river from an operational perspective
* Review of "UX Consistency"
    * Ensure choices regarding configuration, to ensure that options are reasonable
    * Ensure Configuration Files, Command Line, and Environment Variable interfaces
      are consistent with each other
    * Ensure emitted logs, metrics, and tracing data is consistent and readable
      for operators

### Release / v1.x.x

At this stage, `river` will make a 1.0 release.

## Far Future Milestones - Beyond 1.0

### Scripting Language

The largest open milestone which is likely to be deferred until AFTER 1.0 is the
introduction of a scripting interface and integrated scripting language. This
language is intended to allow for:

* Dynamic Path Control - allowing for modification or filtering of requests and
  responses
* Dynamic Service Discovery - allowing for discovery of new upstream servers based
  on scripted logic
* Dynamic Health Checks - allowing for more expressive or in-depth checks of upstream
  server health
* Dynamic Load Balancing - allowing for more control over delegation of requests to
  upstream servers

This work will be informed by the "baked in" choices we make towards 1.0 for all of
the above items, and will entail:

* Development of a stable API and/or language interface for performing these actions
  externally and dynamically
* Selection of a scripting language (such as WASM), as well as the execution environment
  or runtime (such as WasmTime)
* Management and loading of dynamic components as part of the application configuration
