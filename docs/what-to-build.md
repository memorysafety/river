# River - What to Build

This document is intended to describe HOW to build River, bridging the existing components provided
by Pingora, and the implementation goals for an initial public release of the River application.

This document is intended for potential implementers of the application to clarify the scope of
work, to assist them in planning and estimation of work required.

## 1 - Foreword

The intent is to build a Reverse Proxy Application (River) around the `pingora` project by
CloudFlare.

`pingora` and its related dependencies are a set of Rust crates that contain the majority of the
core logic necessary to implement a performant reverse proxy.

However, `pingora` is not a complete user facing application. It is absolutely a software library
component, and although it is deeply extensible and customizable, it is missing the "last mile" of
features and software that would allow it to be used in the way operators are used to configuring
and deploying today's reverse proxies like NGINX or Apache.

Metaphorically: it is all the parts you would find underneath the hood of a car, without any of the
body, seats, steering wheel, or other components you would find in a go-kart, dune buggy, smart car,
or sedan.

This "last mile" of implementation is the task for you: deciding how to assemble the pieces,
ensuring that the assembly works as expected, and ensuring that operators know how they are expected
to use the entire vehicle.

This document will describe what the `pingora` engine and related parts do, as well as our current
impression of what is necessary for the "minimum viable vehicle" you will be expected to develop.

It will also occasionally discuss future hopes and dreams: in the hope that knowing about these
ideas avoids design decisions today that make those future ideas more challenging to implement,
should they ever be deemed ready to develop.

That being said: priority should be given to the requirements of TODAY, over the possibilities of
TOMORROW.

## 2 - An overview of `pingora`

The remainder of this document assumes you are familiar with the `pingora` project, including the
main crate, as well as the major dependencies, primarily the `pingora-proxy` library, which provides
the main subset of HTTP proxying behavior.

Refer to the [`pingora` overview] document for an introduction, as well as
the public documentation in the `pingora` repository.

[`pingora` overview]: ./pingora-overview.md

## 3 - The missing pieces

The following are the primary pieces that we believe are missing today.

All of the following items require:

1. An implementation of all options, as well as the implementation to select each option
2. Verification that the implementation matches the specification

## 4 - Detailed Discussion

