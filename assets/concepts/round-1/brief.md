# dohsub — logo exploration, round 1

Status: concept checkpoint, not approved production identity. Current assets/logo.svg and assets/banner.svg remain the installed identity until a direction is selected.

## Brief

- Exact name: **dohsub**. Meaning: subscribe on DoH.
- Product: a local subscription service that resolves proxy node hostnames through configurable DoH, manages imported nodes and publishes subscriptions.
- Audience: developers and technically confident proxy users. Main surfaces: README, terminal documentation, project avatar and a 1280 × 420 product banner.
- Intended character: clear, steady, lightweight, controllable. Avoid VPN/security promises and complicated network diagrams.
- Assumptions: no established brand equity needs preserving beyond the lowercase name; geometric original lettering and a symbol must work without colour or a tagline.
- Success: identifiable at 16/32 px, one-colour and reversed; the wordmark reads dohsub; one memorable gesture per mark. The logo identifies the product; the future banner explains its capability.

## Diagnosis of the starting identity

The existing three-node junction illustrates connectivity, but it does not provide a name-specific identifier or a subscription cue. Its thin lines and dots rely on the coloured tile for recognition. The redesign keeps the calm developer-tool tone and replaces the junction metaphor.

## Word map

- DoH → resolve, translate, changing address, dependable source.
- subscribe → receive, collect, keep current, slip, tab, subscript.
- local service → pocket, dock, steady base, contained endpoint.
- nodes → multiple entries, managed inventory; avoid literal webs of circles.
- Intersections: open d / receiving pocket; paired slips / receive and publish; mathematical subscript / subscribe on DoH.

## Longlist

Scores are internal comparative judgements (clarity, distinction, simplicity, relevance, small-size strength; 1–5 each), not external certification.

| Idea | Type | One-sentence concept | Score /25 |
| --- | --- | --- | --- |
| Open docket | Letterform | An open lowercase d is a receiving pocket with one stable upright. | 23 |
| Paired slips | Abstract | Two opposing, offset slips make a repeatable receive-and-publish sign. | 22 |
| Subscript | Custom wordmark | Put sub below the baseline of doh, like a dependency in a formula. | 21 |
| Binding margin | Abstract | A binding edge collects three node entries into one subscription. | 18 |
| Address window | Letterform | A square d contains a single address window. | 18 |
| Underline | Wordmark | One unbroken underline holds the changing node names together. | 17 |
| Local seal | Emblem | A compact round seal frames a quiet local endpoint. | 15 |
| Switch ticket | Pictorial | A punched ticket changes address while keeping its identity. | 17 |
| Cache fold | Abstract | A folded corner separates the next snapshot from the current one. | 18 |
| Resolution brackets | Lettermark | Matched brackets resolve into the letters d and s. | 16 |

Selected: A, B and C, because they test a letterform, an abstract mark and a typographic identity rather than recolouring the same symbol.

## Reference study

Used the installed logo-design library to study developer-tool conventions, negative space, letter d, links and book imagery. Studied construction in Kotlin, Patreon, Framer and Pagekit. No reference paths are copied into the concepts or distributed in project artwork. Obvious shields, globes, clouds, RSS arcs, node webs and interlocking chain logos were excluded.

## Construction

- Symbols use a 256 × 256 canvas; A/B lockups use 1056 × 256 and the C wordmark uses 792 × 256.
- Geometry uses integer coordinates, 32-unit structural strokes and open counters.
- A: a squared receiving bowl, a 32-unit right upright and a 24-unit entry gap.
- B: two 44-radius returns, a 32-unit band, 24-unit inner opening and 24-unit inter-module gap.
- C: original geometric d plus an underscore, with doh/sub at distinct sizes and baselines in the lockup.
- The lettering is constructed from original paths, not a typeface or font conversion. No installed fonts are required by the mark files.
- These are monochrome concepts. Final colour, small-size production variants and banner follow the chosen direction.

## Two-round self-review

Round 1: rendered the three symbols, wordmarks and a size/background/reference sheet. The main gaps were A's left-biased visible bounds, B's narrow inter-module space at 16 px, and C's repeated d_ in its wordmark arrangement. The geometric wordmark reads dohsub in a large render.

Round 2: moved A right by 8 units and up by 4 units (intentional optical centring); opened B's central gap from 16 to 24 units; reduced C's underscore height and made its primary lockup wordmark-only. Re-audited and re-rendered all three. A's vertical margin difference is intentional, not an uncorrected geometry error.

Review is by the implementer, not an independent designer. Audit scores are automated heuristics, not professional certification or legal clearance. Reference logos only appear in temporary internal comparison sheets, not in delivered project artwork.

Product banner is intentionally pending direction selection, following the installed logo-design skill's Phase 6 checkpoint. No final colour, icon kit or replacement of the existing main assets has been performed.

## Editable preview verification

The standalone design.html preview was checked in isolated Chrome at desktop/tablet sizes and with an explicit 390 CSS-pixel viewport. At 390 px it has no horizontal overflow, uses one column, preserves the actual 64/32/16 px size ladder, and supports reverse, wordmark hiding, reset and a fitting detail dialog. Escape closes the dialog and returns focus. SVG download content and offline loading were also checked.
