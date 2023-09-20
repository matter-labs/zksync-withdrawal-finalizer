# Changelog

## [0.1.45](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.44...v0.1.45) (2023-09-20)


### Bug Fixes

* **deps:** update rust crate chrono to 0.4.31 ([#189](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/189)) ([0983090](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/0983090b22b6ba96964da2d8803155d7be88b0fa))
* **deps:** update rust crate syn to 2.0.35 ([#191](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/191)) ([7b9a911](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/7b9a911c8f8b225ff0a61432745aa0639a158bf2))
* **deps:** update rust crate syn to 2.0.37 ([#192](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/192)) ([c1940f9](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/c1940f9433ecbbfad40ab2cf3aa15b1c4f0c2928))
* request finalization status of withdrawal on the fetch params phase ([#193](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/193)) ([426bcd5](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/426bcd561e1f663cbf4f49f630f4684bd9fdc762))

## [0.1.44](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.43...v0.1.44) (2023-09-15)


### Bug Fixes

* adds logging to l2 events and removes panics on closed channels ([#188](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/188)) ([73f0f2a](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/73f0f2a3b6a35493d0987de44883c582bcb77652))
* **deps:** update rust crate proc-macro2 to 1.0.67 ([#181](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/181)) ([46c4909](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/46c4909b3d79ccca90f70a6b4da3972c69efc5aa))
* **deps:** update rust crate syn to 2.0.33 ([#182](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/182)) ([05b48a8](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/05b48a84a1ca4047f667b2889d7202c4d0957bbe))
* everything but subscriptions is now http ([#186](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/186)) ([831bdf4](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/831bdf439cda7fb40ab51baa382a257a77194c50))
* if some withdrawals params fetching fails, do not derail whole service ([#187](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/187)) ([d6403c3](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/d6403c3c2d9f860c912d4ae4d595c4eb688d56ea))
* removes panics from client code ([#185](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/185)) ([c06651c](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/c06651c52c3f590600bc084b1fa170c958fb958c))

## [0.1.43](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.42...v0.1.43) (2023-09-12)


### Bug Fixes

* revert query withdrawals without data ([#173](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/173)) ([dc2ade5](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/dc2ade51a90fc1323a0b20b94e1d41c151952f5a))

## [0.1.42](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.41...v0.1.42) (2023-09-12)


### Bug Fixes

* query withdrawals without data ([#170](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/170)) ([8eef896](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/8eef896bb1df5363700cdac081b37163099ca234))

## [0.1.41](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.40...v0.1.41) (2023-09-11)


### Bug Fixes

* correctly calculate the indices of events for withdrawal parameters ([#153](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/153)) ([7678531](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/767853130bc483995955afa134b674d40a9a9f2e))
* **deps:** update rust crate chrono to 0.4.29 ([#160](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/160)) ([f295670](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/f2956702c8e5b4cf5ea9fe08710fb0b944c7bd9a))
* **deps:** update rust crate chrono to 0.4.30 ([#162](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/162)) ([40c58cd](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/40c58cd7b088e9d2eb2e233d2ab42f14902f9af0))
* **deps:** update rust crate ethers to 2.0.10 ([#163](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/163)) ([9ebf2a9](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/9ebf2a97e5aa09f139c8feff20275a4922739160))
* **deps:** update rust crate syn to 2.0.32 ([#165](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/165)) ([d433cfe](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/d433cfea5c072072f876952aa3b5f6e88c0f99ee))

## [0.1.40](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.39...v0.1.40) (2023-09-05)


### Bug Fixes

* broken glibc in outdated debian image ([#158](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/158)) ([96b8744](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/96b8744e52947800f30b48098c246435f6350881))

## [0.1.39](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.38...v0.1.39) (2023-09-05)


### Features

* removes toml config file support ([#142](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/142)) ([6366e16](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/6366e1616d08b9e12edb45477a5f8ccb19481bfa))


### Bug Fixes

* **deps:** update rust crate chrono to 0.4.27 ([#151](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/151)) ([18a73fa](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/18a73fa94e832ad9e109981c11495fa21de9dd6b))
* **deps:** update rust crate chrono to 0.4.28 ([#152](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/152)) ([89ecec2](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/89ecec2ac2220b45f442bb05810d09869421abc9))
* **deps:** update rust crate clap to 4.3.23 ([#138](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/138)) ([bfd80f8](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/bfd80f8341f09a8d152086d681cfe6d31e668535))
* **deps:** update rust crate ethers to 2.0.9 ([#144](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/144)) ([368e15d](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/368e15dcb81d1cee22796e9ab87ab2c89f5ffdc5))
* **deps:** update rust crate serde to 1.0.184 ([#140](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/140)) ([92633d6](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/92633d6275297f0a064b25b2870284f004e61041))
* **deps:** update rust crate serde to 1.0.185 ([#141](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/141)) ([e57b58f](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/e57b58f7aba50c0852fbcecf20efecbaf07967ce))
* **deps:** update rust crate serde to 1.0.186 ([#145](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/145)) ([e61c1e1](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/e61c1e1ed7fa4938df012045e6ef0359e31f7903))
* **deps:** update rust crate serde to 1.0.188 ([#149](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/149)) ([098669b](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/098669beb20975181f4ca1c8e06b84d3bdfc2a14))
* **deps:** update rust crate syn to 2.0.31 ([#155](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/155)) ([f45c427](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/f45c427e2e42ac633341cb9f0da537130a114807))
* **deps:** update rust crate thiserror to 1.0.48 ([#154](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/154)) ([1c2bf76](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/1c2bf764f20fbe740d850fa330b2375661126c26))
* **deps:** update rust crate url to 2.4.1 ([#150](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/150)) ([0d9fe51](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/0d9fe5153962b80b47a6d78b96907fa84a1f0b31))
* if racing on a pending tx in infura, backoff ([#157](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/157)) ([15f0131](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/15f013196e1503b8c23c5a46f4b06e7f03d80366))

## [0.1.38](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.37...v0.1.38) (2023-08-18)


### Bug Fixes

* remove noncemanager ([#136](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/136)) ([5e0d488](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/5e0d488c18d0bd327b51bb20f3d9ccf92e8caf0f))

## [0.1.37](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.36...v0.1.37) (2023-08-18)


### Bug Fixes

* use correct finalizer account address ([#133](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/133)) ([0689449](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/06894493a1474f1409c63ea3a70232a89dd9b65f))

## [0.1.36](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.35...v0.1.36) (2023-08-18)


### Features

* adds tx sender with gas adjustment ([#125](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/125)) ([6c84586](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/6c845861c2d4026c31708cfd70238dcf921fca90))


### Bug Fixes

* **deps:** update rust crate clap to 4.3.22 ([#131](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/131)) ([e333291](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/e3332914aac64452d4f0b073b56b07b6fc64599c))
* **deps:** update rust crate quote to 1.0.33 ([#128](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/128)) ([59ecc83](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/59ecc83394c0dae76f635d145f5708f354549bc8))
* **deps:** update rust crate syn to 2.0.29 ([#129](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/129)) ([ae452ba](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/ae452bab0903c22e382bc9fdd4bead8654117c42))
* **deps:** update rust crate thiserror to 1.0.45 ([#123](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/123)) ([1b18823](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/1b18823da4a2774212891c2ae20bf39a7023b1d0))
* **deps:** update rust crate thiserror to 1.0.46 ([#126](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/126)) ([b8b4d2c](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/b8b4d2cfec6d2a7d2d9d401dd75748c5c4c08e85))
* **deps:** update rust crate thiserror to 1.0.47 ([#130](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/130)) ([dc10c2b](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/dc10c2b3136f1234bb10109c474c7d56afaac694))
* **deps:** update rust crate tokio to 1.32.0 ([#127](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/127)) ([177260f](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/177260fecdecb87078c59a62b5a53e818b9288ac))

## [0.1.35](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.34...v0.1.35) (2023-08-14)


### Bug Fixes

* **deps:** update rust crate async-trait to 0.1.72 ([#97](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/97)) ([f803a00](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/f803a0026f058dc0865a298ad03490b5843a9b3a))
* **deps:** update rust crate async-trait to 0.1.73 ([#107](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/107)) ([1507c04](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/1507c047e7557f0ddf8722f3220fec4de78901d4))
* **deps:** update rust crate chrono to 0.4.26 ([#98](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/98)) ([52c8b2d](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/52c8b2daf4aec0e60dd2fa36bc9ca49a7d06b74b))
* **deps:** update rust crate clap to 4.3.21 ([#113](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/113)) ([1271b3d](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/1271b3dee948e3ebaddce34999ad7c80db2e63c3))
* **deps:** update rust crate itertools to 0.11.0 ([#114](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/114)) ([80b1847](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/80b1847cd06ceebd950c41d0662654c052612d5b))
* **deps:** update rust crate metrics to 0.21.1 ([#101](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/101)) ([879b366](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/879b366b1e56d56e360bfa1100a26878ae9cedab))
* **deps:** update rust crate num to 0.4.1 ([#102](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/102)) ([7deb151](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/7deb151de63d604402dab4ab7d37ccf15f4a9fd7))
* **deps:** update rust crate proc-macro2 to 1.0.66 ([#103](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/103)) ([0db6f9e](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/0db6f9e84871265af6cee645695ad4f0f47505dc))
* **deps:** update rust crate quote to 1.0.32 ([#104](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/104)) ([1d547ed](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/1d547ed0f5f857ab0eb6f85f16dc4d88634421ab))
* **deps:** update rust crate serde to 1.0.183 ([#105](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/105)) ([02e9d51](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/02e9d51eef725d96c032e88ae07bc735ff059097))
* **deps:** update rust crate syn to 2.0.28 ([#106](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/106)) ([e82ff59](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/e82ff590e9dbe4f0dc4f25f1baa249d7a32b12a4))
* **deps:** update rust crate thiserror to 1.0.44 ([#108](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/108)) ([a312a00](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/a312a00a61b43ee50eadbf0b82ec4adf1f60c0b4))
* **deps:** update rust crate tokio to 1.30.0 ([#115](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/115)) ([8cdaffb](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/8cdaffbd5ce418b39cafb9d0ab62fce52178e627))
* **deps:** update rust crate tokio to 1.31.0 ([#117](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/117)) ([4159ea4](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/4159ea4192a2c5484e1b9fd91e001402d5ccfce2))
* **deps:** update rust crate toml to 0.7.6 ([#109](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/109)) ([5a68841](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/5a688412b7a9ba47b251f964739b65d4c9b0d54e))
* **deps:** update rust crate tracing to 0.1.37 ([#110](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/110)) ([79e978f](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/79e978fb9b28a431a87c32044fe1a19356c236aa))
* **deps:** update rust crate url to 2.4.0 ([#116](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/116)) ([be74d3b](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/be74d3b8084a1023cf34ba4b93361a415f06354c))
* use bigdecimal exported by sqlx ([#121](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/121)) ([7e6cbb3](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/7e6cbb34f6bf871b7b448e67f4bec5ef8c143bcd))

## [0.1.34](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.33...v0.1.34) (2023-08-11)


### Bug Fixes

* fix events processing in the finalizer loop ([#90](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/90)) ([604c0ca](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/604c0ca112232bb27ec19ba315c82c12d1f88aa6))

## [0.1.33](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.32...v0.1.33) (2023-08-10)


### Bug Fixes

* try to avoid duplicates in l2 events ([#88](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/88)) ([5a75c64](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/5a75c64573a80a45600549ba1489435a149bdf65))

## [0.1.32](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.31...v0.1.32) (2023-08-07)


### Bug Fixes

* actually print the predictions not requests ([#86](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/86)) ([74bd76c](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/74bd76cbf39a9fc35c7f33bc02094193975a930a))

## [0.1.31](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.30...v0.1.31) (2023-08-07)


### Bug Fixes

* more prediction logging ([#84](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/84)) ([89b9df1](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/89b9df115c1cda313e0c82ae06e52d07aeb6d23b))

## [0.1.30](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.29...v0.1.30) (2023-08-06)


### Bug Fixes

* only select the executed withdrawals to be finalized ([#82](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/82)) ([c954585](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/c9545859e83e0dd358d683d6be9f11d683a7e462))

## [0.1.29](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.28...v0.1.29) (2023-08-06)


### Bug Fixes

* use correct query boundaries in the migrator ([#80](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/80)) ([50f9fdb](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/50f9fdb164f0e96f6c961013ca42d9201d726110))

## [0.1.28](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.27...v0.1.28) (2023-08-04)


### Bug Fixes

* add correct indices and optimize query ([#77](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/77)) ([a2941f5](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/a2941f5e5d38763317f3610e6343b7b2231a12c5))
* adds logs to transaction sending ([#79](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/79)) ([f5c24c0](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/f5c24c0de3136c24ccef80499472f464a9024a24))

## [0.1.27](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.26...v0.1.27) (2023-08-02)


### Bug Fixes

* backoff after failed finalizer loop iterations ([#75](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/75)) ([1b25e0e](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/1b25e0e098c9a2a484050c791404fabf039fc3e4))

## [0.1.26](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.25...v0.1.26) (2023-08-02)


### Features

* do not terminate on errors and meter the highest finalized batch number  ([#73](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/73)) ([9ce921b](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/9ce921bb6e20663c8dad9115761ed44d5b5764f8))

## [0.1.25](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.24...v0.1.25) (2023-08-02)


### Features

* adds more debugging statements to the finalizer loop 3 ([#71](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/71)) ([c7906b5](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/c7906b55418ea4abf4b945cbe6c488923e2e7b71))

## [0.1.24](https://github.com/matter-labs/zksync-withdrawal-finalizer/compare/v0.1.23...v0.1.24) (2023-08-02)


### Features

* adds more debugging statements to the finalizer loop ([#69](https://github.com/matter-labs/zksync-withdrawal-finalizer/issues/69)) ([ad54f75](https://github.com/matter-labs/zksync-withdrawal-finalizer/commit/ad54f75f62518ccf65c807a78b72e3c1364d7249))

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