Note: This matches the organization of the [Functional Description](what-is-it.md#2---functional-description) of River.

### 4.1 - Downstream

`pingora` provides quite a bit of management of the connection life cycle already. The majority of
work necessary with regards to managing Downstreams is with respect to Configuration, selecting
options for each downstream connection.

Refer to [An Overview of Pingora][`pingora` overview] for details on how Downstreams work in the
`pingora` engine. Refer to 4.6 - Configuration below for details regarding configuration.

There is currently no support for HTTP3.0 in `pingora` as of February 2024, although it is planned
in the future. It is likely River will want to add support for HTTP3.0 once `pingora` supports it.

It is not expected that major work will be required with respect to basic HTTP 1.0/2.0 protocol
handling of Downstreams in River.

It will likely be necessary to support "pre-proxying" protocols. These are used when the proxying
should be transparent. A typical setup is:

1. The Client makes a request
2. This request goes to a third party, providing proxying or security features
3. The third party refers this request to the River server

In this case, we will need to consider the IP and details of the client, rather than the third
party, for purposes such as rate limiting, blocking, etc.

This is for two main reasons:

1. To avoid false negatives: We trust requests from malicious clients just because they were proxied
   from a trusted third party
2. To avoid false positives/unintended side effects: We begin blocking a trusted third party because
   they proxied a malicious client

These pre-proxying protocols may require some work and additional verification to ensure information
is properly forwarded, as well as ensuring there are not unintended performance or other regressions
that would make pre-proxying a denial of service vector.

### 4.2 - Upstream

Upstream support, particularly HTTP-based upstreams, including HTTP0.x, HTTP1.x, and HTTP2.0 are
already provided in the `pingora` repository.

However, it may be necessary to build out or provide configurable combinations with respect to:

* Pooling and reuse of connections, either at the TCP connection level (all HTTP versions),
  or at the stream level (HTTP2 only), particularly with respect to timeouts, back-pressure, and
  other tunables.
* Load Balancing options, including open loop control (e.g. round robin scheduling), or potentially
  closed loop control (e.g. with load feedback from upstreams)
* Health Checks of upstreams, either from response codes, or from periodic checks of health or other
  metric endpoints

Some of these functionalities are provided by (optional) crates in the `pingora` repository, or have
example code available. Development will be required both to implement behavior (either within the
existing `pingora-proxy` crate, or on top of it), as well as configuration to enable each option.

Appendix A below shows the availability of context at different phases in the connection and request
life cycle. Access to this context will allow for updating this metadata for items above.

Refer to 4.6 - Configuration below for details regarding configuration.

Refer to An Overview of Pingora above for details on how Upstreams work in the `pingora` engine,
particularly Connector entities.

There is currently no support for HTTP3.0 in `pingora` as of February 2024, although it is planned
in the future. It is likely River will want to add support for HTTP3.0 once `pingora` supports it.

### 4.3 - Upstream Service Discovery

Although the majority of configuration for the initial release will be static over the lifetime of
the application, Upstream Service Discovery is currently the only exception.

It will be expected to update the list of upstreams over time, adding and removing upstreams from
the current active list.

This will require development effort to either `pingora` or as part of pingora-proxy, as both assume
that the list of upstreams is constant.

This work is primarily in two parts:

1. Adding support for relevant Service Discovery protocols
2. Making the load balancing algorithm(s) aware of these changes

This work will also need to be designed in tandem with Configuration, making it possible to
specify the desired service discovery options in a declarative way.

### 4.4 - Request Path Control

Proxy Customization Options, allowing an operator to specifies customization of behaviors applied
to connections and requests and responses, based on fixed/running metrics or other policy, such as:

* Filtering (or rejection) based on metrics or policy
* Additional metrics gathering or logging
* Modification, adding or removing aspects of a request or response

As an implementer, the development of these Request Path Control options are like building a set
of tools that can be used by the operator. It is expected that the development of these tools, as
well as ensuring that the different tools work predictably and reliably with each other is likely
to be the largest section of development work towards the initial release of River.

Implementers are suggested to pick reasonable, safe defaults, with the goal that installation
with no configuration effort always being an acceptable (if not ideal) choice with respect to
security and performance.

It is likely that there will be additional feature requests in this area in the future, beyond the
initial requirements, including functionality such as checking authentication prior to proxying.
Care should be taken with respect to future extensibility.

This work will also need to be designed in tandem with Configuration, making it possible to
specify the desired request path control options in a declarative way.

### 4.5 - Observability

An observability system, allowing operators to inspect and make observations about the running
system, both in an exploratory way as a human, as well as an automated way as part of a larger
monitoring system.

Currently, `pingora` uses the `log` ecosystem in Rust. It may be worth investigating switching to
`tracing`, or using an integration with the `tracing` ecosystem.

There are a number of existing integrations for push based aggregation systems (e.g. OpenTracing or
OpenTelemetry), or pull based aggregation systems (e.g. Prometheus).

Metrics may also be emitted as structured fields via the same infrastructure.

This work will also need to be designed in tandem with Configuration, making it possible to
specify the desired log/trace level and metrics calculation options in a declarative way.


### 4.6 - Configuration

A configuration system, allowing users to specify all of the options that follow. Likely based on
configuration files, but potentially with integrations for environment variables and command line
options.

System-wide Performance and Resource Options, describing things like rate limiting, connection
pooling behaviors, timeouts and back-offs, and other similar parameters.

Together with Request Path Control, the design and implementation of the configuration system is
likely to be a significant part of the integration work. This is for two main reasons:

1. The configuration system is required to configure quite a bit of complexity, exposing a wide
   array of dials
2. The configuration system is largely the "user interface" of the system - meaning people will have
   strong opinions on how it should function.

In the future, there will likely be a need for a scripting interface, or integrated scripting
language/runtime, such as Rhai, WASM, or others.

Until then, it's recommended to be as conservative as possible in what can be done with the
configuration file, in order to meet the necessary feature set.

As configuration is the primary user interface, care should be taken to help users understand
the impact of their configuration choices.

### 4.7 - Environmental Requirements

In general, River is intended to be run on a Linux system for production usage. This maybe be on
"bare metal", in a virtual machine, or in a containerized environment.

The `pingora` engine allows for a "two stage" start, the first runs at whatever the user/group
context that was used to launch the program. This can be used to enable a greater level of access
such as loading secrets or configuration files from the filesystem. Once this "setup" phase is
completed, the program is forked, and "steady state" is launched using the user and group that was
configured.

It is not expected to require any additional work to support this use case - it is already
supported by `pingora` itself. However any code that wraps `pingora` may need to keep this
operational model in mind.

### 4.8 - Graceful Reloading

Graceful reloading allows operators to stop, reconfigure, and restart the River server, with minimal
or no visible downtime to downstream clients.

This capability is important, as other than Upstream Service Discovery, no other way is provided
to change configuration of operational River instances. This approach was chosen largely because:

1. This is the model chosen by `pingora`
2. It greatly simplifies logic - as we don't need to worry about "cache invalidation" of
   configuration or other settings.

It is not expected to require any additional work to support this use case - it is already
supported by `pingora` itself. However any code that wraps `pingora` may need to keep this
capability/working model in mind.

### 4.9 - Certificate Provisioning and Management

There is desire for River to be able to automatically provision certificates for domains served
by it. This presents as two major capabilities:

1. Obtaining a new certificate - on first run, it will be necessary to obtain a certificate before
   serving any TLS secured traffic
2. Renewing an existing certificate - in steady state, it will be necessary to periodically (on the
   order of weeks/months) renew a certificate, and replace old ones with new ones.

By having the reverse proxy perform this step automatically, it avoids the need to have manual or
other setups in order to deploy or manage the reverse proxy, such as one-shot or scheduled container
runs.

For new certificates: It is likely (though unspecified) how this should be achieved. It is likely
that if configured to obtain/manage certificates automatically, and none exist, this should be
performed BEFORE serving traffic for the relevant listeners.

For existing certificates: It is unspecified whether renewing certificates is something that should
be done "in flight", or whether it requires a graceful reload to occur.

In both cases, care should be taken (and documentation) should make it clear how these features
interact with potentially unprivileged "steady state" operational modes.

Where it is not possible to handle this "in flight", reference examples should be provided to
document how users are expected to setup their systems correctly.
