import DOMPurify from 'dompurify'
import { marked } from 'marked'

export function renderIssueMarkdown(body: string): string {
  const rendered = marked.parse(body, { async: false }) as string
  return DOMPurify.sanitize(rendered)
}
