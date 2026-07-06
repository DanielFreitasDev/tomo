import { Component, type ErrorInfo, type ReactNode } from 'react'

interface Props {
  children: ReactNode
}

interface State {
  error: Error | null
}

/**
 * Last-resort boundary: a render throw anywhere below shows a recoverable
 * screen instead of a blank window. Deliberately self-contained — it must not
 * depend on stores, i18n, or transport, since any of those may be what failed.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null }

  static getDerivedStateFromError(error: Error): State {
    return { error }
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error('Tomo crashed while rendering:', error, info.componentStack)
  }

  render(): ReactNode {
    const { error } = this.state
    if (!error) return this.props.children

    return (
      <div className="flex h-full items-center justify-center bg-app p-6 text-primary">
        <div className="w-full max-w-md rounded-lg border border-default bg-surface p-6 shadow-(--shadow-lg)">
          <h1 className="font-mono text-base font-semibold text-primary">Something went wrong</h1>
          <p className="mt-2 text-sm text-secondary">
            Tomo hit an unexpected error and couldn't render. Your files on disk are untouched.
          </p>
          <pre className="mt-4 max-h-40 overflow-auto rounded-md border border-default bg-app px-3 py-2 font-mono text-xs text-danger">
            {error.message || String(error)}
          </pre>
          <div className="mt-5 flex justify-end gap-2">
            <button
              type="button"
              className="h-8 rounded-md border border-default px-3 text-sm font-medium text-secondary hover:bg-hover"
              onClick={() => this.setState({ error: null })}
            >
              Try again
            </button>
            <button
              type="button"
              className="h-8 rounded-md bg-accent px-3 text-sm font-medium text-accent-fg hover:bg-accent-hover"
              onClick={() => window.location.reload()}
            >
              Reload
            </button>
          </div>
        </div>
      </div>
    )
  }
}
