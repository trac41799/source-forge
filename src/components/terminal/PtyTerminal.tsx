import { useEffect, useRef, useImperativeHandle, forwardRef } from 'react'
import { Terminal } from 'xterm'
import { FitAddon } from '@xterm/addon-fit'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { IPC } from '../../lib/ipc/commands'
import { useAgentStore } from '../../stores/agentStore'
import { createPipeline } from '../../lib/pty/pipeline'
import type { AgentStatus } from '../../lib/types'

interface AgentOutputEventPayload {
  agent_id: string
  session_id: string
  line: string
  timestamp: string
}

interface PtyTerminalProps {
  agentId: string
  onOutput?: (cleanText: string) => void
  onStatusChange?: (status: AgentStatus) => void
}

export interface PtyTerminalHandle {
  write: (data: string) => void
  clear: () => void
  focus: () => void
}

const PtyTerminal = forwardRef<PtyTerminalHandle, PtyTerminalProps>(
  ({ agentId, onOutput, onStatusChange: _onStatusChange }, ref) => {
    const containerRef = useRef<HTMLDivElement>(null)
    const terminalRef = useRef<Terminal | null>(null)
    const fitAddonRef = useRef<FitAddon | null>(null)
    const pipelineRef = useRef<ReturnType<typeof createPipeline> | null>(null)
    const resizeObserverRef = useRef<ResizeObserver | null>(null)

    useEffect(() => {
      if (!containerRef.current) return

      const terminal = new Terminal({
        theme: {
          background: '#0d1117',
          foreground: '#e6edf3',
          cursor: '#e6edf3',
          cursorAccent: '#0d1117',
          selectionBackground: '#3b5070',
          black: '#0d1117',
          red: '#ff7b72',
          green: '#7ee787',
          yellow: '#d29922',
          blue: '#58a6ff',
          magenta: '#bc8cff',
          cyan: '#39c5cf',
          white: '#e6edf3',
          brightBlack: '#6e7681',
          brightRed: '#ffa198',
          brightGreen: '#56d364',
          brightYellow: '#e3b341',
          brightBlue: '#79c0ff',
          brightMagenta: '#d2a8ff',
          brightCyan: '#56d4dd',
          brightWhite: '#ffffff',
        },
        fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
        fontSize: 13,
        lineHeight: 1.2,
        cursorBlink: true,
        cursorStyle: 'bar',
        scrollback: 10000,
        convertEol: true,
      })

      const fitAddon = new FitAddon()
      terminal.loadAddon(fitAddon)

      terminal.open(containerRef.current)
      fitAddon.fit()

      terminalRef.current = terminal
      fitAddonRef.current = fitAddon

      if (onOutput) {
        pipelineRef.current = createPipeline([
          {
            name: 'output-handler',
            onText: onOutput,
          },
        ])
      }

      const handleResize = () => {
        fitAddon.fit()
        if (terminal.cols > 0 && terminal.rows > 0) {
          invoke(IPC.resizePty, {
            agentId,
            cols: terminal.cols,
            rows: terminal.rows,
          }).catch(() => {})
        }
      }

      const dataDisposable = terminal.onData((data) => {
        useAgentStore.getState().writeToAgent(agentId, data).catch(() => {})
      })

      let disposed = false
      let unlistenOutput: UnlistenFn | null = null
      listen<AgentOutputEventPayload>('agent-output', (event) => {
        if (event.payload.agent_id !== agentId) return
        terminalRef.current?.writeln(event.payload.line)
        pipelineRef.current?.feed(event.payload.line + '\n')
      })
        .then((fn) => {
          if (disposed) {
            fn()
          } else {
            unlistenOutput = fn
          }
        })
        .catch(() => {})

      const observer = new ResizeObserver(handleResize)
      observer.observe(containerRef.current)
      resizeObserverRef.current = observer

      return () => {
        disposed = true
        if (unlistenOutput) {
          unlistenOutput()
        }
        dataDisposable.dispose()
        observer.disconnect()
        pipelineRef.current?.destroy()
        terminal.dispose()
      }
    }, [agentId])

    useImperativeHandle(ref, () => ({
      write: (data: string) => {
        if (terminalRef.current) {
          terminalRef.current.write(data)
          pipelineRef.current?.feed(data)
        }
      },
      clear: () => {
        if (terminalRef.current) {
          terminalRef.current.clear()
        }
      },
      focus: () => {
        terminalRef.current?.focus()
      },
    }), [])

    return (
      <div
        ref={containerRef}
        className="w-full h-full min-h-[200px] bg-[#0d1117] rounded-md overflow-hidden"
      />
    )
  }
)

PtyTerminal.displayName = 'PtyTerminal'

export default PtyTerminal