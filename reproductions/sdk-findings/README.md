# Reproductions of filed SDK findings

One directory per finding in `docs/findings/README.md`. Each reproduction
uses only the SDK's public API on a transaction the network applied, and
compares its result with the network's own hash.

| Directory | Finding | Upstream | Run | Result on the affected release |
|---|---|---|---|---|
| `dotnet-281/` | FND-001: stellar-dotnet-sdk 17.0.0 hashes a default-valued `PRECOND_V2` transaction as `PRECOND_TIME` | Beans-BV/dotnet-stellar-sdk#281 | `dotnet run` | wrong hash, exit 1 |
| `php-252/` | FND-002: stellar-php-sdk 1.18.0 re-encodes a `CREATE_CONTRACT_V2` SAC deploy as `CREATE_CONTRACT` | Soneso/stellar-php-sdk#252 | `composer install && php check.php` | wrong hash, exit 1 |
| `dart-305/` | FND-004: stellar_flutter_sdk 3.12.0 re-encodes a `CREATE_CONTRACT_V2` SAC deploy as `CREATE_CONTRACT` | Soneso/stellar_flutter_sdk#305 | `flutter pub get && dart run bin/check.dart` (the package needs the Flutter SDK) | wrong hash, exit 1 |
| `kmp-175/` | FND-005: kmp-stellar-sdk 1.18.0 hashes a default-valued `PRECOND_V2` transaction as `PRECOND_TIME` | Soneso/kmp-stellar-sdk#175 | `./fetch.sh && java -cp '.deps/*' Check.java` (JDK 17+) | wrong hash, exit 1 |
| `go-6062/` | FND-003: go-stellar-sdk v0.9.0 `txnbuild` cannot build `RevokeSponsorship` for a pool-share trustline | stellar/go-stellar-sdk#6062 | `go run .` | validation error, and the asset encodes as native |

Inputs:

- `dotnet-281`: mainnet transaction
  `b4a6d6a2dfc62268a5f864f51d761207641e0c72bd8eb277159005f0f3503db9`, ledger
  55000000. It is also in the mainnet sweep set
  (`docs/evidence/mainnet-sweep/`).
- `php-252`: testnet transaction
  `11446413b5a7a95f8bb18e616a5187f19a5a7b146fdef3c5c2c77e5ea87aaadc`, ledger
  5822970, status SUCCESS. `deploy-testnet.mjs` is the script that submitted
  it (a fresh friendbot account deploys the SAC of its own asset `LWV2`). It
  shows that core and the host accept the shape.
- `dart-305`: the same testnet transaction as `php-252`.
- `kmp-175`: the focus envelope of corpus entry
  `core.pubnet.cap21.precond-v2-default-fields.p22` (the same transaction as
  `dotnet-281`).
- `go-6062`: no transaction is needed, because the SDK refuses to build the
  operation. stellar-core accepts the shape (`RevokeSponsorshipOpFrame::doCheckValid`,
  and the "pool share trustline" section of `RevokeSponsorshipTests.cpp`).

These reproductions need the .NET 8 SDK, PHP 8 with Composer, the Flutter
SDK, and Go respectively. They are not part of `make` or CI.
