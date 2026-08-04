import { describe, expect, it } from 'vitest'
import { safeGithubIssueUrl } from './github-url'

describe('safe GitHub issue URLs', () => {
  it('canonicalizes an HTTPS github.com issue URL for the expected issue', () => {
    expect(
      safeGithubIssueUrl(
        'https://github.com/teloverge/stellr/issues/42?notification=1#discussion',
        42,
      ),
    ).toBe('https://github.com/teloverge/stellr/issues/42')
  })

  it.each([
    'javascript:alert(1)',
    'http://github.com/teloverge/stellr/issues/42',
    'https://github.com.evil.test/teloverge/stellr/issues/42',
    'https://user@github.com/teloverge/stellr/issues/42',
    'https://github.com:8443/teloverge/stellr/issues/42',
    'https://github.com/teloverge/stellr/issues/99',
  ])('rejects %s', (url) => {
    expect(safeGithubIssueUrl(url, 42)).toBeNull()
  })
})
