// Keep the test environment dependency-light. Accessibility assertions use
// Testing Library queries plus axe-core directly.

// React Aria uses CSS.escape when it manages collection focus. jsdom does not
// currently provide the CSS global, so install the small subset our generated
// IDs need.
if (typeof globalThis.CSS === 'undefined') {
  Object.defineProperty(globalThis, 'CSS', {
    configurable: true,
    value: {},
  })
}

if (typeof globalThis.CSS.escape !== 'function') {
  Object.defineProperty(globalThis.CSS, 'escape', {
    configurable: true,
    value: (value: string) => value.replace(/[^a-zA-Z0-9_-]/g, (character) => `\\${character}`),
  })
}
