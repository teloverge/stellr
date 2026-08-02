const GITHUB_NAME = /^[A-Za-z0-9_.-]+$/

export function safeGithubIssueUrl(raw: string, expectedIssue: number): string | null {
  if (!Number.isSafeInteger(expectedIssue) || expectedIssue <= 0) return null

  let url: URL
  try {
    url = new URL(raw)
  } catch {
    return null
  }

  if (
    url.protocol !== 'https:' ||
    url.hostname !== 'github.com' ||
    url.port !== '' ||
    url.username !== '' ||
    url.password !== ''
  ) {
    return null
  }

  const match = /^\/([^/]+)\/([^/]+)\/issues\/([1-9]\d*)\/?$/.exec(url.pathname)
  if (match === null) return null

  const [, owner, repo, issueText] = match
  const issue = Number(issueText)
  if (
    !GITHUB_NAME.test(owner) ||
    !GITHUB_NAME.test(repo) ||
    !Number.isSafeInteger(issue) ||
    issue !== expectedIssue
  ) {
    return null
  }

  return `https://github.com/${owner}/${repo}/issues/${issue}`
}
