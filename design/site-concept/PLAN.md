# Sayl website redesign: concept plan (mockup, not production code)

## Palette
- Abyss #070F2E: page background, same as the app's dark mode
- Navy #0B1A4E: cards, panels, logo text
- Teal #00B6BD: primary action, the current in the background, progress
- Foam #DDF7F8: main text
- Silt #8FA3C7: secondary text
- Sand #F2C46B: used once only, for "coming next" and warning notes

## Type
- IBM Plex Sans Arabic (400/600/700) for everything, the same font as the app. The display weight is set large and tight (about 5.6rem) with Egyptian-colloquial headlines.
- ui-monospace is used only inside the terminal/CLI blocks.

## Layout
- Home (RTL): the hero puts the animated logo, headline, lead and download buttons on the right, and a live download card on the left showing 8 segments filling at different speeds (this is what multi-connection downloading looks like). Below that: six features as a ruled list (no card grid), the extension and CLI shown side by side, the v0.2.0 video strip, then the footer.
- Docs: a sticky sidebar with search and section links, plus a single readable column (68ch). Numbered steps use Arabic-Indic digits only where the content really is a sequence. Includes callouts, kbd keys and a FAQ accordion.
- Mobile: everything stacks into one column; the sidebar becomes just the search box.

## The one bold thing
A full-page torrent: thousands of streaks pushed by a flow field, moving right to left in the reading direction, with a stronger current along a bending channel. Everything else stays quiet.
The logo stays completely still. Only the colors move inside the letters, like water flowing (a slow teal/navy gradient drifting through the shape).
With prefers-reduced-motion the page shows a static frame.

## Libraries (recommendation for the real build)
- Background: **OGL** (about 30 KB) with a GPU fragment/particle shader. It does the same thing as the 2D canvas prototype here, but at 60fps with far more particles and almost no CPU. Three.js would be overkill for a single full-screen effect.
- Motion: **GSAP + ScrollTrigger** (free, including the plugins, since 2025) for the logo intro, the download-card demo, and having the current speed up a little as the user scrolls. **Lenis** for smooth scroll (optional).
- Logo: an animated SVG driven by GSAP (the logo never moves; only a gradient drifts inside it). Lottie isn't needed, since we already have the vector.
- Site: **Astro + Starlight** for the docs page. It handles RTL/Arabic, search (Pagefind) and AR/EN i18n out of the box, and deploys as static files on Cloudflare Pages.

## Figma MCP
It adds little value here. The design lives in code (shader + GSAP), and Figma can't preview that motion. It would only help if the owner wants to comment on static frames, or later for a social or store-listing asset set. Skip it for now.

## Copy
Short Egyptian Arabic, plain verbs, no slogans. English toggle comes in the real build (the button is a placeholder here).
