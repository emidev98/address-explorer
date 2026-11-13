# Privacy notice

Address Explorer stores its database locally, or on the remote server you
configure. It does not include analytics, telemetry, advertising, or a
first-party tracking service.

## Third-party requests

When you import or sync an address, the address is sent to the configured
mempool.space-compatible endpoint. Mempool.space can therefore link every
address you track as belonging to the same wallet owner. This is an inherent
property of querying a public blockchain index; it is not a bug that can be
fixed by hiding a header.

Price features also contact:

- Coin Metrics (`community-api.coinmetrics.io`, or `api.coinmetrics.io` when a
  paid API key is configured);
- Frankfurter (`api.frankfurter.dev`) for fiat exchange rates.

These providers see the requested symbols, dates, and standard HTTP metadata.
A custom mempool or price hostname sees the same requests and should be trusted
accordingly.

## What the app does not send

Passwords, session tokens, and database contents are never sent to those
providers. Remote Address Explorer traffic is protected by HTTPS. Every
exported backup is encrypted with a separate passphrase.
