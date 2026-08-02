import { describe, expect, it } from 'vitest'
import { renderIssueMarkdown } from './markdown'

describe('issue Markdown rendering', () => {
  it('keeps Markdown structure while removing executable issue-body HTML', () => {
    const html = renderIssueMarkdown(
      '# Safe\n\n<script>window.pwned=1</script><img src=x onerror="window.pwned=2">',
    )

    expect(html).toContain('<h1>Safe</h1>')
    expect(html).not.toContain('<script')
    expect(html).not.toContain('onerror')
  })
})
