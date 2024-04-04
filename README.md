# River

`river` is a Reverse Proxy Application based on the `pingora` library from Cloudflare.

## Current State

As part of the initial [Kickstart Spike], we are working towards an early preview of
the `river` tool. This work will be released in a binary format, see
[this issue][dist-rfc] for more details.

[Kickstart Spike]: https://github.com/memorysafety/river/milestone/1
[dist-rfc]: https://github.com/memorysafety/river/issues/15

**Until releases begin, there is no expectation of stability.**

### Demonstration steps

At the moment, `river` can be invoked from the command line. See `--help` for
all options.

Configuration is currently done exclusively via configuration file. See
[`test-config.toml`] for an example configuration file.

[`test-config.toml`]: ./source/river/assets/test-config.toml

The server can be launched as follows:

```sh
# starting in the root of this repository
cd source/river
cargo run --release -- --config-toml ./assets/test-config.toml
```

Requests against the proxy can be made via `curl`, for example with `HTTP`:

```
curl -v http://127.0.0.1:8080
* processing: http://127.0.0.1:8080
*   Trying 127.0.0.1:8080...
* Connected to 127.0.0.1 (127.0.0.1) port 8080
> GET / HTTP/1.1
> Host: 127.0.0.1:8080
> User-Agent: curl/8.2.1
> Accept: */*
>
< HTTP/1.1 403 Forbidden
< Server: cloudflare
< Date: Thu, 04 Apr 2024 13:17:27 GMT
< Content-Type: text/html
< Content-Length: 151
< Connection: keep-alive
< CF-RAY: 86f194286d6158de-TXL
<
<html>
<head><title>403 Forbidden</title></head>
<body>
<center><h1>403 Forbidden</h1></center>
<hr><center>cloudflare</center>
</body>
</html>
* Connection #0 to host 127.0.0.1 left intact
```

Or using HTTPS:

```
curl -vk -H 'host: one.one.one.one' https://127.0.0.1:4443 | wc -c
* processing: https://127.0.0.1:4443
  % Total    % Received % Xferd  Average Speed   Time    Time     Time  Current
                                 Dload  Upload   Total   Spent    Left  Speed
  0     0    0     0    0     0      0      0 --:--:-- --:--:-- --:--:--     0*   Trying 127.0.0.1:4443...
* Connected to 127.0.0.1 (127.0.0.1) port 4443
* ALPN: offers h2,http/1.1
} [5 bytes data]
* TLSv1.3 (OUT), TLS handshake, Client hello (1):
} [512 bytes data]
* TLSv1.3 (IN), TLS handshake, Server hello (2):
{ [122 bytes data]
* TLSv1.3 (IN), TLS handshake, Encrypted Extensions (8):
{ [6 bytes data]
* TLSv1.3 (IN), TLS handshake, Certificate (11):
{ [1028 bytes data]
* TLSv1.3 (IN), TLS handshake, CERT verify (15):
{ [264 bytes data]
* TLSv1.3 (IN), TLS handshake, Finished (20):
{ [52 bytes data]
* TLSv1.3 (OUT), TLS change cipher, Change cipher spec (1):
} [1 bytes data]
* TLSv1.3 (OUT), TLS handshake, Finished (20):
} [52 bytes data]
* SSL connection using TLSv1.3 / TLS_AES_256_GCM_SHA384
* ALPN: server did not agree on a protocol. Uses default.
* Server certificate:
*  subject: C=DE; ST=Berlin; L=Berlin; O=River Test Organization; OU=River Test Unit; CN=NOT FOR ACTUAL USE
*  start date: Apr  3 17:53:06 2024 GMT
*  expire date: Apr  1 17:53:06 2034 GMT
*  issuer: C=DE; ST=Berlin; L=Berlin; O=River Test Organization; OU=River Test Unit; CN=NOT FOR ACTUAL USE
*  SSL certificate verify result: self-signed certificate (18), continuing anyway.
* using HTTP/1.x
} [5 bytes data]
> GET / HTTP/1.1
> Host: one.one.one.one
> User-Agent: curl/8.2.1
> Accept: */*
>
{ [5 bytes data]
* TLSv1.3 (IN), TLS handshake, Newsession Ticket (4):
{ [233 bytes data]
* TLSv1.3 (IN), TLS handshake, Newsession Ticket (4):
{ [233 bytes data]
* old SSL session ID is stale, removing
{ [5 bytes data]
< HTTP/1.1 200 OK
< Date: Thu, 04 Apr 2024 13:19:07 GMT
< Content-Type: text/html; charset=utf-8
< Content-Length: 56604
< Connection: keep-alive
< CF-Ray: 86f196978c3e4534-TXL
< Access-Control-Allow-Origin: *
< Cache-Control: public, max-age=0, must-revalidate
< ETag: "5dd740d0e716a31c1b8437db0263fa93"
< Vary: Accept-Encoding
< referrer-policy: strict-origin-when-cross-origin
< x-content-type-options: nosniff
< Server: cloudflare
< alt-svc: h3=":443"; ma=86400
<
{ [570 bytes data]
100 56604  100 56604    0     0   310k      0 --:--:-- --:--:-- --:--:--  310k
* Connection #0 to host 127.0.0.1 left intact
56604
```

## License

Licensed under the Apache License, Version 2.0: ([LICENSE-APACHE](./LICENSE-APACHE)
or <http://www.apache.org/licenses/LICENSE-2.0>).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
