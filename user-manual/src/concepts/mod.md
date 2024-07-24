# Core Concepts

River is a Reverse Proxy application.

It is intended to handle connections from **Downstream** clients, forward
**Requests** to **Upstream** servers, and then forward **Responses** from
the **Upstream** servers back to the **Downstream** clients.

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

For the purpose of this guide, we define **Requests** as messages sent
from the downstream client to the upstream server, and define **Responses**
as messages sent from the upstream server to the downstream client.

River is capable of handling connections, requests, and responses from
numerous downstream clients and upstream servers simultaneously.

When proxying between a downstream client and upstream server, River
may modify or block requests or responses. Examples of modification
include the removal or addition of HTTP headers of requests or responses,
to add internal metadata, or to remove sensitive information. Examples
of blocking include the rejection of requests for authentication or
rate limiting purposes.

## Services

River is oriented around the concept of **Services**. **Services** are
composed of three major elements:

* **Listeners** - the sockets used to accept incoming connections from
  downstream clients
* **Connectors** - the listing of potential upstream servers that requests
  may be forwarded to
* **Path Control Options** - the modification or filtering settings used
  when processing requests or responses.

Services are configured independently from each other. This allows a single
instance of the River application to handle the proxying of multiple different
kinds of traffic, and to apply different rules when proxying these different
kinds of traffic.

Each service also creates its own pool of worker threads, in order to allow for
the operating system to provide equal time and resources to each Service,
preventing one highly loaded Service from starving other Services of resources
such as memory and CPU time.

## Listeners

Listeners are responsible for accepting incoming connections and requests
from downstream clients. Each listener is a single listening socket, for
example listening to IPv4 traffic on address `192.168.10.2:443`.

Listeners may optionally support the establishment and termination of TLS.
They may be configured with a TLS certificate and [SNI], allowing them
to securely accept traffic sent to a certain domain name, such as
`https://example.com`.

[SNI]: https://www.cloudflare.com/en-gb/learning/ssl/what-is-sni/

Unlike some other reverse proxy applications, in River, a given listener
is "owned" by a single service. This means that multiple services may not
be listening to the same address and port. Traffic received by a given
Listener will always be processed by the same Service for the duration
of time that the River application is running.

Listeners are configured "statically": they are set in the configuration
file loaded at the start of the River application, and are constant for
the time that the River application is running.

## Connectors

Connectors are responsible for the communication between the Service and
the upstream server(s).

Connectors manage a few important tasks:

* Allowing for Service Discovery, changing the set up potential upstream servers over time
* Allowing for Health Checks, selectively enabling and disabling which upstream servers
  are eligible for proxying
* Load balancing of proxied requests across multiple upstream servers
* Optionally establishing secure TLS connections to upstream servers
* Maintaining reusable connections to upstream servers, to reduce the cost of connection
  and proxying

Similar to Listeners, each Service maintains its own unique set of Connectors. However,
Services may have overlapping sets of upstream servers, each of them considering an
upstream server in the list of proxy-able servers in their own connectors. This allows
multiple services to proxy to the same upstream servers, but pooled connections and
other aspects managed by Connectors are not shared across Services.

## Path Control

Path Control allows for configurable filtering and modification of requests and
responses at multiple stages of the proxying process.
