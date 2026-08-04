/// <reference types="node" />

import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const chromeComponents = [
  'src/App.svelte',
  'src/lib/AppearanceMenu.svelte',
  'src/lib/DetailPane.svelte',
  'src/lib/Sidebar.svelte',
  'src/lib/SignInPanel.svelte',
]

describe('Polar Observatory tokens', () => {
  it('defines the complete semantic token set for light and dark modes', () => {
    const css = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8')

    expect(css).toContain(":root[data-theme='light']")
    expect(css).toContain(":root[data-theme='dark']")
    for (const token of [
      '--surface',
      '--surface-raised',
      '--foreground',
      '--muted-foreground',
      '--border',
      '--focus',
      '--primary',
      '--destructive',
      '--success',
      '--warning',
    ]) {
      expect(css.match(new RegExp(`${token}:`, 'g'))).toHaveLength(2)
    }
  })

  it('keeps raw colors in the token sheet instead of feature components', () => {
    for (const file of chromeComponents) {
      const source = readFileSync(resolve(process.cwd(), file), 'utf8')
      expect(source, file).not.toMatch(/#[0-9a-f]{3,8}\b|\b(?:rgb|hsl|oklch)\(/i)
    }
  })
})
