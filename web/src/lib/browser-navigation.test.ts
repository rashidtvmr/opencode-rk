import { describe, expect, it } from 'vitest'
import { browserRouteUrl, readBrowserRoute } from './browser-navigation'

const ID = '0195f36a-2997-7a89-a11a-fc3359b0aa77'

describe('browser route serialization', () => {
  it('supports safe chat links and view names', () => {
    expect(readBrowserRoute(`?chat=${ID}&view=agents`)).toEqual({ chatId: ID, view: 'agents' })
    expect(readBrowserRoute('?chat=../../.env&view=admin')).toEqual({ chatId: null, view: null })
  })
  it('preserves unrelated query parameters but strips native launch-token fragments', () => {
    expect(browserRouteUrl('/', '?foo=bar&chat=invalid', { chatId: ID, view: 'files' }))
      .toBe(`/?foo=bar&chat=${ID}&view=files`)
    expect(browserRouteUrl('/web', `?chat=${ID}&view=agents`, { chatId: null, view: null }))
      .toBe('/web')
  })
})
