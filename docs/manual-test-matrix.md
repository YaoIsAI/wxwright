# Manual test matrix (PRD 9-2, four-view regression)

Run per theme (`minimal`, `techblue`, `magazine`) each quarter and before any
release. Attach screenshots to a release issue.

## Preparation

```bash
wxwright convert examples/sample-article.md --theme <T> --out sample-<T>.html
wxwright copy examples/sample-article.md
```

## View 1: MP editor (paste)

- [ ] Paste into draft; body renders without style loss
- [ ] Headings/paragraph spacing matches preview
- [ ] Tables keep borders and centering; no horizontal overflow
- [ ] Code block keeps colors; long lines wrap (no truncation)
- [ ] Callout cards (NOTE/WARNING/KEYPOINT) keep background + left bar
- [ ] Images are re-hosted by the editor (become mmbiz automatically)
- [ ] Footnote link list appears at the end

## View 2: Published desktop article

- [ ] Fonts fall back gracefully (no font-family is set - by design)
- [ ] Quote cards keep rounded corners
- [ ] Lists keep indentation and task checkbox styling

## View 3: Published mobile article (iOS + Android)

- [ ] No text overlap (R-1.3: line-height >= font size everywhere)
- [ ] No horizontal scroll (R-1.4: no fixed px widths)
- [ ] Code wraps within the viewport
- [ ] Table cells do not squeeze text into vertical stacks

## View 4: Dark Mode (toggle in MP editor / system)

- [ ] Light backgrounds convert to dark cards
- [ ] Text stays readable (no invisible text)
- [ ] Gradient-under-text sections flatten to solid (R-4.1.2 warnings in the report)
- [ ] Images with transparency remain visible on #191919

## GUI additions (per release)

- [ ] Device frames: iPhone 15 Pro / Pixel 8 switch buttons under the phone; bezel, side keys, Dynamic Island / punch-hole, home bar all render; dark/light toggle on the device
- [ ] Compliance badge in the stats row: green when clean, colored counts otherwise; list never overlaps the AI drawer
- [ ] Pet 墨仔: looks like a sitting cat; bounces on typing, sleeps after 45s, hearts + quote on click, party on triple logo click
- [ ] 码聋 click opens the WeChat QR code modal
- [ ] Asset library: posters/AI images/pastes appear as thumbnails; insert/delete work
- [ ] ComfyUI: offline -> clear hint; online (if available) -> t2i and i2i produce images into the library
- [ ] Poster Studio: responsive at narrow widths; preview scales; export-and-insert works
- [ ] AI theme generation: invalid output gets rejected and retried; result applies immediately
- [ ] SVG kit: all 6 components insert with content; AI-generated component appears in the list with an AI badge and is deletable; image upload embeds a data URI
- [ ] Compliance panel shows localized empty text (never "undefined")
- [ ] Pet: idle theater visibly cycles forms; dblclick swaps with squash-flip; hearts are red
- [ ] Asset library open-folder opens the real directory
- [ ] Settings shows the Free Tokens card; link opens the site
- [ ] Language toggle EN/中文 re-renders all labels

## Regression log

| Date | Themes | Views | Findings | Action |
|---|---|---|---|---|
| (fill each run) | | | | |
