# Introduction

This is the user/operator facing manual for the River reverse proxy application.

River is a reverse proxy application under development, utilizing the `pingora` reverse proxy engine
from Cloudflare. It is written in the Rust language. It is configurable, allowing for options
including routing, filtering, and modification of proxied requests.

River acts as a binary distribution of the `pingora` engine - providing a typical application
interface for configuration and customization for operators.

The source code and issue tracker for River can be found [on GitHub]

[on GitHub]: https://github.com/memorysafety/river

For developer facing documentation, including project roadmap and feature requirements for the
1.0 release, please refer to the `docs/` folder [on GitHub].
