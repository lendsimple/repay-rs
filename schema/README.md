# Repay Gateway API schema

The Swagger 2.0 specifications the client is generated from, vendored so that builds
never depend on the network. Swashbuckle serves them next to the Swagger UI at
<https://api.sandbox.repayonline.com/rgapi/swagger/ui/index>.

| File | Source | Fetched | SHA-256 |
|---|---|---|---|
| `repay-v1.json` | <https://api.sandbox.repayonline.com/rgapi/v1/swagger> | 2026-11-04 | `203ff87e6aa2bf5219f67ddc8da50138fdd7c43f9622dc338948b10bc5a3edd4` |
| `repay-v1-internal.json` | <https://api.sandbox.repayonline.com/rgapi/v1-internal/swagger> | 2026-11-04 | `c1fbcfdc68d90c366a7f422ce29392b6b58fdf2f132ec368c71aa198638b0776` |

`repay-v1.json` is the public API (104 paths, 134 operations, 132 definitions).
`repay-v1-internal.json` adds the Adjustments operations (4 operations); its other
definitions are identical to the public ones.

The files are kept byte for byte as served (see `.gitattributes`).

## Updating

1. Download both files over the vendored ones:

   ```sh
   curl -fsS -o schema/repay-v1.json https://api.sandbox.repayonline.com/rgapi/v1/swagger
   curl -fsS -o schema/repay-v1-internal.json https://api.sandbox.repayonline.com/rgapi/v1-internal/swagger
   ```

2. Update the dates and checksums above (`shasum -a 256 schema/*.json`).
3. Regenerate the schema code and review the diff, and record user-visible changes in
   `CHANGELOG.md`.
