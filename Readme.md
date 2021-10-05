# Generic API cache

This repo is supposed to take any target api such as `https://api.sampleapis.com`
and cache it without changing the request layout.


## Tech Stack

- Server: [Warp](docs.rs/warp/*)
- Cache: [Memcached](https://en.wikipedia.org/wiki/Memcached)


## Features

- [x] Https targets
- [x] Change Cache time in config
- [x] http method is transferred as well
- [x] http body is transferred as well

## Limits

### Memcached

Keys are up to 250 bytes long and values can be at most 1 megabyte in size.


