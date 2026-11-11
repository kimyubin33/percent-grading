# Vendored httpdate

HTTP date parsing and formatting for Hyper's HTTP/1 server.

- Upstream: https://github.com/pyfisch/httpdate
- Version: 1.0.3
- Pinned upstream revision: 53df52094835dd85aca70e23b9f384abcf843f7f
- License: MIT OR Apache-2.0; both upstream license files are preserved.
- Source, manifest, README, and declared benchmark are copied from upstream without code changes.

This small path patch makes server builds possible in an environment whose Cargo registry cache lacks httpdate and whose shell cannot resolve crates.io. It does not replace Hyper's HTTP parser or change the application API.
