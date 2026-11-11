# dohsub wordmark

Approved direction: the complete lowercase name only, with custom typography and line work. No separate symbol or subscript. This supersedes the round-1 concepts.

- `../logo.svg`: primary ink wordmark, viewBox 615 × 162; artwork bounds x16–599, y16–146.
- `dohsub-white.svg`: reversed wordmark on dark backgrounds.
- `../banner.svg`: 1280 × 420 product banner; embedded outlined wordmark, system-font supporting text.
- `../../design.html`: standalone editable preview with background switching and SVG downloads.
- `dohsub-wordmark-v1.svg` / `v2.svg`: initial and optically re-spaced iterations. `previous/` preserves the earlier main assets.

The six letters are original filled paths, not a stock font. d/b share geometry; bowls overshoot the straight baseline by 2 units and s/u by 4 units. Main stems are 20 units, bowl horizontals 18; s terminals are diagonal and open. Recommended minimum wordmark width: 120 CSS px (about 25 px height). A complete six-letter wordmark is not suitable for a 16 × 16 favicon; no additional symbol is introduced. Keep clear space of at least one 20-unit stem around visible bounds. Do not distort, add outlines, crop letters, or use the six-letter mark as a square icon.

Digital palette: ink #151A18 (21,26,24), paper #F5F5EF (245,245,239), accent #A5C8AD (165,200,173). The accent belongs to banner labels/rules; the wordmark remains one colour. Print spot colours are not specified or proofed.

Two implementation self-review rounds inspect letter reading, spacing, curves and the wordmark at widths 240/160/120 px, one-colour and reversed. No trademark clearance or independent design review is claimed.

Verification: parsed all SVGs; the logo masters contain no live text, raster images or filters. Rendered and inspected both letter-spacing iterations and the product banner. Browser checks passed at actual 1440/960/390 CSS-pixel viewports: no overflow, unique IDs, correct 240/160/120 px wordmark samples, reverse preview, ink/white/banner SVG download content and no external resource loads. No runtime CLI code was changed.
