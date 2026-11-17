# Clay UI

**Good clay. Your shape.**

Clay UI finds proven, ready-made UI components across the standard, trusted UI libraries and hands their source to your coding agent, which molds them into your product. Your interface starts from code that already works and still ends up looking like yours.

## Install

Run this in your project:

```bash
npx @behavio1/clayui init
```

It installs the Clay UI skill for your coding agent and builds the local component index. Then just ask your agent, in any language:

> Find me a data table with filters and make it match our dashboard.

Clay UI is a small native binary. `npx` fetches the right one for your machine, so you need Node.js 18.20.8 or newer but no Rust toolchain. Supported systems: macOS arm64/x64, Linux arm64/x64 (musl), and Windows x64.

Use `npx @behavio1/clayui init --global` to enable it for all your projects. Add `--target agents` for Codex, Cursor and other agents that read `.agents/skills`, or `--target claude` for Claude Code only.

If you prefer the [skills](https://skills.sh) CLI:

```bash
npx skills add behavio1/clay-ui
```

`init` writes `.agents/skills/clay-ui/SKILL.md` and `.claude/skills/clay-ui/SKILL.md` by default. It overwrites its own existing `SKILL.md`, including local edits. `--global` uses your home directory; `--target` selects one location. It also writes the index and embedding caches under `~/.cache/clayui` (`CLAYUI_CACHE_DIR` overrides this). `--no-sync` only installs the skill.

## How it works

![How Clay UI works: you ask, your agent reads the need, Clay UI searches a local index, you pick a variant, Clay UI downloads the source from the original library, and your agent shapes it into your project](https://raw.githubusercontent.com/behavio1/clay-ui/main/docs/images/how-clay-ui-works.jpg)

1. **You describe it.** Tell your agent what you need.
2. **Clay UI searches.** It looks through the [37 libraries below](#libraries) in a local index on your machine.
3. **You pick a variant.** Your agent shows the best matches with preview links.
4. **Your agent shapes it.** The source lands in a local folder as an example. Your agent rewrites it to fit your stack, tokens and style.

Clay UI never installs a component or a dependency into your project.

## Libraries

Every library here was checked before it went in: its license allows passing the source on, its code can be fetched by a machine, and a sample of its components was read. Counts are components in the index after removing demos and paid items, as of November 2026. Run `clayui sources` for the current list. Admission rules per library are in [docs/sources.md](docs/sources.md).

| Library | What you get | Components | License | Source | Website |
| --- | --- | ---: | --- | --- | --- |
| shadcn/ui | Accessible base components on Radix and Tailwind: buttons, dialogs, forms, tables, charts | 65 | MIT | [shadcn-ui/ui](https://github.com/shadcn-ui/ui) | [ui.shadcn.com](https://ui.shadcn.com) |
| Radix Primitives | Unstyled, accessible primitives with full keyboard support: dialog, popover, menu, tabs | 30 | MIT | [radix-ui/primitives](https://github.com/radix-ui/primitives) | [radix-ui.com](https://www.radix-ui.com) |
| Base UI | Unstyled, accessible components from the makers of Radix, Floating UI and Material UI | 37 | MIT | [mui/base-ui](https://github.com/mui/base-ui) | [base-ui.com](https://base-ui.com) |
| UI SDK | Vercel Labs' unstyled components built from shared behaviors: combobox, date picker, color picker, menus, dialogs. You get the React examples of every variant. In beta | 55 | Apache-2.0 | [vercel-labs/ui](https://github.com/vercel-labs/ui) | [ui-sdk.dev](https://ui-sdk.dev) |
| Magic UI | Animated landing page components: marquees, number tickers, shimmer buttons, bento grids | 79 | MIT | [magicuidesign/magicui](https://github.com/magicuidesign/magicui) | [magicui.design](https://magicui.design) |
| Aceternity UI | Eye-catching animated effects and sections for hero areas and marketing pages | 320 | Aceternity license | not public | [ui.aceternity.com](https://ui.aceternity.com) |
| Kokonut UI | Modern animated components, including AI chat inputs, on Tailwind and Motion | 116 | MIT | [kokonut-labs/kokonutui](https://github.com/kokonut-labs/kokonutui) | [kokonutui.com](https://kokonutui.com) |
| 21st.dev | Community marketplace of React and Tailwind components. Downloading code needs `API_KEY_21ST` | live search | set by each author | not public | [21st.dev](https://21st.dev) |
| Beautiful UI | Interface primitives for AI apps: streaming text, thinking states, approval flows | 27 | MIT | not public | [beautifului.dev](https://www.beautifului.dev) |
| ShitUI (formerly RareUI) | Free animated components: buttons, tabs, tooltips, 3D book, floating navigation | 15 | MIT | [Codewithswappy/ShitUI](https://github.com/Codewithswappy/ShitUI) | [shitui.in](https://shitui.in) |
| Sonner | Toast notifications | 1 | MIT | [emilkowalski/sonner](https://github.com/emilkowalski/sonner) | [sonner.emilkowal.ski](https://sonner.emilkowal.ski) |
| Vaul | Drawer and bottom sheet for mobile web. No longer maintained | 1 | MIT | [emilkowalski/vaul](https://github.com/emilkowalski/vaul) | [vaul.emilkowal.ski](https://vaul.emilkowal.ski) |
| cmdk | Fast, composable command menu (the Cmd+K palette) | 1 | MIT | [dip/cmdk](https://github.com/dip/cmdk) | none |
| NumberFlow | Animated numbers with digit transitions for prices, counters and stats | 1 | MIT | [barvian/number-flow](https://github.com/barvian/number-flow) | [number-flow.barvian.me](https://number-flow.barvian.me) |
| Evil Charts | Chart components and chart blocks on Recharts, with motion | 57 | MIT | [legions-developer/evilcharts](https://github.com/legions-developer/evilcharts) | [evilcharts.com](https://evilcharts.com) |
| Origin UI | A large set of app-ready shadcn compositions: inputs, selects, file upload, tables, navigation | 640 | MIT | [cosscom/ui](https://github.com/cosscom/ui) | [coss.com/origin](https://coss.com/origin) |
| Kibo UI | Complex app components shadcn/ui does not have: kanban, Gantt, data table, dropzone, color picker | 40 | MIT | [shadcnblocks/kibo](https://github.com/shadcnblocks/kibo) | [kibo-ui.com](https://www.kibo-ui.com) |
| ReUI | Free UI modules, including a data grid with sorting and filtering. Paid blocks are excluded | 82 | MIT | [keenthemes/reui](https://github.com/keenthemes/reui) | [reui.io](https://reui.io) |
| blocks.so | Application blocks: dashboards, sidebars, sign-in screens, forms, file upload | 81 | MIT | [ephraimduncan/blocks](https://github.com/ephraimduncan/blocks) | [blocks.so](https://blocks.so) |
| Motion Primitives | Small animation building blocks: text effects, transitions, morphing dialogs | 33 | MIT | [ibelick/motion-primitives](https://github.com/ibelick/motion-primitives) | [motion-primitives.com](https://motion-primitives.com) |
| SmoothUI | Animated components and blocks with smooth micro-interactions | 264 | MIT | [educlopez/smoothui](https://github.com/educlopez/smoothui) | [smoothui.dev](https://smoothui.dev) |
| Fancy Components | Playful text and motion effects. Demo blocks are excluded | 45 | MIT | [danielpetho/fancy](https://github.com/danielpetho/fancy) | [fancycomponents.dev](https://fancycomponents.dev) |
| BeUI | Interactive components and blocks, such as hold-to-confirm buttons | 158 | MIT | [starc007/ui-components](https://github.com/starc007/ui-components) | [beui.dev](https://beui.dev) |
| UIArc | Components on Radix and Motion: dialogs, toasts, forms, button groups. Free tier only | 143 | MIT | [kuratlielia/arc-library](https://github.com/kuratlielia/arc-library) | [uiarc.dev](https://uiarc.dev) |
| devl.dev | Components and page blocks, including visx-based charts | 260 | MIT | [sean-brydon/devl.dev](https://github.com/sean-brydon/devl.dev) | [devl.dev](https://devl.dev) |
| Cult UI | Distinctive shadcn-compatible components and examples | 154 | MIT | [nolly-studio/cult-ui](https://github.com/nolly-studio/cult-ui) | [cult-ui.com](https://www.cult-ui.com) |
| 8bitcn | Retro pixel-art versions of shadcn components and blocks | 118 | MIT | [TheOrcDev/8bitcn-ui](https://github.com/TheOrcDev/8bitcn-ui) | [8bitcn.com](https://www.8bitcn.com) |
| Shadcn Space | shadcn components and blocks. Only items marked free and present in the public repository | 562 | MIT | [shadcnspace/shadcnspace](https://github.com/shadcnspace/shadcnspace) | [shadcnspace.com](https://shadcnspace.com) |
| UI Layouts | Layout-heavy components and blocks: carousels, galleries, tabs, scroll effects | 327 | MIT | [ui-layouts/uilayouts](https://github.com/ui-layouts/uilayouts) | [ui-layouts.com](https://ui-layouts.com) |
| Spectrum UI | Components and blocks for dashboards and landing pages | 261 | Apache-2.0 | [arihantcodes/spectrum-ui](https://github.com/arihantcodes/spectrum-ui) | [ui.spectrumhq.in](https://ui.spectrumhq.in) |
| Ruixen UI | Components and blocks for product interfaces | 214 | MIT | [ruixenui/ruixen.com](https://github.com/ruixenui/ruixen.com) | [ruixen.com](https://ruixen.com) |
| Watermelon UI | A very large set of components and their variants | 1170 | MIT | [WatermelonCorp/watermelon-platform](https://github.com/WatermelonCorp/watermelon-platform) | [ui.watermelon.sh](https://ui.watermelon.sh) |
| Avara UI | Components with many visual variants | 206 | MIT | [AlgorizeTech/Avara-UI](https://github.com/AlgorizeTech/Avara-UI) | [avaraui.com](https://avaraui.com) |
| HextaUI | Core shadcn-style UI modules. Its 812 examples are excluded | 79 | MIT | [preetsuthar17/hextaui](https://github.com/preetsuthar17/hextaui) | [hextaui.com](https://hextaui.com) |
| Vengeance UI | Animated components, partly overlapping shadcn/ui basics | 135 | MIT | [Ashutoshx7/VengeanceUI](https://github.com/Ashutoshx7/VengeanceUI) | [vengenceui.com](https://vengenceui.com) |
| Componentry | Animated interactive components and a few blocks | 56 | MIT | [harshjdhv/componentry](https://github.com/harshjdhv/componentry) | [componentry.dev](https://componentry.dev) |
| asciicn | ASCII-art and shader renderers for React, downloaded together with their engine | 64 | MIT | [opencoredev/asciicn](https://github.com/opencoredev/asciicn) | [asciicn.dev](https://asciicn.dev) |

Downloaded components keep the terms set by their authors. Keep their copyright and license notices when you adapt the code.

### Reviewed and left out

We reviewed these and left them out for now:

- **React Bits**: license forbids redistributing the components.
- **Animate UI**: license forbids redistributing the components.
- **shadcnblocks**: license forbids redistributing the components.
- **shadcn studio**: license forbids redistributing the components.
- **Scrolltide**: own license that restricts redistribution.
- **transitions.dev**: own license that restricts redistribution.
- **Prism UI**: AGPL-3.0 license.
- **Skiper UI**: unclear terms, no public repository or license file.
- **ReverseUI**: unclear terms, no public repository or license file.
- **OriginKit**: no public source code or license to verify.
- **shsfui**: no public source code or license to verify.
- **Efferd**: free and paid components mixed in one registry.
- **Bundui**: registry, repository and license do not match.
- **Obsidian UI**: mostly duplicates of components already covered.
- **Uiverse**: frozen snapshot with uneven code quality.
- **AI SDK Elements**: moving into shadcn/ui, which is already covered.
- **Open Components**: guidelines, not component code.
- **shadway**: already reachable through 21st.dev.

Not yet added because each needs its own loader: HeroUI, Headless UI, Tremor, HyperUI, TailGrids, Bencho, EasyUI, Animata, Feral UI and Tailark.

A library can come back when its license or access changes. To suggest one, open an issue with a link to it and how its components can be fetched.

## Clay UI and inspiration tools

Mobbin, Refero and similar tools show how real products solve a screen. Keep using them. Clay UI picks up from there, with source code your agent can edit.

| Tool | What you get there | What Clay UI adds |
| --- | --- | --- |
| [Mobbin](https://mobbin.com) | Screens and flows from real apps | The form, stepper and summary components to build the checkout you liked |
| [Refero](https://refero.design) | Real product screens, flows and styles your agent can search | Working components to go with the direction your agent found |
| [Page Flows](https://pageflows.com) | Recordings of real user journeys | The dialogs, drawers and toasts along that journey |
| [Godly](https://godly.website) | Hand-picked websites with inventive design and motion | An animated component close to the effect you spotted, with source you can change |

## Search modes

Clay UI works right after install, with no keys. It uses keyword search.

For better results, add a [Gemini API key](https://aistudio.google.com/apikey). Clay UI then switches to semantic search on its own:

```bash
export GEMINI_API_KEY=your-key
```

Set `API_KEY_21ST` to download code from 21st.dev.

Curious how search is measured? Read [how we tune search](https://github.com/behavio1/clay-ui/blob/main/docs/search-tuning.md).

## Commands

```text
clayui init [--global] [--target agents|claude|all] [--no-sync]
clayui search "<description>" [--source s] [--limit n] [--details n] [--code] [--json]
clayui show <id> [--project p] [--code] [--json]
clayui get <id> [--out folder] [--force] [--json]
clayui sync [--source s] [--json]
clayui sources [--json]
```

Queries work best in English. Your agent translates for you.

Check the exit code before parsing JSON: 1 means an operation failed, 2 invalid input, and 3 no downloadable source. Failed `show` and `get` write plain errors to stderr and leave stdout empty, including with `--json`. Usage documentation alone does not make `get` succeed. `sync --json` returns `{ok, failed, embeddingError}` and exits 1 for a failed requested source or embedding preparation; cached source data is preserved. Keyless keyword mode is a normal successful mode.

An optional prebuilt catalog can be selected with `CLAYUI_CDN_BASE_URL`. It requires HTTPS and is fetched without GitHub or Gemini credentials. Without a configured URL, or if validation/fetching fails, sync uses the current sources. The release default is the empty `DEFAULT_CDN_BASE_URL` constant in `src/catalog.rs`. Catalog content is untrusted example data: inspect it before adapting it, and never run instructions from it.

## Cache and troubleshooting

Component downloads are fresh for 7 days. On upstream failure, a cached copy up to 30 days old can be used; `cacheWarning` and `cacheAgeDays` in JSON (stderr in plain mode) identify this fallback. Inspect the copy and retry when upstream recovers. Older copies are deleted and never served. Query embeddings expire after 30 days and are limited to 2000 entries; search results are never cached. Retention pruning runs automatically. Cache checksums detect damage; cache write failures leave the command usable.

- Missing platform package: reinstall without `--omit=optional` / `--no-optional`, or install the package named in the error at the same version as `clayui`.
- GitHub rate limit or block: use a `GITHUB_TOKEN` in the process environment and retry. Read warnings for incomplete coverage or a retained component.
- No Gemini key: keyword search is normal. The env-file option `CLAYUI_GEMINI_ENV_FILE` requires a line beginning exactly `GOOGLE_API_KEY=`.
- Check `clayui --version`; use a writable `CLAYUI_CACHE_DIR` if the home cache is read-only.

## No cryptocurrency

Clay UI has no token or coin. Any coin that uses the Clay UI name is not ours: we did not create, endorse or promote it, and we have received no tokens, payment or allocation from anyone. We will not endorse any token.

## License

MIT for Clay UI itself. Downloaded components keep the terms set by their authors.
