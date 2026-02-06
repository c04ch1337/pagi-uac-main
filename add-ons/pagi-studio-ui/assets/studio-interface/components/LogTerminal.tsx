import React, { useState, useEffect, useRef } from 'react';
import { Terminal, ChevronDown, ChevronUp, Trash2 } from 'lucide-react';

const DEFAULT_LOGS_URL = 'http://127.0.0.1:8001/api/v1/logs';

interface LogTerminalProps {
  logsUrl?: string;
  maxLines?: number;
}

const LogTerminal: React.FC<LogTerminalProps> = ({
  logsUrl = DEFAULT_LOGS_URL,
  maxLines = 500,
}) => {
  const [lines, setLines] = useState<string[]>([]);
  const [isOpen, setIsOpen] = useState(true);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setError(null);
    const es = new EventSource(logsUrl);
    es.onopen = () => setConnected(true);
    es.onerror = () => {
      setConnected(false);
      setError('Connection lost. Is the Gateway running at ' + logsUrl + '?');
    };
    es.onmessage = (e: MessageEvent) => {
      setLines((prev) => {
        const next = [...prev, e.data].slice(-maxLines);
        return next;
      });
    };
    return () => {
      es.close();
      setConnected(false);
    };
  }, [logsUrl, maxLines]);

  useEffect(() => {
    if (isOpen && bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [lines, isOpen]);

  const clear = () => setLines([]);

  return (
    <div className="border-t border-zinc-200 dark:border-zinc-800 bg-zinc-100 dark:bg-zinc-900/80 flex flex-col flex-shrink-0">
      <button
        type="button"
        onClick={() => setIsOpen((o) => !o)}
        className="flex items-center justify-between w-full px-4 py-2 text-left text-sm font-medium text-zinc-700 dark:text-zinc-300 hover:bg-zinc-200 dark:hover:bg-zinc-800 transition-colors"
      >
        <span className="flex items-center gap-2">
          <Terminal size={16} />
          Gateway log stream
          {connected && (
            <span className="w-2 h-2 rounded-full bg-green-500 animate-pulse" title="Connected" />
          )}
          {error && (
            <span className="text-amber-600 dark:text-amber-400 text-xs" title={error}>
              Disconnected
            </span>
          )}
        </span>
        <span className="flex items-center gap-2">
          <span className="text-xs text-zinc-500 dark:text-zinc-400">{lines.length} lines</span>
          {isOpen ? <ChevronDown size={16} /> : <ChevronUp size={16} />}
        </span>
      </button>
      {isOpen && (
        <>
          <div className="flex items-center justify-end gap-2 px-2 py-1 border-b border-zinc-200 dark:border-zinc-800">
            <button
              type="button"
              onClick={clear}
              className="p-1.5 text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-200 hover:bg-zinc-200 dark:hover:bg-zinc-800 rounded transition-colors"
              title="Clear log"
            >
              <Trash2 size={14} />
            </button>
          </div>
          <div
            className="h-40 overflow-y-auto overflow-x-auto p-3 font-mono text-xs text-zinc-800 dark:text-zinc-200 bg-zinc-50 dark:bg-zinc-950 border-b border-zinc-200 dark:border-zinc-800"
            style={{ minHeight: '10rem' }}
          >
            {lines.length === 0 && !error && (
              <div className="text-zinc-500 dark:text-zinc-400">Connecting to log stream…</div>
            )}
            {lines.map((line, i) => (
              <div key={i} className="whitespace-pre-wrap break-all">
                {line}
              </div>
            ))}
            <div ref={bottomRef} />
          </div>
        </>
      )}
    </div>
  );
};

export default LogTerminal;
