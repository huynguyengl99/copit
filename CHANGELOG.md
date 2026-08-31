# Changelog

All notable changes to this project will be documented in this file.

## [0.6.0] - 2026-08-31

### Features

- Resolve the components and packages an optional group needs ([b968272](https://github.com/huynguyengl99/copit/commit/b968272ef6324eece8cbe6ff77f771c832f3115e))

## [0.5.0] - 2026-08-11

### Features

- Read the registry index from copit-registry.json ([13ca8f1](https://github.com/huynguyengl99/copit/commit/13ca8f1aae7a519683e75b31d6d70bc39fcbbf3f))

### Miscellaneous

- Stop release hooks from rewriting the generated changelog ([4305243](https://github.com/huynguyengl99/copit/commit/4305243afc87274f7cb9dcf5b24825beb3c027dd))

### Testing

- Keep github fetch tests from sharing cached archives ([31668a5](https://github.com/huynguyengl99/copit/commit/31668a5cb45a0ec1114b543f5a21b1f90419d3c5))

## [0.4.1] - 2026-07-30

### Bug Fixes

- Reject install targets outside the project on Windows ([3bcd11c](https://github.com/huynguyengl99/copit/commit/3bcd11c855436bdf5c78951cb77bb90f8ec06276))

## [0.4.0] - 2026-07-30

### Features

- Add component registries with dependency-aware install ([f1577cc](https://github.com/huynguyengl99/copit/commit/f1577cc1b93243d08885c9616951f115d189f074))

### Miscellaneous

- Add rust-analyzer to the toolchain components ([d06732d](https://github.com/huynguyengl99/copit/commit/d06732d07cf13534f3d9f37461c0043e7762e278))

## [0.3.0] - 2026-03-11

### CI

- Add code coverage with Codecov and CI/coverage badges ([a2bea95](https://github.com/huynguyengl99/copit/commit/a2bea95e655fedd3f43a7aa4d7ee68c4dd2cca75))

### Documentation

- Add missing CLI flags and features to README and docs index ([b7a44a9](https://github.com/huynguyengl99/copit/commit/b7a44a9d1e58321356d6074399d9ee9a44950386))

### Features

- Add frozen flag, glob excludes, and rename sync to update-all ([f3d25c3](https://github.com/huynguyengl99/copit/commit/f3d25c3ca21517bdc84cd12731203857a56fdf8e))
- Add config-level settings with project defaults and per-source overrides ([d0a26d0](https://github.com/huynguyengl99/copit/commit/d0a26d01f025e0e373aa8cae7b524ef144ff8364))
- Flatten [project] config into root-level fields ([80774bb](https://github.com/huynguyengl99/copit/commit/80774bb0a201b170e0b78fcfd41bb14acb61df75))
- Implement license track ([3829505](https://github.com/huynguyengl99/copit/commit/3829505d8e77170912b9f7ebf4491ebb78e3ee78))
- Add licenses-sync command and refactor license path layout ([e715c17](https://github.com/huynguyengl99/copit/commit/e715c177a074278b8556a7230ff6b2d75d543a6f))

## [0.2.2] - 2026-03-08

### Bug Fixes

- Install script unbound variable error in exit trap ([57926f2](https://github.com/huynguyengl99/copit/commit/57926f2b621987b565aeeb486f230844a1be2e5f))

## [0.2.1] - 2026-03-08

### Bug Fixes

- Update docs with use cases and uninstall instructions ([2b61a61](https://github.com/huynguyengl99/copit/commit/2b61a61c63f2e7fd44fccbb3db95bc99ec406ae3))

## [0.2.0] - 2026-03-08

### Bug Fixes

- Update readme for pyproject.toml of python ([c40673c](https://github.com/huynguyengl99/copit/commit/c40673cf7b45647617e90f18380f259df3de64d0))

### Features

- Add project docs site with auto-generated CLI reference ([405536e](https://github.com/huynguyengl99/copit/commit/405536ecce05423486584bdaa2251e6a46c70aef))
- Add uninstall script for copit ([3f8b3b3](https://github.com/huynguyengl99/copit/commit/3f8b3b3177e30425c2f1111cfc8c51139ffff82c))
- Add versioned docs deployment with mike ([2731045](https://github.com/huynguyengl99/copit/commit/2731045e5446418d4cf659227a49d7b97c7f209a))

## [0.1.0] - 2026-03-08

### CI

- Add release script ([7d98b60](https://github.com/huynguyengl99/copit/commit/7d98b606bcfffe49d65ede40c68d489721b22e58))

### Features

- Init copit project ([f86c7b1](https://github.com/huynguyengl99/copit/commit/f86c7b1d4b2e8cfd9f9fe30f76b437f98aaca608))
