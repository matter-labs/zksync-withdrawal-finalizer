# Changelog

## [0.1.23](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.22...v0.1.23) (2023-08-02)


### Features

* Adds more debug logging to finalizer ([#67](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/67)) ([59cb856](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/59cb856fabb58ba62accc9f6dbd5cf950b167bcc))

## [0.1.22](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.21...v0.1.22) (2023-08-01)


### Features

* adds withdrawal finalizing logic. ([#56](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/56)) ([5297c02](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/5297c023d8db0e88166e234b8b91570a7c206f8b))
* watcher improvements ([#64](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/64)) ([ee52d81](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/ee52d8110f7c68e64680f2d4ec6f2e43d482d2f0))


### Bug Fixes

* signer has to be configured with concrete chainid ([#65](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/65)) ([44f91e4](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/44f91e43a30dbbe45eafb6bed619e33f0e24787a))

## [0.1.21](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.20...v0.1.21) (2023-07-24)


### Features

* do not ship build artifacts with docker image ([#57](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/57)) ([9271d6d](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/9271d6d8ed7d9c7ade230546f5f8e31e459f4fe7))


### Bug Fixes

* try fix releaseplease borked pr ([#60](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/60)) ([8dbc9c8](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/8dbc9c880305fa0971ce4e9f1df1e7b054118aad))

## [0.1.20](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.19...v0.1.20) (2023-07-15)


### Bug Fixes

* request all past tokens in one go ([#53](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/53)) ([0165979](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/016597974c9ae5668eecb1ce11255da928567cab))

## [0.1.19](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.18...v0.1.19) (2023-07-14)


### Bug Fixes

* ignore non deployer contract deployed events  ([#51](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/51)) ([46854da](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/46854da922fbbc93264e4d623e9e0b9927562058))

## [0.1.18](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.17...v0.1.18) (2023-07-13)


### Bug Fixes

* halven the pagination instead of linear ([#49](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/49)) ([8e36e5d](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/8e36e5dbaeab727977010d01b2c2f621834865f0))

## [0.1.17](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.16...v0.1.17) (2023-07-13)


### Bug Fixes

* pagination increase and decrase should not be in sync ([#47](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/47)) ([37b32be](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/37b32be03da90f30e754650146b908a4d5285adf))

## [0.1.16](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.15...v0.1.16) (2023-07-13)


### Bug Fixes

* try upscaling the pagination  ([#43](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/43)) ([286ff7a](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/286ff7a4f91ce55b09941ffbad87584f44bd7aa8))

## [0.1.15](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.14...v0.1.15) (2023-07-13)


### Bug Fixes

* try upscaling the pagination  ([#43](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/43)) ([286ff7a](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/286ff7a4f91ce55b09941ffbad87584f44bd7aa8))

## [0.1.14](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.13...v0.1.14) (2023-07-13)


### Bug Fixes

* run on a tag unquoted  ([#40](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/40)) ([46d23b3](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/46d23b3b787295a881477a4ad995c95f9d153c3c))

## [0.1.13](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.12...v0.1.13) (2023-07-13)


### Bug Fixes

* run docker build only on tags pushed ([#38](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/38)) ([ee144ab](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/ee144ab047773615cdd88b675b5f3cac015b40dd))

## [0.1.12](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.11...v0.1.12) (2023-07-13)


### Bug Fixes

* decrease pagination backoff step ([#34](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/34)) ([d430699](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/d43069952edb9561c4a6b7230048aaca471a1833))
