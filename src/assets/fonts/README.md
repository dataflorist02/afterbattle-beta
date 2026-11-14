# Fonts

Afterbattle serves every font from its local bundle and makes no runtime font requests (CSP `font-src 'self'`).

## Big Shoulders (numbers and headings, E5-flare §1)

- Project: Big Shoulders, by the Big Shoulders Project Authors (https://github.com/xotypeco/big_shoulders)
- Files taken from the npm package `@fontsource/big-shoulders` 5.3.0 (fetched with `npm pack`, not kept as a dependency)
- Weights 300, 500, 600; subsets `latin` and `latin-ext` (all Polish characters present)
- No tabular figures (`tnum`): numbers in tables stay right-aligned
- License: SIL Open Font License 1.1; see `OFL-BigShoulders.txt`

## Inter (text, spec 25 §3)

- Project: Inter, by the Inter Project Authors (https://github.com/rsms/inter)
- Files taken from the npm package `@fontsource/inter` 5.3.0 (fetched with pnpm, not kept as a dependency)
- Weights 400, 500, 600; subsets `latin` and `latin-ext`
- License: SIL Open Font License 1.1; see `OFL-Inter.txt`
