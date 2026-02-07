import React, { useState, useRef, useEffect, useMemo } from 'react';
import { Send, Cpu, User, Loader2, Copy, Check, Bot, Pin, BrainCircuit, AlertCircle } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';
import { Message, AppSettings } from '../types';
import ThoughtBlock from './ThoughtBlock';

interface ChatInterfaceProps {
  messages: Message[];
  isLoading: boolean;
  isStreaming: boolean;
  onSendMessage: (text: string) => void;
  onTogglePin: (id: string) => void;
  settings: AppSettings;
}

// Improved CodeBlock component with Copy functionality and enhanced styling
const CodeBlock = ({ node, inline, className, children, ...props }: any) => {
  const [isCopied, setIsCopied] = useState(false);
  const match = /language-(\w+)/.exec(className || '');
  const language = match ? match[1] : 'text';
  const codeString = String(children).replace(/\n$/, '');

  const handleCopy = async () => {
    if (!navigator.clipboard) return;
    try {
        await navigator.clipboard.writeText(codeString);
        setIsCopied(true);
        setTimeout(() => setIsCopied(false), 2000);
    } catch (e) {
        console.error("Failed to copy code", e);
    }
  };

  if (inline) {
    return (
      <code className="bg-zinc-200 dark:bg-zinc-800 px-1.5 py-0.5 rounded-md font-mono text-[0.85em] text-zinc-900 dark:text-zinc-100 border border-zinc-300 dark:border-zinc-700" {...props}>
        {children}
      </code>
    );
  }

  return (
    <div className="relative group my-4 rounded-lg overflow-hidden border border-zinc-200 dark:border-zinc-800 shadow-sm bg-zinc-50 dark:bg-[#1e1e1e]">
      {/* Code Block Header */}
      <div className="flex items-center justify-between px-3 py-1.5 bg-zinc-100 dark:bg-[#252526] border-b border-zinc-200 dark:border-zinc-800 select-none">
         <span className="text-[10px] text-zinc-500 dark:text-zinc-400 font-mono font-medium lowercase">
            {language}
         </span>
         <button 
           onClick={handleCopy}
           className="flex items-center gap-1.5 text-[10px] text-zinc-500 hover:text-zinc-900 dark:text-zinc-400 dark:hover:text-zinc-100 transition-colors px-1.5 py-0.5 rounded hover:bg-zinc-200 dark:hover:bg-zinc-700"
           title="Copy code"
         >
           {isCopied ? (
             <>
               <Check size={12} className="text-emerald-500" />
               <span className="text-emerald-600 dark:text-emerald-400 font-medium">Copied!</span>
             </>
           ) : (
             <>
               <Copy size={12} />
               <span>Copy</span>
             </>
           )}
         </button>
      </div>
      <SyntaxHighlighter
        style={vscDarkPlus}
        language={language}
        PreTag="div"
        {...props}
        customStyle={{
          margin: 0,
          borderRadius: 0,
          padding: '1rem',
          fontSize: '0.85rem',
          lineHeight: '1.6',
          backgroundColor: 'transparent', 
        }}
        codeTagProps={{
            style: {
                fontFamily: "Menlo, Monaco, Consolas, 'Courier New', monospace",
            }
        }}
      >
        {codeString}
      </SyntaxHighlighter>
    </div>
  );
};

// Robust Avatar component to handle image loading errors
const Avatar = ({ url, role, fallbackIcon }: { url?: string, role: 'user' | 'agi', fallbackIcon: React.ReactNode }) => {
  const [error, setError] = useState(false);
  
  useEffect(() => setError(false), [url]);

  if (url && !error) {
    return (
        <img 
            src={url} 
            alt={role} 
            className="w-full h-full object-cover transition-opacity duration-300" 
            onError={() => setError(true)}
        />
    );
  }
  
  return (
    <div className={`w-full h-full flex items-center justify-center ${role === 'agi' ? 'bg-gradient-to-br from-indigo-500 to-purple-600' : 'bg-zinc-200 dark:bg-zinc-700'}`}>
        {fallbackIcon}
    </div>
  );
};

