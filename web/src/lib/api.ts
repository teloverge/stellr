export function addSpace(body: { path?: string; repo?: string }): Promise<Response> {
  return fetch('/api/spaces', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  })
}

export function removeSpace(id: string): Promise<Response> {
  return fetch(`/api/spaces/${encodeURIComponent(id)}`, {
    method: 'DELETE',
    credentials: 'same-origin',
  })
}

export function refreshSpace(id: string): Promise<Response> {
  return fetch(`/api/spaces/${encodeURIComponent(id)}/refresh`, {
    method: 'POST',
    credentials: 'same-origin',
  })
}
