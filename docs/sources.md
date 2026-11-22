# Public source registries

Clay downloads examples; it does not install packages, run downloaded code, or certify production readiness. Catalog entries can be compositions and variants. Explicit demos are excluded; retained blocks carry `example: true`. Availability, license, preview, local imports and dependencies still need inspection with `show` and `get`. asciicn additionally downloads its explicitly referenced same-registry engine files and preserves dependent CSS configuration in `registryItems`; external dependency URLs are not followed by that resolver.

| Source ID | Library | License file | Admission scope |
| --- | --- | --- | --- |
| `evilcharts` | [Evil Charts](https://evilcharts.com/r/registry.json) | [MIT](https://github.com/legions-developer/evilcharts/blob/HEAD/LICENSE) | UI, components and blocks; explicit demos and pro flags excluded |
| `origin` | [Origin UI](https://raw.githubusercontent.com/cosscom/ui/main/apps/origin/registry.json) | [MIT](https://github.com/cosscom/ui/blob/HEAD/apps/origin/LICENSE.md) | UI, components and blocks; explicit demos and pro flags excluded |
| `kibo` | [Kibo UI](https://www.kibo-ui.com/r/registry.json) | [MIT](https://github.com/shadcnblocks/kibo/blob/HEAD/license.md) | UI, components and blocks; explicit demos and pro flags excluded |
| `reui` | [ReUI](https://reui.io/r/styles/base-nova/registry.json) | [MIT](https://github.com/keenthemes/reui/blob/HEAD/LICENSE.md) | UI modules only; examples excluded |
| `blocks-so` | [blocks.so](https://blocks.so/r/registry.json) | [MIT](https://github.com/ephraimduncan/blocks/blob/HEAD/LICENSE.md) | UI, components and blocks; explicit demos and pro flags excluded |
| `motion-primitives` | [Motion Primitives](https://motion-primitives.com/c/registry.json) | [MIT](https://github.com/ibelick/motion-primitives/blob/HEAD/LICENCE.md) | UI, components and blocks; explicit demos and pro flags excluded |
| `smoothui` | [SmoothUI](https://smoothui.dev/r/registry.json) | [MIT](https://github.com/educlopez/smoothui/blob/HEAD/LICENSE) | UI, components and blocks; explicit demos and pro flags excluded |
| `fancy` | [Fancy Components](https://fancycomponents.dev/r/registry.json) | [MIT](https://github.com/danielpetho/fancy/blob/HEAD/LICENSE) | UI modules only; examples excluded |
| `beui` | [BeUI](https://beui.dev/r/registry.json) | [MIT](https://github.com/starc007/ui-components/blob/HEAD/LICENSE) | UI, components and blocks; explicit demos and pro flags excluded |
| `uiarc` | [UIArc](https://uiarc.dev/r/registry.json) | [MIT](https://github.com/kuratlielia/arc-library/blob/HEAD/LICENSE) | Explicit free tier only |
| `devl` | [devl.dev](https://devl.dev/r/registry.json) | [MIT](https://github.com/sean-brydon/devl.dev/blob/HEAD/LICENSE) | UI, components and blocks; explicit demos and pro flags excluded |
| `cult` | [Cult UI](https://www.cult-ui.com/r/registry.json) | [MIT](https://github.com/nolly-studio/cult-ui/blob/HEAD/LICENSE.md) | UI, components and blocks; explicit demos and pro flags excluded |
| `8bitcn` | [8bitcn](https://www.8bitcn.com/r/registry.json) | [MIT](https://github.com/TheOrcDev/8bitcn-ui/blob/HEAD/license.md) | UI, components and blocks; explicit demos and pro flags excluded |
| `shadcn-space` | [Shadcn Space](https://shadcnspace.com/r/registry.json) | [MIT](https://github.com/shadcnspace/shadcnspace/blob/HEAD/LICENSE) | Explicit live free flag AND membership in the public MIT repository index |
| `ui-layouts` | [UI Layouts](https://ui-layouts.com/r/registry.json) | [MIT](https://github.com/ui-layouts/uilayouts/blob/HEAD/LICENSE) | UI, components and blocks; explicit demos and pro flags excluded |
| `spectrum` | [Spectrum UI](https://ui.spectrumhq.in/r/registry.json) | [Apache-2.0](https://github.com/arihantcodes/spectrum-ui/blob/HEAD/LICENSE) | UI, components and blocks; explicit demos and pro flags excluded |
| `ruixen` | [Ruixen UI](https://ruixen.com/registry.json) | [MIT](https://github.com/ruixenui/ruixen.com/blob/HEAD/LICENSE.md) | UI, components and blocks; explicit demos and pro flags excluded |
| `watermelon` | [Watermelon UI](https://registry.watermelon.sh/r/registry.json) | [MIT](https://github.com/WatermelonCorp/watermelon-platform/blob/HEAD/LICENSE) | UI, components and blocks; explicit demos and pro flags excluded |
| `avara` | [Avara UI](https://avaraui.com/r/registry.json) | [MIT](https://github.com/AlgorizeTech/Avara-UI/blob/HEAD/LICENSE) | UI, components and blocks; explicit demos and pro flags excluded |
| `hextaui` | [HextaUI](https://hextaui.com/r/registry.json) | [MIT](https://github.com/preetsuthar17/hextaui/blob/HEAD/LICENSE) | UI modules only; examples excluded |
| `vengeance` | [Vengeance UI](https://vengenceui.com/r/registry.json) | [MIT](https://github.com/Ashutoshx7/VengeanceUI/blob/HEAD/LICENSE) | UI, components and blocks; explicit demos and pro flags excluded |
| `componentry` | [Componentry](https://componentry.dev/r/registry.json) | [MIT](https://github.com/harshjdhv/componentry/blob/HEAD/LICENSE) | UI, components and blocks; explicit demos and pro flags excluded |
| `asciicn` | [asciicn](https://asciicn.dev/r/registry.json) | [MIT](https://github.com/opencoredev/asciicn/blob/HEAD/LICENSE) | UI, components and blocks; explicit demos and pro flags excluded |

ReUI additionally verifies anonymous, code-bearing access during sync and skips 401/403/404 modules; premium blocks are excluded. Shadcn Space requires both indices to be available. HextaUI excludes its 812 examples, and Fancy excludes its 99 demo blocks. A free flag is not a license grant for an unrelated paid catalog.

Preview URLs are emitted only for known documentation patterns or an upstream `meta.docs` URL. Missing previews remain `no-preview`; an HTTP 200 only establishes that a page responds.

Source LICENSE files were read and fingerprinted in `experiments/new-sources/license-audit.json`. They describe the public source repository. Preserve upstream copyright, license and any additional notices when adapting downloaded code. Spectrum acknowledges upstream code from shadcn, Magic UI and Aceternity; its Apache-2.0 repository license does not relicense those portions. devl.dev includes a separate MIT notice for visx patterns. Third-party dependency licenses and assets need their own attribution.

`THIRD_PARTY_NOTICES.md` and its generator concern linked Cargo dependencies. These integrations add source URLs and metadata, with no bundled upstream component code or new Cargo dependencies, so those generated notices are unchanged.

## UI SDK

`ui-sdk` indexes [UI SDK](https://ui-sdk.dev) from Vercel Labs ([Apache-2.0](https://github.com/vercel-labs/ui/blob/HEAD/LICENSE), license read from the `@ui-sdk/react` npm package). The catalog is the React component docs in `apps/docs/content/docs/react/components/`, one entry per page, with title and description from its frontmatter. `get` downloads, pinned to the current commit, the component's React examples (`examples/components/<name>/react/`, one file per variant), its CSS and the shared example styles and helpers, plus the docs page as `USAGE.md`. The examples import `@ui-sdk/react`, which stays a dependency. A component without React examples (currently `time-input`) has only usage documentation, so `get` exits 3 for it. Every entry carries the `beta` gap while upstream is in beta.

## Tremor

`tremor` indexes the copy-and-paste components of [Tremor](https://www.tremor.so) ([Apache-2.0](https://github.com/tremorlabs/tremor/blob/HEAD/LICENSE)), one entry per `src/components/<Name>/<Name>.tsx`. `tremor-blocks` indexes [Tremor Blocks](https://blocks.tremor.so) ([MIT](https://github.com/tremorlabs/tremor-blocks/blob/HEAD/LICENSE.md)), one entry per `src/content/components/<category>/<block>.tsx`; its description combines the category with the Tremor parts and static headings read from the block's code, so variants of one category can be told apart. `get` pins the current commit and downloads the entry file plus every local module it reaches through `@/` or relative imports, keeping upstream's `src/` layout so the alias still resolves. Test, story and changelog files are not followed. Blocks target Tailwind v3 and Recharts 2; components target Tailwind v4. Both repositories have been inactive since 2025, so every entry carries the `unmaintained` gap.

Loader-specific sources (HeroUI, Headless UI, HyperUI, TailGrids, Bencho, EasyUI, Animata, Feral UI and Tailark) remain deferred in this change; see the integration report for individual reasons.