const ChatInterface: React.FC<ChatInterfaceProps> = ({ messages, isLoading, isStreaming, onSendMessage, onTogglePin, settings }) => {
  const [input, setInput] = useState('');
  const [inputError, setInputError] = useState<string | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [expandedThoughts, setExpandedThoughts] = useState<Record<string, boolean>>({});
  
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 150)}px`;
    }
  }, [input]);

  // Auto-scroll to bottom
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, isLoading, isStreaming]);

  const handleSubmit = (e?: React.FormEvent) => {
    e?.preventDefault();
    if (isLoading || isStreaming) return;
    
    if (!input.trim()) {
      setInputError('Message cannot be empty.');
      return;
    }

    onSendMessage(input);
    setInput('');
    setInputError(null);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const copyToClipboard = async (id: string, text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedId(id);
      setTimeout(() => setCopiedId(null), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  };

  const toggleThoughts = (id: string) => {
    setExpandedThoughts(prev => ({
        ...prev,
        [id]: !(prev[id] ?? true)
    }));
  };

  const markdownComponents = useMemo(() => ({
    code: CodeBlock,
    p: ({node, ...props}: any) => <p {...props} className="mb-3 last:mb-0 leading-relaxed" />,
    a: ({node, ...props}: any) => <a {...props} className="text-blue-500 hover:text-blue-600 hover:underline" target="_blank" rel="noopener noreferrer" />,
    ul: ({node, ...props}: any) => <ul {...props} className="list-disc pl-5 mb-3 space-y-1" />,
    ol: ({node, ...props}: any) => <ol {...props} className="list-decimal pl-5 mb-3 space-y-1" />,
    li: ({node, ...props}: any) => <li {...props} className="pl-1" />,
    strong: ({node, ...props}: any) => <strong {...props} className="font-semibold text-zinc-900 dark:text-zinc-100" />,
    em: ({node, ...props}: any) => <em {...props} className="italic text-zinc-800 dark:text-zinc-200" />,
    del: ({node, ...props}: any) => <del {...props} className="line-through text-zinc-400 dark:text-zinc-500" />,
    h1: ({node, ...props}: any) => <h1 {...props} className="text-xl font-bold mt-4 mb-2 pb-1 border-b border-zinc-200 dark:border-zinc-700" />,
    h2: ({node, ...props}: any) => <h2 {...props} className="text-lg font-bold mt-3 mb-2" />,
    h3: ({node, ...props}: any) => <h3 {...props} className="text-md font-bold mt-2 mb-1" />,
    blockquote: ({node, ...props}: any) => <blockquote {...props} className="border-l-4 border-zinc-300 dark:border-zinc-700 pl-4 italic text-zinc-600 dark:text-zinc-400 my-3" />,
    table: ({node, ...props}: any) => <div className="overflow-x-auto my-4 rounded-lg border border-zinc-200 dark:border-zinc-700"><table {...props} className="w-full text-sm text-left" /></div>,
    thead: ({node, ...props}: any) => <thead {...props} className="bg-zinc-100 dark:bg-zinc-800 text-zinc-700 dark:text-zinc-300 uppercase font-medium" />,
    tbody: ({node, ...props}: any) => <tbody {...props} className="divide-y divide-zinc-200 dark:divide-zinc-700" />,
    tr: ({node, ...props}: any) => <tr {...props} className="hover:bg-zinc-50 dark:hover:bg-zinc-800/50 transition-colors" />,
    th: ({node, ...props}: any) => <th {...props} className="px-4 py-3 border-b border-zinc-200 dark:border-zinc-700" />,
    td: ({node, ...props}: any) => <td {...props} className="px-4 py-3" />,
    img: ({node, ...props}: any) => <img {...props} className="rounded-lg max-w-full h-auto my-2 border border-zinc-200 dark:border-zinc-700" />,
    hr: ({node, ...props}: any) => <hr {...props} className="my-6 border-zinc-200 dark:border-zinc-700" />,
  }), []);

  return (
    <div className="flex flex-col h-full max-w-4xl mx-auto w-full relative">
      {/* Messages Area */}
      <div className="flex-1 overflow-y-auto px-4 py-6 space-y-6">
        {messages.length === 0 && (
          <div className="h-full flex flex-col items-center justify-center text-zinc-400 dark:text-zinc-600 opacity-50 select-none transition-colors">
            <Cpu size={48} className="mb-4" />
            <p className="text-sm">PAGI Orchestrator Ready</p>
            <p className="text-xs mt-2 font-mono">Status: Idle</p>
          </div>
        )}

        {messages.map((msg, index) => (
          <div key={msg.id} className={`flex gap-4 ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}>
            {msg.role === 'agi' && (
              <div className="w-8 h-8 rounded-full bg-white dark:bg-zinc-800 flex items-center justify-center flex-shrink-0 border border-zinc-200 dark:border-zinc-700 transition-colors overflow-hidden">
                <Avatar 
                    url={settings.agiAvatar} 
                    role="agi" 
                    fallbackIcon={<Bot size={16} className="text-white drop-shadow-md" />} 
                />
              </div>
            )}
            
            <div className={`max-w-[85%] ${msg.role === 'user' ? 'items-end' : 'items-start'} flex flex-col`}>
              {/* User Alias Display */}
              {msg.role === 'user' && settings.userAlias && (
                <span className="text-[10px] text-zinc-500 dark:text-zinc-400 mb-1 px-1 font-mono uppercase tracking-wider">
                  {settings.userAlias}
                </span>
              )}
              {/* PAGI Name Display */}
              {msg.role === 'agi' && (
                <span className="text-[10px] text-zinc-500 dark:text-zinc-400 mb-1 px-1 font-mono uppercase tracking-wider">
                  PAGI
                </span>
              )}

              <div 
                className={`px-4 py-3 rounded-lg text-sm shadow-sm relative group transition-colors overflow-hidden
                  ${msg.role === 'user' 
                    ? 'bg-zinc-200 dark:bg-zinc-800 text-zinc-900 dark:text-zinc-100 border border-zinc-300 dark:border-zinc-700 rounded-tr-none' 
                    : 'bg-white dark:bg-zinc-900/50 text-zinc-800 dark:text-zinc-300 border border-zinc-200 dark:border-zinc-800/80 rounded-tl-none pr-10'
                  } 
                  ${msg.isError ? 'border-red-200 dark:border-red-900/50 bg-red-50 dark:bg-red-950/10 text-red-800 dark:text-red-200' : ''}
                  ${msg.isPinned && !msg.isError ? 'border-orange-200 dark:border-orange-900/50 bg-orange-50/30 dark:bg-orange-900/10' : ''}
                `}
              >
                {!msg.isError ? (
                  <>
                    <ReactMarkdown 
                        remarkPlugins={[remarkGfm]}
                        components={markdownComponents}
                    >
                        {msg.content}
                    </ReactMarkdown>
                    {/* Streaming Indicator */}
                    {msg.role === 'agi' && isStreaming && index === messages.length - 1 && (
                        <div className="mt-2 flex items-center gap-2 text-zinc-400 dark:text-zinc-500 animate-pulse">
                            <span className="w-1.5 h-1.5 bg-current rounded-full" />
                            <span className="text-[10px] font-mono uppercase tracking-widest">Processing</span>
                        </div>
                    )}
                  </>
                ) : (
                  msg.content
                )}

                {/* Message Actions (Toggle Thoughts, Pin, Copy) */}
                {msg.role === 'agi' && !msg.isError && (
                  <div className={`absolute top-2 right-2 flex items-center gap-1 transition-opacity duration-200 ${
                      msg.isPinned ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'
                  }`}>
                     {/* Toggle Thoughts Button */}
                     {settings.showThoughts && msg.thoughts && msg.thoughts.length > 0 && (
                        <button
                            onClick={() => toggleThoughts(msg.id)}
                            className={`p-1.5 rounded transition-all border ${
                                (expandedThoughts[msg.id] ?? true)
                                ? 'bg-indigo-100 dark:bg-indigo-900/30 text-indigo-600 dark:text-indigo-400 border-indigo-200 dark:border-indigo-800'
                                : 'bg-zinc-100/80 dark:bg-zinc-800/80 text-zinc-400 dark:text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-200 border-transparent hover:border-zinc-300 dark:hover:border-zinc-600'
                            }`}
                            title={(expandedThoughts[msg.id] ?? true) ? "Collapse thoughts" : "Expand thoughts"}
                        >
                            <BrainCircuit size={14} className={(expandedThoughts[msg.id] ?? true) ? "fill-current" : ""} />
                        </button>
                     )}
                     
                     <button
                        onClick={() => onTogglePin(msg.id)}
                        className={`p-1.5 rounded transition-all border ${
                            msg.isPinned 
                            ? 'bg-orange-100 dark:bg-orange-900/30 text-orange-600 dark:text-orange-400 border-orange-200 dark:border-orange-800'
                            : 'bg-zinc-100/80 dark:bg-zinc-800/80 text-zinc-400 dark:text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-200 border-transparent hover:border-zinc-300 dark:hover:border-zinc-600'
                        }`}
                        title={msg.isPinned ? "Unpin message" : "Pin message"}
                      >
                        <Pin size={14} className={msg.isPinned ? "fill-current" : ""} />
                      </button>
                      <button
                        onClick={() => copyToClipboard(msg.id, msg.content)}
                        className="p-1.5 rounded bg-zinc-100/80 dark:bg-zinc-800/80 hover:bg-zinc-200 dark:hover:bg-zinc-700 text-zinc-400 dark:text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-200 transition-all border border-transparent hover:border-zinc-300 dark:hover:border-zinc-600"
                        title="Copy Message"
                        aria-label="Copy message"
                      >
                        {copiedId === msg.id ? (
                          <Check size={14} className="text-emerald-500 dark:text-emerald-400" />
                        ) : (
                          <Copy size={14} />
                        )}
                      </button>
                  </div>
                )}
              </div>
              
              {/* Render Thoughts if AGI and enabled */}
              {msg.role === 'agi' && settings.showThoughts && msg.thoughts && (
                <div className="w-full mt-1">
                  <ThoughtBlock 
                    thoughts={msg.thoughts} 
                    isExpanded={expandedThoughts[msg.id] ?? true}
                    onToggle={() => toggleThoughts(msg.id)}
                  />
                </div>
              )}
              
              <span className="text-[10px] text-zinc-400 dark:text-zinc-600 mt-1 px-1">
                {new Date(msg.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
              </span>
            </div>

            {msg.role === 'user' && (
              <div className="w-8 h-8 rounded-full bg-zinc-200 dark:bg-zinc-300 flex items-center justify-center flex-shrink-0 overflow-hidden border border-zinc-300 dark:border-zinc-600">
                <Avatar 
                    url={settings.userAvatar} 
                    role="user" 
                    fallbackIcon={<User size={16} className="text-zinc-700 dark:text-zinc-900" />} 
                />
              </div>
            )}
          </div>
        ))}

        {isLoading && (
          <div className="flex gap-4 justify-start animate-pulse">
            <div className="w-8 h-8 rounded-full bg-white dark:bg-zinc-800 flex items-center justify-center border border-zinc-200 dark:border-zinc-700 overflow-hidden">
               <Avatar 
                   url={settings.agiAvatar} 
                   role="agi" 
                   fallbackIcon={<Bot size={16} className="text-white drop-shadow-md" />} 
                />
            </div>
            <div className="bg-white/50 dark:bg-zinc-900/30 px-4 py-3 rounded-lg border border-zinc-200 dark:border-zinc-800/50 flex items-center gap-2">
              <span className="w-2 h-2 bg-zinc-400 dark:bg-zinc-600 rounded-full animate-bounce" style={{ animationDelay: '0ms' }}/>
              <span className="w-2 h-2 bg-zinc-400 dark:bg-zinc-600 rounded-full animate-bounce" style={{ animationDelay: '150ms' }}/>
              <span className="w-2 h-2 bg-zinc-400 dark:bg-zinc-600 rounded-full animate-bounce" style={{ animationDelay: '300ms' }}/>
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input Area */}
      <div className="p-4 bg-white/80 dark:bg-zinc-950/80 backdrop-blur-md border-t border-zinc-200 dark:border-zinc-800/50 sticky bottom-0 z-10 transition-colors">
        <div className="max-w-4xl mx-auto relative group">
          <textarea
            ref={textareaRef}
            value={input}
            onChange={(e) => {
              setInput(e.target.value);
              if (inputError) setInputError(null);
            }}
            onKeyDown={handleKeyDown}
            placeholder="Input instructions for PAGI..."
            className={`w-full bg-zinc-50 dark:bg-zinc-900 border text-zinc-900 dark:text-zinc-200 text-sm rounded-lg px-4 py-3 pr-12 focus:outline-none focus:ring-1 transition-all resize-none max-h-[150px] placeholder:text-zinc-400 dark:placeholder:text-zinc-600
              ${inputError 
                ? 'border-red-400 dark:border-red-500/80 focus:border-red-500 focus:ring-red-500/20' 
                : 'border-zinc-300 dark:border-zinc-800 focus:border-zinc-400 dark:focus:border-zinc-700 focus:ring-zinc-400 dark:focus:ring-zinc-700'
              }
            `}
            rows={1}
          />
          <button
            onClick={() => handleSubmit()}
            disabled={isLoading || isStreaming}
            className={`absolute right-2 bottom-2 p-1.5 transition-colors disabled:opacity-30 disabled:hover:text-zinc-400
               ${inputError 
                 ? 'text-red-400 hover:text-red-600 dark:text-red-500 dark:hover:text-red-400' 
                 : 'text-zinc-400 dark:text-zinc-400 hover:text-orange-500 dark:hover:text-orange-400'
               }`}
          >
            <Send size={18} />
          </button>
        </div>
        <div className="max-w-4xl mx-auto mt-2 flex justify-between items-center px-1">
            <div className="flex items-center gap-3">
              <p className="text-[10px] text-zinc-400 dark:text-zinc-600 font-mono">
                  CONNECTED: 127.0.0.1:8001/api/v1/chat
              </p>
              {inputError && (
                <div className="flex items-center gap-1 text-[10px] text-red-500 dark:text-red-400 animate-in fade-in slide-in-from-left-1">
                  <AlertCircle size={10} />
                  <span>{inputError}</span>
                </div>
              )}
            </div>
            <span className={`text-[10px] font-mono opacity-60 transition-colors ${inputError ? 'text-red-400' : 'text-zinc-400 dark:text-zinc-600'}`}>
                {input.length} chars
            </span>
        </div>
      </div>
    </div>
  );
};

export default ChatInterface;