import type { HubPage } from '@/components/workspace-hub'

const SESSION_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
const VIEWS: readonly HubPage[] = [
  'projects', 'files', 'jobs', 'agents', 'models', 'providers', 'mcp', 'permissions', 'tasks',
]

export interface BrowserRoute { chatId: string | null; view: HubPage | null }

export function readBrowserRoute(search: string): BrowserRoute {
  const params = new URLSearchParams(search)
  const chatId = params.get('chat')
  const view = params.get('view')
  return {
    chatId: chatId && SESSION_ID.test(chatId) ? chatId : null,
    view: view && VIEWS.includes(view as HubPage) ? view as HubPage : null,
  }
}

export function browserRouteUrl(pathname: string, search: string, route: BrowserRoute): string {
  const params = new URLSearchParams(search)
  if (route.chatId && SESSION_ID.test(route.chatId)) params.set('chat', route.chatId)
  else params.delete('chat')
  if (route.view && VIEWS.includes(route.view)) params.set('view', route.view)
  else params.delete('view')
  const query = params.toString()
  // Native browser-resume launch secrets are conveyed via the hash and must
  // never be copied into browser history or a shared URL.
  return pathname + (query ? `?${query}` : '')
}
