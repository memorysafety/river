# River - What is it?

This is a document that is intended to describe WHAT River is, without discussing the details of how it is or should be implemented.

This document is intended for potential users of the River application, with a secondary goal of serving as a "big picture" view for implementers.

## 1 - Abstract

River will be a reverse proxy application, utilizing the `pingora` reverse proxy engine from Cloudflare. It will be written in Rust. It will be configurable, allowing for options including routing, filtering, and modification of proxied requests.

## 2 - Functional Description

The primary behavior of a reverse proxy application is to act as an intermediary between downstream clients and upstream servers, terminating TLS for inbound connections if in use. The reverse proxy application may decide to accept or reject the connection at any point, and may decide to modify messages at any point.


**TODO PICTURE HERE**

*Figure 1: Proxying Behavior*

**TODO: Discuss Services yet? Downstreams + Upstreams on a per-Service basis?**

### 2.1 - Downstream

River operates by listening to one or more downstream Listener interfaces, accepting connections from clients.

**TODO: Diagram showing many-listeners to one-service**

1. River MUST accept connections via:
    1. Unix Domain Sockets
    2. TCP Sockets
        1. IPv4
        2. IPv6
2. River MUST support the termination of TLS sessions
3. River MUST support the specification of TLS algorithms used for a given downstream listener as a subset of all supported algorithms
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

River operates by making and maintaining connections to one or more upstream services, forwarding messages from clients.

**TODO: Diagram showing one-service to one-of-many connections**

1. River MUST support a configurable Time To Live (TTL) for DNS lookups
2. River MUST support a configurable timeouts for:
    1. Connections
    2. Requests
3. River MUST support pooling of connections, including:
    1. Reuse of TCP sessions for all HTTP versions
    2. Reuse of HTTP2.0 streams for HTTP2.0
4. River MUST support health checks of upstream servers
    1. **TODO: “Configurable TTL override & cache drop upon health check failure for backends' hostnames in DNS. (i.e. allow lower TTLs than the DNS standard; re-resolve DNS if health checks fail)”**
5. River MUST support load balancing of upstream servers
6. River MUST support sending information for protocols used for pre-proxying, including:
    1. v1 and v2 of the PROXY protocol
    2. Cloudflare Spectrum
    3. Akamai X-Forwarded-For (XFF) HTTP header field
7. River MUST support the configurable selection of a subset of upstream servers based on HTTP URI paths

### 2.3 - Upstream Service Discovery

River allows for the configurable runtime discovery of upstream servers, in order to dynamically handle changing sets of upstream servers without requiring a restart or reconfiguration of the application.

1. River MUST support the use of a fixed list of upstream servers
2. River MUST support the use of DNS-Service Discovery to provide a list of upstream servers for a given service
3. River MUST support the use of SRV records to provide a list of upstream servers for a given service
4. **TODO: xDS?**
5. River MUST have a configurable timeout for re-polling poll-based service discovery mechanisms
6. River MUST support the use of DNS TTL as timeout value for re-polling poll-based service discovery mechanisms

### 2.4 - Request Path Control

River allows for configurable behavior modifiers at multiple stages in the request and response process. These behaviors allow for the modification or rejection of messages exchanged in either direction between downstream client and upstream server

**TODO: Diagram goes here**

Figure 2: Request Path Lifecycle

1. River MUST support modifying or rejecting a connection at any of the following stages:
    1. Downstream Request Arrival
    2. Peer Selection
    3. Upstream Request Forwarding
    4. Upstream Response Arrival
    5. Downstream Request Forwarding
    6. (todo: response body filter stage?) - these are for “chunks”
    7. TODO: maybe ADD a request body filter
2. River MUST support rejecting a connection by returning an error response
3. River MUST support CIDR/API range-based filtering allow and deny lists
4. River MUST support rate limiting of requests or response on the basis of one or more of the following:
    1. TODO
5. River MUST support removal of HTTP headers on a glob or regex matching basis
6. River MUST support addition of fixed HTTP headers to a request
7. TODO: Do we need some kind of metadata/template/context based content matching or filling?
8. TODO: Normalization of headers/bodies?
    1. EX: URL/URI normalization using browser rules
    2. Some kind of OWASP list for this?
9. TODO: Support External Authentication Requests?
    * Make subrequest to auth provider - NGINX (free module, maybe 3rd party? - need the name)
    * <https://nginx.org/en/docs/http/ngx_http_auth_request_module.html>

### 2.5 - Observability

River allows for configurable observability settings, in order to support the operation and maintenance of the system.

1. River MUST support emitting structured logs at a configurable level
2. River MUST provide quantitative metrics and performance counters
3. River MUST support push- and pull-based methods of obtaining structured logs, metrics, and performance counters
4. River MUST support emitting logs and metrics locally to file, stdout, or stderr in a consistently structured format.

### 2.6 - Configuration

River MUST provide methods for configuration in order to control the behavior of the reverse proxy application

1. River MUST support the configuration of all configurable options via a human editable text file (e.g. TOML, YAML).
2. River MUST support emitting a configuration file containing all configuration items and default configuration settings as a command line option
3. River MUST support a subset of configuration options at the command line
4. River MUST document all command line configurable options via a help command
5. River MUST support a subset of configuration options via environment variables
6. River MUST support emitting a list of all configuration options configurable via environment variables as a command line option
7. River MUST give the following priority to configuration:
    1. Command Line Options (highest priority)
    2. Environment Variable Options
    3. Configuration File Options (lowest priority)
8. TODO: How to configure + command hot-reloads?

### 2.7 - Environmental Requirements

These requirements relate to the supported execution environment(s) of the application.

1. The application shall support execution in a Linux environment
    2. TODO: minimum kernel version? Additional details?
2. The application shall support execution without “root” or “administrator” privileges
3. The application shall support execution within a container

### 2.x - What else?

Collecting things that still need a final space

1. Automatic ACME/cert provisioning
2. Rustls support
3. Specification
4. Graceful restarts
