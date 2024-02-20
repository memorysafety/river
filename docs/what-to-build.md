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

This work will also need to be designed in tandem with Configuration, making it possible to
specify the desired request path control options in a declarative way.

### 4.5 - Observability

An observability system, allowing operators to inspect and make observations about the running
system, both in an exploratory way as a human, as well as an automated way as part of a larger
monitoring system.

### 4.6 - Configuration

A configuration system, allowing users to specify all of the options that follow. Likely based on
configuration files, but potentially with integrations for environment variables and command line
options.

System-wide Performance and Resource Options, describing things like rate limiting, connection
pooling behaviors, timeouts and back-offs, and other similar parameters.

### 4.7 - Environmental Requirements


A Service Discovery System, allowing for runtime updates of the list of potential upstream servers
to connect to.
