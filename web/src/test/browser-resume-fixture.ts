/**
 * UI tests exercise API behavior after a trusted launcher has authenticated.
 * Return a syntactically valid browser capability from the authorization
 * exchange; keep all application API calls observable through the caller's
 * mock. This does not weaken or replace production authentication.
 */
export function withBrowserResume(
  mockFetch: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>,
) {
  return async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const path = String(input)
    if (path === '/auth/browser-resume' || path === '/auth/browser-resume/exchange') {
      return new Response(JSON.stringify({ browser_capability: 'v'.repeat(43) }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      })
    }
    return mockFetch(input, init)
  }
}
