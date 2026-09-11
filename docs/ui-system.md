# Perch interface

The companion uses a compact desktop layout with a quiet green accent.
Shared tokens and primitives live in src/style.css; task-specific layouts live
in src/assistant/assistant.css.

- White surfaces on a cool neutral canvas; green indicates primary actions and
  active state. Warm semantic colours identify warnings, approvals, and errors.
- One system font: 25px page titles, 17px section titles, 13px body, 11–12px
  controls and supporting text, 9–10px compact metadata.
- Cards have an 18px radius and 16–18px padding; controls use a 10px radius and
  34–36px height. Layout gaps are 8, 12, 14, 16, or 20px.
- Navigation separates workspace pages from preferences. Each page starts at
  the top when selected. Composer drafts survive navigation.
- CollectionToolbar provides a labelled search field, labelled filter, result
  count, and explicit reset. Search and filter combine. Empty states distinguish
  an empty collection from no matches.
- Companion controls align to their content height. Pack authoring and secondary
  actions use native details/summary disclosure. Long task responses expand
  inline instead of introducing nested scroll areas.
- Markdown replies are parsed with Marked and sanitized with DOMPurify using
  a formatting-only allowlist. Images and active content are excluded; http(s)
  links use the existing backend URL-opening command.
- Visible focus outlines, native form semantics, selected navigation state, and
  reduced-motion support apply throughout. Narrow windows wrap toolbars and
  stack cards; very narrow windows use a horizontally scrolling navigation row.

Verification belongs in docs/verification.md. These rules describe the
implementation, not a claim that a live accessibility audit has passed.
