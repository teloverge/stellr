import {
  getThemePreference,
  setThemePreference,
  type ThemePreference,
} from './native-shell'

export type { ThemePreference }
export type ResolvedTheme = 'light' | 'dark'

interface ThemeControllerOptions {
  root?: HTMLElement
  media?: MediaQueryList
  getPreference?: () => Promise<ThemePreference>
  setPreference?: (preference: ThemePreference) => Promise<void>
}

export class ThemeController {
  preference = $state<ThemePreference>('system')
  resolved = $state<ResolvedTheme>('light')
  error = $state<string | null>(null)

  readonly #root: HTMLElement
  readonly #media: MediaQueryList
  readonly #getPreference: () => Promise<ThemePreference>
  readonly #setPreference: (preference: ThemePreference) => Promise<void>
  readonly #systemChanged: () => void
  #listening = false

  constructor(options: ThemeControllerOptions = {}) {
    this.#root = options.root ?? document.documentElement
    this.#media =
      options.media ??
      window.matchMedia?.('(prefers-color-scheme: dark)') ?? {
        matches: false,
        addEventListener: () => undefined,
        removeEventListener: () => undefined,
      } as unknown as MediaQueryList
    this.#getPreference = options.getPreference ?? getThemePreference
    this.#setPreference = options.setPreference ?? setThemePreference
    this.#systemChanged = () => this.#apply()
  }

  async start(): Promise<void> {
    try {
      this.preference = await this.#getPreference()
      this.error = null
    } catch (error) {
      this.preference = 'system'
      this.error = `Could not load appearance preference: ${String(error)}`
    }
    if (!this.#listening) {
      this.#media.addEventListener('change', this.#systemChanged)
      this.#listening = true
    }
    this.#apply()
  }

  async setPreference(preference: ThemePreference): Promise<void> {
    try {
      await this.#setPreference(preference)
      this.preference = preference
      this.error = null
      this.#apply()
    } catch (error) {
      this.error = `Could not save appearance preference: ${String(error)}`
      throw error
    }
  }

  destroy(): void {
    if (this.#listening) {
      this.#media.removeEventListener('change', this.#systemChanged)
      this.#listening = false
    }
  }

  #apply(): void {
    this.resolved =
      this.preference === 'system' ? (this.#media.matches ? 'dark' : 'light') : this.preference
    this.#root.dataset.theme = this.resolved
    this.#root.style.colorScheme = this.resolved
  }
}
