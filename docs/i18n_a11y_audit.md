# Universus i18n & a11y Audit

## User-Facing Strings (for i18n extraction)
- Found in `/frontend/src/leaderboard.ts`, `/frontend/views/pages/shop.njk`, `/frontend/views/pages/chat.njk`, `/frontend/views/partials/nav.njk`, etc.
- Examples:
  - "All Items", "Resources", "Officers", "Boosts", "Premium"
  - "No active perks"
  - "Loading...", "Select a channel", "Type your message..."
  - Error messages: "Failed to fetch user info", "Error loading leaderboard"
  - Button/label text: "Send", "Cancel", "Notifications", "View Trades", "My Fleets"
  - Modal/dialog titles: "Purchase Item", "Purchase Dark Matter", "Send Private Message"
  - Table/column headers, tooltips, etc.

## Accessibility Attributes (a11y)
- Present: `aria-label`, `aria-live`, `aria-modal`, `role`, `tabindex`, `alt`, `aria-labelledby`, `aria-haspopup`, `aria-expanded`, `aria-hidden`
- Missing/Needs Review:
  - Some images have empty or missing `alt` attributes
  - Not all interactive elements have clear focus indicators
  - Some modals/dialogs use ARIA but need keyboard trap/focus management
  - Color contrast and keyboard navigation need systematic review

## Supported Languages/Locales
- Currently: `en` (English)
- Plan: Add more (e.g., `es`, `fr`, `de`, etc.)

## Accessibility Standard
- Target: [WCAG 2.1 AA](https://www.w3.org/WAI/standards-guidelines/wcag/)

## Next Steps
- Extract all user-facing strings to translation files
- Add i18n library/configuration
- Systematically review and improve accessibility attributes and patterns
- Add automated and manual a11y testing
